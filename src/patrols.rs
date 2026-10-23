//! Navy patrols: well-manned sloops-of-war of the lawful nations' navies
//! (from the factions data), cruising between the ports of their beat. A patrol hunts her nation's enemies (see [`crate::hunting`]).
//! Patrols are victualled for free at their own nation's ports, and a lost
//! one is replaced. When a nation hears of one of its ships wrecked in
//! shallow water, its nearest patrol stands guard over her, salvaging what
//! she can for her home port, and hunts whoever else dives on her.

use bevy::ecs::query::QueryData;
use bevy::prelude::*;

use crate::cargo::{Hold, Purse};
use crate::clock::{DAY, WorldClock};
use crate::combat::EngageOrder;
use crate::crew::Crew;
use crate::economy::market::Market;
use crate::factions::{Colours, GroupKind, Nation, Relations, Serves, nations};
use crate::goods::Goods;
use crate::hunting::{Chase, Hunter, Pursuit, Sea, pursue};
use crate::merchants::{Harbour, harbours};
use crate::navigation::{Arrival, Route, SailOrder};
use crate::port::{CastOff, Docked, Port};
use crate::roster::{Listed, Roster};
use crate::schedule::SimSet;
use crate::seamanship::{AtHerBusiness, ReadyInPort, Upkeep};
use crate::ship_class::ShipClass;
use crate::word::Happening;
use crate::wrecks::{SALVAGE_RANGE, WreckTalk};

const PURSE: u32 = 100;
/// Tons of provisions carried, topped up at each home port.
const PROVISIONS: f64 = 20.0;
/// A lost patrol is replaced this long after.
const REPLACEMENT: f64 = 4.0 * DAY;
/// A patrol this close to a home port's anchorage, meters, is victualled.
const VICTUALLING: f32 = 200.0;

/// A navy ship on patrol, and the port on her beat she's bound for next.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[require(Serves(GroupKind::Navy), Chase, Upkeep = Upkeep::Navy)]
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
    /// Every lawful nation's navy, from the factions data.
    fn default() -> Self {
        Self(Roster::of_fleets(
            |nation| {
                if nation.is_crown() {
                    &nation.fleet().navy
                } else {
                    &[]
                }
            },
            REPLACEMENT,
        ))
    }
}

pub struct PatrolsPlugin;

impl Plugin for PatrolsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Admiralty>().add_systems(
            FixedUpdate,
            (patrol, put_to_sea, commission_patrols).in_set(SimSet::Economy),
        );
    }
}

