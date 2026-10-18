//! Navy patrols: well-manned sloops-of-war cruising between the ports of
//! their beat. A patrol hunts her nation's enemies (see [`crate::hunting`]).
//! Patrols are victualled for free at their own nation's ports, and a lost
//! one is replaced.

use bevy::prelude::*;

use crate::cargo::{Hold, Purse};
use crate::clock::{DAY, WorldClock};
use crate::combat::{EngageOrder, Engaged, Struck};
use crate::factions::{Colours, NATIONS, Nation, Relations};
use crate::goods::Goods;
use crate::hunting::{Hunter, Pursuit, Sea, pursue};
use crate::merchants::{Harbour, harbours};
use crate::navigation::{Arrival, Route, SailOrder};
use crate::port::{Docked, Port};
use crate::roster::Roster;
use crate::schedule::SimSet;
use crate::ship_class::ShipClass;

/// The Royal Navy's Jamaica station, and a guarda costa out of Santiago.
const NAMES: [&str; 3] = ["HMS Swift", "HMS Ferret", "San Miguel"];
/// The nation each of [`NAMES`] sails for.
const FLAGS: [Nation; NAMES.len()] = [Nation::England, Nation::England, Nation::Spain];
/// A full fighting crew, every gun mounted.
const CREW: u32 = 40;
const GUNS: u32 = ShipClass::SLOOP.guns;
const PURSE: u32 = 100;
/// Tons of provisions carried, topped up at each home port.
const PROVISIONS: f64 = 20.0;
/// A lost patrol is replaced this long after.
const REPLACEMENT: f64 = 4.0 * DAY;
/// A patrol this close to a home port's anchorage, meters, is victualled.
const VICTUALLING: f32 = 200.0;

/// A navy ship on patrol, and the port on her beat she's bound for next.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patrol {
    pub next: usize,
    /// A sail order is given but not yet a route. Orders become routes once
    /// a frame, and several fixed steps may run before then.
    pending: bool,
}

impl Patrol {
    pub fn bound_for(next: usize) -> Self {
        Self {
            next,
            pending: false,
        }
    }
}

#[derive(Resource, Debug)]
struct Admiralty(Roster);

impl Default for Admiralty {
    fn default() -> Self {
        Self(Roster::new(&NAMES, REPLACEMENT))
    }
}

pub struct PatrolsPlugin;

impl Plugin for PatrolsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Admiralty>().add_systems(
            FixedUpdate,
            (patrol, commission_patrols).in_set(SimSet::Economy),
        );
    }
}

/// A patrol for `nation` named `name` at `at`, beginning her beat at port
/// `next`.
fn patrol_ship(goods: &Goods, name: &str, nation: Nation, at: Vec3, next: usize) -> impl Bundle {
    let class = ShipClass::SLOOP;
    let mut hold = class.empty_hold();
    hold.owned.add(goods.provisions, PROVISIONS);
    (
        Name::new(name.to_string()),
        Patrol::bound_for(next),
        Colours(nation),
        class.build(CREW, GUNS, hold),
        Purse(PURSE),
        Transform::from_translation(at),
    )
}

/// The ports on a `nation`'s patrols' beat, in a stable order: every port
/// but her enemies'.
pub fn beat(
    ports: &Query<(Entity, &Port, &Transform)>,
    relations: &Relations,
    nation: Nation,
) -> Vec<Harbour> {
    harbours(ports)
        .into_iter()
        .filter(|&(_, flag, _)| !relations.at_war(nation, flag))
        .collect()
}

/// Where the `index`th patrol of `nation` starts on `beat`, and the place
/// on the beat she's bound for next: at one of her nation's own ports. A
/// nation with no port has no navy.
fn station(beat: &[Harbour], nation: Nation, index: usize) -> Option<(Vec3, usize)> {
    let home: Vec<usize> = (0..beat.len()).filter(|&i| beat[i].1 == nation).collect();
    let here = *home.get(index % home.len().max(1))?;
    Some((beat[here].2, (here + 1) % beat.len()))
}

