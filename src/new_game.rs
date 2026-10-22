//! Starting a new game: load the opening map, and the opening world on it.
//!
//! The opening world is data, a save made 18 hours into a fresh world,
//! so the player joins a sea already busy: merchants under way, raiders on
//! their lanes, and patrols on their beats. Three groups have an errand
//! waiting for her, to introduce themselves. The fresh world it
//! grows from is built in [`fresh`], and the `write_the_opening` test
//! writes it. Run `cargo test write_the_opening -- --ignored` to make it
//! again when the map, the fleets, the player's start, or the save format
//! change.

use bevy::prelude::*;

use crate::goods::Goods;
use crate::map::data::{MapData, WEST_INDIES};
use crate::map::{PlayerStart, load_map};
use crate::save::{SaveGame, restore};

/// The opening world.
const OPENING: &str = include_str!("../assets/data/opening.ron");

pub struct NewGamePlugin;

impl Plugin for NewGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_map, load_opening).chain());
    }
}

fn spawn_map(goods: Res<Goods>, mut commands: Commands) {
    open_map(&goods, &mut commands);
}

/// Spawns the opening map, and says where the player's ship starts on it.
fn open_map(goods: &Goods, commands: &mut Commands) -> PlayerStart {
    let map = MapData::parse(WEST_INDIES).expect("the embedded map is validated by map tests");
    load_map(commands, &map, goods).expect("the embedded map is validated by map tests")
}

/// Puts the opening world's ships, contracts, and officers on the map.
fn load_opening(world: &mut World) {
    const STALE: &str =
        "the opening is out of date: run `cargo test write_the_opening -- --ignored`";
    let opening = SaveGame::from_ron(OPENING).expect(STALE);
    restore(world, &opening).expect(STALE);
}

#[cfg(test)]
pub(crate) use fresh::{calm_seas, new_game};

/// A fresh world, as it is before it starts running: the player's sloop,
/// every fleet in port, and the errands for a new captain. The opening
/// grows from it, and tests start from it.
#[cfg(test)]
mod fresh {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use super::open_map;
    use crate::alert::InSight;
    use crate::cargo::Purse;
    use crate::clock::DAY;
    use crate::economy::jobs::{Job, payment};
    use crate::factions::{CaptainRecord, Colours, Group, GroupKind, Nation, Relations};
    use crate::goods::Goods;
    use crate::map::PlayerStart;
    use crate::merchants::{harbours, merchant_fleet};
    use crate::patrols::patrol_squadron;
    use crate::player::Player;
    use crate::port::{Docked, Port};
    use crate::raiders::raiding_party;
    use crate::sailing::Heading;
    use crate::ship_class::ShipClass;

    /// How long the world has been running when the player joins it:
    /// long enough for every fleet to be at sea, and not so long that the
    /// raiders have thinned the merchants.
    pub const OPENING_HOURS: f64 = 18.0;
    /// The player's opening stake, in pieces of eight.
    pub const STARTING_PURSE: u32 = 500;
    /// Enough men to work a sloop, with a few to spare.
    const STARTING_CREW: u32 = 12;
    /// About a month's provisions for the starting crew.
    const STARTING_PROVISIONS: f64 = 2.0;
    /// One errand from a group of each nation, to introduce it to a new
    /// captain: who asks, and how many of what to carry from where to where.
    /// The first is in Port Royal, where she starts, and leads her to the
    /// others in Kingston.
    const ERRANDS: [(Group, &str, u32, &str, &str); 3] = [
        (
            Group {
                nation: Nation::ENGLAND,
                kind: GroupKind::Governor,
            },
            "soldiers",
            10,
            "Port Royal",
            "Kingston",
        ),
        (
            Group {
                nation: Nation::SPAIN,
                kind: GroupKind::Merchants,
            },
            "manufactures",
            10,
            "Kingston",
            "Santiago de Cuba",
        ),
        (
            Group {
                nation: Nation::PIRATES,
                kind: GroupKind::Navy,
            },
            "munitions",
            8,
            "Kingston",
            "Nassau",
        ),
    ];
    /// A new captain has this long to run them, from the world's beginning.
    const ERRAND_DAYS: f64 = 60.0;

    /// Posts the errands for a new captain.
    fn post_errands(
        goods: Res<Goods>,
        ports: Query<(Entity, &Name, &Transform), With<Port>>,
        mut commands: Commands,
    ) {
        let port = |name: &str| {
            ports
                .iter()
                .find(|(_, port, _)| port.as_str() == name)
                .map(|(entity, _, at)| (entity, at.translation))
                .expect("the opening map has the errands' ports")
        };
        for (group, good, quantity, from, to) in ERRANDS {
            let ((origin, from), (destination, to)) = (port(from), port(to));
            let good = goods.id(good).expect("the errands carry standard goods");
            commands.spawn(Job {
                good,
                quantity,
                origin,
                destination,
                payment: payment(goods.carriage(good), quantity, from.distance(to)),
                deadline: ERRAND_DAYS * DAY,
                introduction: Some(group),
            });
        }
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
        let class = ShipClass::sloop();
        let mut hold = class.empty_hold();
        hold.owned.add(goods.provisions, STARTING_PROVISIONS);
        (
            Name::new("Player Sloop"),
            Player,
            class.build(STARTING_CREW, class.guns, hold),
            Purse(STARTING_PURSE),
            Colours(Nation::ENGLAND),
            CaptainRecord::default(),
            InSight::default(),
            Heading(start.heading),
            Transform::from_translation(start.position),
            Docked { port: start.port },
        )
    }