/// A patrol from the roster at `at`, beginning her beat at port `next`: a
/// full fighting crew, and every gun mounted.
fn patrol_ship(goods: &Goods, listed: Listed, at: Vec3, next: usize) -> impl Bundle {
    let class = listed.class;
    let mut hold = class.empty_hold();
    hold.owned.add(goods.provisions, PROVISIONS);
    (
        Name::new(listed.name),
        Patrol::bound_for(next),
        Colours(listed.nation),
        class.build(class.complement.full, class.guns, hold),
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
    Admiralty::default()
        .0
        .ships
        .into_iter()
        .enumerate()
        .filter_map(|(i, listed)| {
            let nation = listed.nation;
            let (at, next) = station(&beat(ports, relations, nation), nation, i)?;
            Some(patrol_ship(goods, listed, at, next))
        })
        .collect()
}

/// A patrol, as her captain sees her.
#[derive(QueryData)]
#[query_data(mutable)]
struct PatrolShip {
    entity: Entity,
    patrol: &'static mut Patrol,
    colours: &'static Colours,
    at: &'static Transform,
    hold: &'static mut Hold,
    route: Option<&'static Route>,
    crew: &'static Crew,
    chase: &'static mut Chase,
}

/// A patrol only lies in port to be made good: once her seamanship is
/// done with her, she puts to sea and takes up her beat afresh.
fn put_to_sea(
    mut patrols: Query<(Entity, &mut Patrol), ReadyInPort>,
    mut cast_offs: MessageWriter<CastOff>,
) {
    for (ship, mut patrol) in &mut patrols {
        patrol.pending = false;
        cast_offs.write(CastOff { ship });
    }
}

/// Hunts her nation's enemies in sight: gives chase, and engages at close
/// range. Otherwise cruises the beat, victualling at home ports.
fn patrol(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    relations: Res<Relations>,
    ports: Query<(Entity, &Port, &Transform)>,
    mut patrols: Query<PatrolShip, (AtHerBusiness, Without<Docked>)>,
    sea: Sea,
    talk: WreckTalk,
    mut markets: Query<&mut Market>,
    mut engagements: MessageWriter<EngageOrder>,
    mut orders: MessageWriter<SailOrder>,
) {
    let beats: Vec<Vec<Harbour>> = nations()
        .map(|nation| beat(&ports, &relations, nation))
        .collect();
    let positions: Vec<(Entity, Nation, Vec3)> = patrols
        .iter()
        .map(|ship| (ship.entity, ship.colours.0, ship.at.translation))
        .collect();
    let guards = guard_duty(&talk, &positions);
    for mut ship in &mut patrols {
        let (Colours(nation), at, route) = (*ship.colours, ship.at, ship.route);
        let beat = &beats[nation.index()];
        let now = clock.elapsed;
        let quarry = sea.nearest_quarry(
            Hunter::Navy(nation),
            at.translation,
            ship.chase.letting_be(now),
        );
        if ship
            .chase
            .keep_after(quarry.map(|(target, _)| target), now, ship.crew.morale)
            && let Some((target, quarry)) = quarry
        {
            let heading_for = route.and_then(|route| route.waypoints.back().copied());
            match pursue(at.translation, heading_for, quarry) {
                Pursuit::Engage => {
                    engagements.write(EngageOrder {
                        ship: ship.entity,
                        target,
                    });
                }
                Pursuit::SailTo(quarry) => {
                    orders.write(SailOrder {
                        ship: ship.entity,
                        destination: quarry,
                        on_arrival: Arrival::HeaveTo,
                    });
                }
                Pursuit::Hold => {}
            }
            continue;
        }
        if let Some(&(_, lies)) = guards.iter().find(|(guard, _)| *guard == ship.entity) {
            let heading_for = route.and_then(|route| route.waypoints.back().copied());
            let closing = heading_for.is_some_and(|end| end.distance(lies) <= SALVAGE_RANGE);
            if at.translation.distance(lies) > SALVAGE_RANGE && !closing {
                orders.write(SailOrder {
                    ship: ship.entity,
                    destination: lies,
                    on_arrival: Arrival::HeaveTo,
                });
            }
            continue;
        }
        if beat.is_empty() {
            continue;
        }
        if route.is_some() {
            ship.patrol.pending = false;
            continue;
        }
        if ship.patrol.pending {
            continue;
        }
        // On station: victual if this is home, then on to the next port.
        let here = (ship.patrol.next + beat.len() - 1) % beat.len();
        if beat[here].1 == nation && at.translation.distance(beat[here].2) < VICTUALLING {
            let short = PROVISIONS - ship.hold.owned.get(goods.provisions);
            ship.hold.owned.add(goods.provisions, short.max(0.0));
            // What she salvaged goes ashore, to the port's stores.
            if let Ok(mut market) = markets.get_mut(beat[here].0) {
                land_salvage(&goods, &mut ship.hold, &mut market);
            }
        }
        let (_, _, station) = beat[ship.patrol.next % beat.len()];
        orders.write(SailOrder {
            ship: ship.entity,
            destination: station,
            on_arrival: Arrival::HeaveTo,
        });
        ship.patrol.next = (ship.patrol.next + 1) % beat.len();
        ship.patrol.pending = true;
    }
}

/// Which patrol guards which of her nation's wrecks: of every wreck worth
/// diving on that its nation has heard of, the patrol of that nation in
/// `patrols` nearest it.
fn guard_duty(talk: &WreckTalk, patrols: &[(Entity, Nation, Vec3)]) -> Vec<(Entity, Vec3)> {
    talk.ledger
        .news
        .iter()
        .filter_map(|news| match news.event {
            Happening::Wrecked {
                wreck: Some(wreck),
                nation: Some(nation),
                ..
            } if news.heard_by.contains(&nation) => Some((wreck, nation)),
            _ => None,
        })
        .filter_map(|(wreck, nation)| {
            let (_, lies, remains) = talk.wrecks.get(wreck).ok()?;
            let lies = lies.translation;
            let (guard, ..) = patrols
                .iter()
                .filter(|(_, flag, _)| *flag == nation)
                .filter(|_| remains.has_anything())
                .min_by(|a, b| a.2.distance(lies).total_cmp(&b.2.distance(lies)))?;
            Some((*guard, lies))
        })
        .collect()
}

/// Lands everything in `hold` but provisions into `market`, for the goods
/// it deals in.
fn land_salvage(goods: &Goods, hold: &mut Hold, market: &mut Market) {
    let cargo: Vec<_> = hold
        .owned
        .iter()
        .filter(|&(good, _)| good != goods.provisions)
        .collect();
    for (good, tons) in cargo {
        if let Some(entry) = market.entry_mut(good) {
            entry.stock += hold.owned.remove(good, tons);
        }
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
    let Some((index, listed)) = admiralty.0.due(ships.iter(), clock.elapsed) else {
        return;
    };
    let nation = listed.nation;
    if let Some((at, next)) = station(&beat(&ports, &relations, nation), nation, index) {
        commands.spawn(patrol_ship(&goods, listed, at, next));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::{Alert, AlertKind};
    use crate::combat::tactics::battered;
    use crate::combat::{Engaged, Struck, Timbers};
    use crate::crew::Complement;
    use crate::factions::Standing;
    use crate::map::Land;
    use crate::new_game::new_game;
    use crate::port::CastOff;
    use crate::shipyard::has_shipyard;
    use crate::test_support::{run_checked, run_until};
    use crate::time_scale::MAX_STEP;

    #[test]
    fn a_battered_patrol_is_made_good_at_a_home_yard_and_sails_again() {
        let (mut app, _) = new_game();
        fast(&mut app);
        app.update();
        let (ferret, complement) = app
            .world_mut()
            .query_filtered::<(Entity, &Name, &Complement), With<Patrol>>()
            .iter(app.world())
            .find(|(_, name, _)| name.as_str() == "HMS Ferret")
            .map(|(ship, _, complement)| (ship, *complement))
            .unwrap();
        app.world_mut()
            .entity_mut(ferret)
            .insert((Timbers(0.05), Crew::new(complement.min - 1)));
        let docked = run_until(&mut app, (3.0 * DAY) as f32, |world| {
            world.get::<Docked>(ferret).is_some()
        });
        assert!(docked, "she never made port");
        let port = app.world().get::<Docked>(ferret).unwrap().port;
        let port = app.world().get::<Port>(port).unwrap();
        assert!(port.flag == Nation::ENGLAND && has_shipyard(port));
        let sailed = run_until(&mut app, (3.0 * DAY) as f32, |world| {
            world.get::<Docked>(ferret).is_none()
        });
        assert!(sailed, "she never sailed again");
        let world = app.world();
        assert!(!battered(world.get::<Timbers>(ferret).unwrap().0, 1.0));
        assert_eq!(world.get::<Crew>(ferret).unwrap().count, complement.full);
    }

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
        assert_eq!(ships.len(), Admiralty::default().0.ships.len());
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
        let patrol = named(app, Admiralty::default().0.ships[0].name);
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
            .get_mut::<Standing>(player)
            .unwrap()
            .set(Nation::ENGLAND, -50.0);
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
            .get_mut::<Standing>(player)
            .unwrap()
            .set(Nation::ENGLAND, -50.0);
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