/// The patrols at the start of a game, each at a port of her own nation.
pub fn patrol_squadron(
    goods: &Goods,
    ports: &Query<(Entity, &Port, &Transform)>,
    relations: &Relations,
) -> Vec<impl Bundle> {
    NAMES
        .iter()
        .zip(FLAGS)
        .enumerate()
        .filter_map(|(i, (name, nation))| {
            let (at, next) = station(&beat(ports, relations, nation), nation, i)?;
            Some(patrol_ship(goods, name, nation, at, next))
        })
        .collect()
}

/// Hunts her nation's enemies in sight: gives chase, and engages at close
/// range. Otherwise cruises the beat, victualling at home ports.
fn patrol(
    goods: Res<Goods>,
    relations: Res<Relations>,
    ports: Query<(Entity, &Port, &Transform)>,
    mut patrols: Query<
        (
            Entity,
            &mut Patrol,
            &Colours,
            &Transform,
            &mut Hold,
            Option<&Route>,
        ),
        (Without<Engaged>, Without<Struck>, Without<Docked>),
    >,
    sea: Sea,
    mut engagements: MessageWriter<EngageOrder>,
    mut orders: MessageWriter<SailOrder>,
) {
    let beats = NATIONS.map(|nation| beat(&ports, &relations, nation));
    for (ship, mut patrol, &Colours(nation), at, mut hold, route) in &mut patrols {
        let beat = &beats[nation.index()];
        if let Some((target, quarry)) = sea.nearest_quarry(Hunter::Navy(nation), at.translation) {
            let heading_for = route.and_then(|route| route.waypoints.back().copied());
            match pursue(at.translation, heading_for, quarry) {
                Pursuit::Engage => {
                    engagements.write(EngageOrder { ship, target });
                }
                Pursuit::SailTo(quarry) => {
                    orders.write(SailOrder {
                        ship,
                        destination: quarry,
                        on_arrival: Arrival::HeaveTo,
                    });
                }
                Pursuit::Hold => {}
            }
            continue;
        }
        if beat.is_empty() {
            continue;
        }
        if route.is_some() {
            patrol.pending = false;
            continue;
        }
        if patrol.pending {
            continue;
        }
        // On station: victual if this is home, then on to the next port.
        let here = (patrol.next + beat.len() - 1) % beat.len();
        if beat[here].1 == nation && at.translation.distance(beat[here].2) < VICTUALLING {
            let short = PROVISIONS - hold.owned.get(goods.provisions);
            hold.owned.add(goods.provisions, short.max(0.0));
        }
        let (_, _, station) = beat[patrol.next % beat.len()];
        orders.write(SailOrder {
            ship,
            destination: station,
            on_arrival: Arrival::HeaveTo,
        });
        patrol.next = (patrol.next + 1) % beat.len();
        patrol.pending = true;
    }
}

