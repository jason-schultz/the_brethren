//! Starting a new game: load the opening map and put the player's ship in port.

use bevy::prelude::*;

use crate::alert::InSight;
use crate::cargo::Purse;
use crate::factions::{Colours, Nation, Relations};
use crate::goods::Goods;
use crate::map::data::{MapData, WEST_INDIES};
use crate::map::{PlayerStart, load_map};
use crate::merchants::{harbours, merchant_fleet};
use crate::patrols::patrol_squadron;
use crate::player::Player;
use crate::port::{Docked, Port};
use crate::raiders::raiding_party;
use crate::sailing::Heading;
use crate::ship_class::ShipClass;

/// The player's opening stake, in pieces of eight.
const STARTING_PURSE: u32 = 500;
/// Enough men to work a sloop, with a few to spare.
const STARTING_CREW: u32 = 12;
/// A fully armed sloop.
const STARTING_GUNS: u32 = ShipClass::SLOOP.guns;
/// About a month's provisions for the starting crew.
const STARTING_PROVISIONS: f64 = 2.0;

pub struct NewGamePlugin;

impl Plugin for NewGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                start_new_game,
                (launch_merchants, launch_patrols, launch_raiders),
            )
                .chain(),
        );
    }
}

fn start_new_game(goods: Res<Goods>, mut commands: Commands) {
    let map = MapData::parse(WEST_INDIES).expect("the embedded map is validated by map tests");
    let start =
        load_map(&mut commands, &map, &goods).expect("the embedded map is validated by map tests");
    commands.spawn(player_ship(&goods, start));
}

/// Puts the merchant fleet in the ports, once the map's ports exist.
fn launch_merchants(
    goods: Res<Goods>,
    ports: Query<(Entity, &Port, &Transform)>,
    mut commands: Commands,
) {
    for merchant in merchant_fleet(&goods, &harbours(&ports)) {
        commands.spawn(merchant);
    }
}

/// Puts the navy's patrols on station, once the map's ports exist.
fn launch_patrols(
    goods: Res<Goods>,
    relations: Res<Relations>,
    ports: Query<(Entity, &Port, &Transform)>,
    mut commands: Commands,
) {
    for patrol in patrol_squadron(&goods, &ports, &relations) {
        commands.spawn(patrol);
    }
}

/// Puts the Brethren's raiders in their havens, once the map's ports exist.
fn launch_raiders(
    goods: Res<Goods>,
    ports: Query<(Entity, &Port, &Transform)>,
    mut commands: Commands,
) {
    for raider in raiding_party(&goods, &ports) {
        commands.spawn(raider);
    }
}

/// The player's sloop, docked at the start port.
fn player_ship(goods: &Goods, start: PlayerStart) -> impl Bundle {
    let class = ShipClass::SLOOP;
    let mut hold = class.empty_hold();
    hold.owned.add(goods.provisions, STARTING_PROVISIONS);
    (
        Name::new("Player Sloop"),
        Player,
        class.build(STARTING_CREW, STARTING_GUNS, hold),
        Purse(STARTING_PURSE),
        Colours(Nation::England),
        InSight::default(),
        Heading(start.heading),
        Transform::from_translation(start.position),
        Docked { port: start.port },
    )
}

/// A headless simulation with the real map loaded, the player's ship in
/// port, and the merchant fleet launched. Returns the player's ship.
#[cfg(test)]
pub(crate) fn new_game() -> (App, Entity) {
    use bevy::ecs::system::RunSystemOnce;

    let mut app = crate::test_support::sim_app();
    let ship = app
        .world_mut()
        .run_system_once(|goods: Res<Goods>, mut commands: Commands| {
            let map = MapData::parse(WEST_INDIES).unwrap();
            let start = load_map(&mut commands, &map, &goods).unwrap();
            commands.spawn(player_ship(&goods, start)).id()
        })
        .unwrap();
    app.world_mut().run_system_once(launch_merchants).unwrap();
    app.world_mut().run_system_once(launch_patrols).unwrap();
    app.world_mut().run_system_once(launch_raiders).unwrap();
    // Builds the navigation graph for the new land.
    app.update();
    (app, ship)
}