    /// A headless simulation of the fresh world on the real map. Returns
    /// the player's ship.
    pub(crate) fn new_game() -> (App, Entity) {
        let mut app = crate::test_support::sim_app();
        let ship = app
            .world_mut()
            .run_system_once(|goods: Res<Goods>, mut commands: Commands| {
                let start = open_map(&goods, &mut commands);
                commands.spawn(player_ship(&goods, start)).id()
            })
            .unwrap();
        app.world_mut().run_system_once(launch_merchants).unwrap();
        app.world_mut().run_system_once(launch_patrols).unwrap();
        app.world_mut().run_system_once(launch_raiders).unwrap();
        app.world_mut().run_system_once(post_errands).unwrap();
        // Builds the navigation graph for the new land.
        app.update();
        (app, ship)
    }

    /// Clears the sea of raiders and patrols, for tests about the passage
    /// rather than who meets her on it.
    pub(crate) fn calm_seas(app: &mut App) {
        let hunters: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, Or<(With<crate::raiders::Raider>, With<crate::patrols::Patrol>)>>()
            .iter(app.world())
            .collect();
        for hunter in hunters {
            app.world_mut().despawn(hunter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fresh::{OPENING_HOURS, STARTING_PURSE};
    use super::*;
    use crate::alert::{Alert, AlertKind};
    use crate::cargo::Hold;
    use crate::cargo::Purse;
    use crate::economy::jobs::Job;
    use crate::economy::jobs::{AcceptJob, Carrier};
    use crate::factions::{Group, GroupKind, Nation, Standing};
    use crate::map::Land;
    use crate::navigation::route::{LAND_CLEARANCE, NavGraph};
    use crate::navigation::{Arrival, SailOrder};
    use crate::player::Player;
    use crate::port::Docked;
    use crate::port::{CastOff, Port};
    use crate::service::RANKS;
    use crate::ship_class::ShipClass;
    use crate::test_support::{run_checked, run_seconds, run_until};
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
            world.get::<crate::crew::Crew>(ship).unwrap().count
                >= ShipClass::sloop().complement.min
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
        let wanted = |destination, key: &str| {
            jobs.iter()
                .any(|(_, to, good)| *to == destination && good == key)
        };
        assert!(
            wanted(nassau, "provisions"),
            "from Kingston or Tortuga: {jobs:?}"
        );
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

    /// A fresh world, run [`OPENING_HOURS`] at full time compression.
    fn opening() -> SaveGame {
        let (mut app, _) = new_game();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        run_seconds(&mut app, (OPENING_HOURS * crate::clock::HOUR) as f32);
        crate::save::capture(app.world_mut())
    }

    #[test]
    #[ignore = "writes assets/data/opening.ron; run by hand to make the opening again"]
    fn write_the_opening() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/data/opening.ron");
        std::fs::write(path, opening().to_ron()).unwrap();
    }

    #[test]
    fn a_new_game_opens_on_a_busy_sea() {
        let (mut app, _) = new_game();
        load_opening(app.world_mut());
        app.update();
        let world = app.world_mut();
        let player = world
            .query_filtered::<&Docked, With<Player>>()
            .single(world)
            .ok()
            .copied();
        let (port_royal, _) = port(&mut app, "Port Royal");
        assert_eq!(player, Some(Docked { port: port_royal }));
        let at_sea = app
            .world_mut()
            .query_filtered::<(), (With<ShipClass>, Without<Docked>)>()
            .iter(app.world())
            .count();
        assert!(at_sea >= 10, "only {at_sea} ships at sea");
        let waiting = app
            .world_mut()
            .query_filtered::<&Job, Without<Carrier>>()
            .iter(app.world())
            .filter(|job| !job.for_anyone())
            .count();
        assert_eq!(waiting, 3, "the merchants left the errands for her");
    }

    #[test]
    fn running_the_governors_errand_introduces_her_to_him() {
        let (mut app, ship) = new_game();
        calm_seas(&mut app);
        let governor = Group {
            nation: Nation::ENGLAND,
            kind: GroupKind::Governor,
        };
        let (port_royal, _) = port(&mut app, "Port Royal");
        let errand = app
            .world_mut()
            .query::<(Entity, &Job)>()
            .iter(app.world())
            .find(|(_, job)| job.introduction == Some(governor) && job.origin == port_royal)
            .map(|(entity, _)| entity)
            .unwrap();
        app.world_mut()
            .write_message(AcceptJob { ship, job: errand });
        app.update();
        let mut introduced = false;
        let mut check = |world: &World| {
            introduced |= world
                .resource::<Messages<Alert>>()
                .iter_current_update_messages()
                .any(|alert| alert.kind == AlertKind::Introduced { group: governor });
        };
        let (kingston, anchorage) = port(&mut app, "Kingston");
        app.world_mut().write_message(CastOff { ship });
        app.world_mut().write_message(SailOrder {
            ship,
            destination: anchorage,
            on_arrival: Arrival::Dock(kingston),
        });
        run_checked(&mut app, 3.0 * crate::clock::HOUR as f32, &mut check);
        assert!(introduced, "never introduced");
        let standing = app.world().get::<Standing>(ship).unwrap();
        assert!(standing.of(governor) >= RANKS[0], "{standing:?}");
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