fn commission_patrols(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    relations: Res<Relations>,
    ships: Query<&Name, With<ShipClass>>,
    ports: Query<(Entity, &Port, &Transform)>,
    mut admiralty: ResMut<Admiralty>,
    mut commands: Commands,
) {
    let Some((index, name)) = admiralty.0.due(ships.iter(), clock.elapsed) else {
        return;
    };
    let nation = FLAGS[index];
    if let Some((at, next)) = station(&beat(&ports, &relations, nation), nation, index) {
        commands.spawn(patrol_ship(&goods, name, nation, at, next));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::{Alert, AlertKind};
    use crate::factions::Standing;
    use crate::map::Land;
    use crate::new_game::new_game;
    use crate::port::CastOff;
    use crate::test_support::{run_checked, run_until};
    use crate::time_scale::MAX_STEP;

    fn fast(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
    }

    fn patrols(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<Patrol>>()
            .iter(app.world())
            .collect()
    }

    fn named(app: &mut App, name: &str) -> Entity {
        app.world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, n)| n.as_str() == name)
            .unwrap()
            .0
    }

    #[test]
    fn patrols_cruise_the_beat_without_running_aground() {
        let (mut app, _) = new_game();
        fast(&mut app);
        let ships = patrols(&mut app);
        assert_eq!(ships.len(), NAMES.len());
        let start: Vec<Vec3> = ships
            .iter()
            .map(|&ship| app.world().get::<Transform>(ship).unwrap().translation)
            .collect();
        run_checked(&mut app, DAY as f32, |world| {
            for &ship in &ships {
                let at = world.get::<Transform>(ship).unwrap().translation;
                assert!(!world.resource::<Land>().contains(at), "aground at {at}");
            }
        });
        for (&ship, start) in ships.iter().zip(start) {
            let moved = app
                .world()
                .get::<Transform>(ship)
                .unwrap()
                .translation
                .distance(start);
            assert!(moved > 1000.0, "only moved {moved:.0} m");
        }
    }

    /// Puts an English patrol and the player in open water south of Port
    /// Royal, 1.5 km apart.
    fn near_a_patrol(app: &mut App, player: Entity) -> Entity {
        let patrol = named(app, NAMES[0]);
        let port_royal = named(app, "Port Royal");
        let offshore = app
            .world()
            .get::<Transform>(port_royal)
            .unwrap()
            .translation
            + Vec3::new(0.0, 0.0, 9000.0);
        app.world_mut().write_message(CastOff { ship: player });
        app.update();
        for (ship, at) in [(patrol, offshore), (player, offshore + Vec3::X * 1500.0)] {
            assert!(!app.world().resource::<Land>().contains(at));
            app.world_mut()
                .get_mut::<Transform>(ship)
                .unwrap()
                .translation = at;
            app.world_mut().entity_mut(ship).remove::<Route>();
        }
        patrol
    }

    #[test]
    fn a_patrol_attacks_a_wanted_captain_and_leaves_an_honest_one_alone() {
        let (mut app, player) = new_game();
        fast(&mut app);
        let patrol = near_a_patrol(&mut app, player);
        run_until(&mut app, 600.0, |_| false);
        assert!(
            app.world().get::<Engaged>(patrol).is_none(),
            "attacked an honest captain"
        );

        app.world_mut()
            .resource_mut::<Standing>()
            .set(Nation::England, -50.0);
        let attacked = run_until(&mut app, 3600.0, |world| {
            world
                .get::<Engaged>(patrol)
                .is_some_and(|e| e.enemy == player)
        });
        assert!(attacked, "let a wanted captain go");
    }

    #[test]
    fn a_patrol_taken_as_a_prize_stops_patrolling() {
        let (mut app, player) = new_game();
        fast(&mut app);
        let patrol = near_a_patrol(&mut app, player);
        let alongside = app.world().get::<Transform>(patrol).unwrap().translation + Vec3::X * 30.0;
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = alongside;
        app.world_mut()
            .get_mut::<crate::crew::Crew>(player)
            .unwrap()
            .count = 75;
        app.world_mut().entity_mut(patrol).insert(Struck);
        app.world_mut().entity_mut(player).insert(Engaged {
            enemy: patrol,
            aggressor: true,
        });
        app.world_mut()
            .write_message(crate::prizes::BoardOrder { ship: player });
        app.update();
        let world = app.world();
        assert!(
            world.get::<crate::prizes::Prize>(patrol).is_some(),
            "not taken"
        );
        assert!(world.get::<Patrol>(patrol).is_none(), "still on patrol");
    }

    #[test]
    fn a_wanted_captain_is_turned_away_from_her_nations_ports() {
        let (mut app, player) = new_game();
        fast(&mut app);
        // No patrols: this is about the port, not the navy.
        for patrol in patrols(&mut app) {
            app.world_mut().despawn(patrol);
        }
        app.world_mut()
            .resource_mut::<Standing>()
            .set(Nation::England, -50.0);
        app.world_mut().write_message(CastOff { ship: player });
        let kingston = named(&mut app, "Kingston");
        let anchorage = app.world().get::<Transform>(kingston).unwrap().translation;
        app.world_mut().write_message(SailOrder {
            ship: player,
            destination: anchorage,
            on_arrival: Arrival::Dock(kingston),
        });
        let mut refused = false;
        run_checked(&mut app, 3.0 * 3600.0, |world| {
            refused |= world
                .resource::<Messages<Alert>>()
                .iter_current_update_messages()
                .any(|alert| {
                    alert.ship == player
                        && alert.kind
                            == AlertKind::TurnedAway {
                                port: kingston,
                                why: crate::port::Refused::Wanted,
                            }
                });
        });
        assert!(refused, "never turned away");
        assert!(app.world().get::<Docked>(player).is_none());
    }
}