/// Clears the sea of raiders and patrols, for tests about the passage
/// rather than who meets her on it.
#[cfg(test)]
pub(crate) fn calm_seas(app: &mut App) {
    let hunters: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, Or<(With<crate::raiders::Raider>, With<crate::patrols::Patrol>)>>(
        )
        .iter(app.world())
        .collect();
    for hunter in hunters {
        app.world_mut().despawn(hunter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Hold;
    use crate::economy::jobs::Job;
    use crate::map::Land;
    use crate::navigation::route::{LAND_CLEARANCE, NavGraph};
    use crate::navigation::{Arrival, SailOrder};
    use crate::port::{CastOff, Port};
    use crate::test_support::{run_seconds, run_until};
    use crate::time_scale::MAX_STEP;

    fn port(app: &mut App, name: &str) -> (Entity, Vec3) {
        app.world_mut()
            .query::<(Entity, &Name, &Transform)>()
            .iter(app.world())
            .find(|(_, port_name, _)| port_name.as_str() == name)
            .map(|(entity, _, transform)| (entity, transform.translation))
            .unwrap_or_else(|| panic!("no port {name}"))
    }

    #[test]
    fn the_player_starts_docked_at_port_royal_ready_to_sail() {
        let (mut app, ship) = new_game();
        let (port_royal, _) = port(&mut app, "Port Royal");
        let world = app.world();
        assert_eq!(
            world.get::<Docked>(ship),
            Some(&Docked { port: port_royal })
        );
        assert_eq!(world.get::<Purse>(ship), Some(&Purse(STARTING_PURSE)));
        assert!(
            world.get::<crate::crew::Crew>(ship).unwrap().count >= ShipClass::SLOOP.complement.min
        );
        let goods = world.resource::<Goods>();
        assert!(world.get::<Hold>(ship).unwrap().owned.get(goods.provisions) > 0.0);
    }

    #[test]
    fn the_opening_map_has_work_for_ports_in_need() {
        let (mut app, _) = new_game();
        run_seconds(&mut app, 1.0);
        let (kingston, _) = port(&mut app, "Kingston");
        let (nassau, _) = port(&mut app, "Nassau");
        let (port_royal, _) = port(&mut app, "Port Royal");
        let goods = app.world().resource::<Goods>().clone();
        let jobs: Vec<(Entity, Entity, String)> = app
            .world_mut()
            .query::<&Job>()
            .iter(app.world())
            .map(|job| (job.origin, job.destination, goods.get(job.good).key.clone()))
            .collect();
        let posted =
            |origin, destination, key: &str| jobs.contains(&(origin, destination, key.to_string()));
        assert!(posted(kingston, port_royal, "naval_stores"), "{jobs:?}");
        assert!(posted(kingston, nassau, "provisions"), "{jobs:?}");
        assert!(posted(port_royal, kingston, "soldiers"), "{jobs:?}");
        assert!(posted(nassau, kingston, "salt"), "{jobs:?}");
    }

    #[test]
    fn every_port_can_reach_every_other_by_sea() {
        let (mut app, _) = new_game();
        let ports: Vec<(String, Vec3)> = app
            .world_mut()
            .query_filtered::<(&Name, &Transform), With<Port>>()
            .iter(app.world())
            .map(|(name, transform)| (name.to_string(), transform.translation))
            .collect();
        let world = app.world();
        let land = world.resource::<Land>();
        let graph = world.resource::<NavGraph>();
        for (from_name, from) in &ports {
            for (to_name, to) in &ports {
                if from_name == to_name {
                    continue;
                }
                let route = graph.plan(land, *from, *to);
                let mut leg_start = *from;
                for (i, waypoint) in route.iter().enumerate() {
                    // The first and last legs may hug an anchorage close to shore.
                    let required = if i == 0 || i == route.len() - 1 {
                        1.0
                    } else {
                        LAND_CLEARANCE
                    };
                    assert!(
                        land.clearance(leg_start, *waypoint) >= required,
                        "{from_name} to {to_name}: leg {i} runs aground"
                    );
                    leg_start = *waypoint;
                }
            }
        }
    }

    /// Sails the player from Port Royal to `port`, asserting the ship never
    /// comes within 10 m of land, and returns whether she docked in time.
    fn voyage(app: &mut App, ship: Entity, port_name: &str, max_hours: f32) -> bool {
        let (destination, anchorage) = port(app, port_name);
        app.world_mut().write_message(CastOff { ship });
        app.world_mut().write_message(SailOrder {
            ship,
            destination: anchorage,
            on_arrival: Arrival::Dock(destination),
        });
        let docked = run_until(app, max_hours * crate::clock::HOUR as f32, |world| {
            let position = world.get::<Transform>(ship).unwrap().translation;
            let room = world.resource::<Land>().clearance(position, position);
            assert!(room > 10.0, "came within {room} m of land at {position}");
            world.get::<Docked>(ship).is_some()
        });
        docked && app.world().get::<Docked>(ship) == Some(&Docked { port: destination })
    }

    #[test]
    fn player_can_sail_to_nassau_at_full_time_compression() {
        let (mut app, ship) = new_game();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        calm_seas(&mut app);
        assert!(
            // About 30 hours; the margin catches passages getting tediously slow.
            voyage(&mut app, ship, "Nassau", 40.0),
            "never reached Nassau"
        );
    }

    #[test]
    fn merchants_earn_their_keep_within_a_few_days() {
        let (mut app, _) = new_game();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let earned = run_until(&mut app, 5.0 * crate::clock::DAY as f32, |world| {
            world
                .iter_entities()
                .filter(|entity| entity.contains::<crate::merchants::Merchant>())
                .any(|entity| entity.get::<Purse>().unwrap().0 > 400)
        });
        assert!(earned, "no merchant delivered anything");
    }

    #[test]
    fn merchants_put_to_sea_within_the_first_hours() {
        let (mut app, _) = new_game();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let mut most_at_sea = 0;
        run_until(&mut app, 2.0 * crate::clock::HOUR as f32, |world| {
            let at_sea = world
                .iter_entities()
                .filter(|e| e.contains::<crate::merchants::Merchant>() && !e.contains::<Docked>())
                .count();
            most_at_sea = most_at_sea.max(at_sea);
            false
        });
        assert!(most_at_sea >= 3, "only {most_at_sea} merchants put to sea");
    }

    #[test]
    fn player_can_cast_off_from_port_royal_and_dock_at_kingston() {
        let (mut app, ship) = new_game();
        assert!(
            voyage(&mut app, ship, "Kingston", 3.0),
            "never reached Kingston"
        );
    }
}
