//! Seamanship: what every captain does to keep her ship fit and out of
//! harm, whatever her business at sea.
//!
//! A merchantman, a raider, and a man-of-war go about different business
//! (see [`crate::merchants`], [`crate::raiders`], [`crate::patrols`]), but
//! they're all ships, and the same seamanship keeps each of them afloat.
//! It takes the helm from her business whenever it must, and hands it back
//! once she's fit and safe:
//!
//! - **Refitting.** A ship too battered to fight, or short of the hands to
//!   work her, puts in at the nearest yard that will have her: her own
//!   nation's, for a navy's, or any that welcomes her colours. The yard
//!   makes her good ([`crate::shipyard`]), and she signs on men: a navy
//!   mans her to the full at its charge, and any other captain hires the
//!   hands she needs from the tavern, out of her purse.
//! - **Running.** A ship with no navy's guns behind her runs from a warship
//!   that would hunt her, out of sight, before she goes on her way.
//!
//! What differs between ships is who keeps them, their [`Upkeep`], set by
//! their business. The player's ship and prizes aren't captained here.

use bevy::ecs::query::QueryData;
use bevy::math::FloatOrd;
use bevy::prelude::*;

use crate::allegiance::Favour;
use crate::cargo::Purse;
use crate::clock::{HOUR, WorldClock};
use crate::combat::tactics::unfit;
use crate::combat::{Engaged, Struck, Timbers};
use crate::crew::{Complement, Crew};
use crate::economy::market::{Market, Side, Trade, affordable};
use crate::factions::{Colours, Relations};
use crate::goods::Goods;
use crate::hunting::{Sea, evade};
use crate::map::Land;
use crate::navigation::{Arrival, Route, SailOrder};
use crate::player::Player;
use crate::port::{Docked, Port, welcomes};
use crate::prizes::Prize;
use crate::sailing::Rigging;
use crate::schedule::SimSet;
use crate::shipyard::has_shipyard;

/// A refitting ship tries the tavern for hands this often.
const HIRING_EVERY: f64 = HOUR;

/// Who keeps a ship, and so where and how she's refitted. Every captained
/// ship has one, required by her business.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upkeep {
    /// A navy's: refitted at her nation's yards and manned to the full, at
    /// its charge. She stands and fights rather than run.
    Navy,
    /// Her captain's own: refitted at any yard that welcomes her colours,
    /// hiring hands out of her purse.
    Purse,
}

/// Seamanship has the helm: she's bound for `yard`, or lying there, to be
/// made good. Her business waits.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Refitting {
    pub yard: Entity,
    /// When she last tried the tavern for hands.
    hired_at: Option<f64>,
}

/// Seamanship has the helm: she's running from a warship. Her business
/// waits, though it may change its plans for it.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Running;

/// A captained ship, as seamanship sees her.
#[derive(QueryData)]
struct Ship {
    entity: Entity,
    upkeep: &'static Upkeep,
    colours: &'static Colours,
    at: &'static Transform,
    crew: &'static Crew,
    complement: &'static Complement,
    timbers: &'static Timbers,
    rigging: &'static Rigging,
    docked: Option<&'static Docked>,
    route: Option<&'static Route>,
    running: Has<Running>,
}

impl ShipItem<'_, '_> {
    fn unfit(&self) -> bool {
        unfit(
            self.timbers.0,
            self.rigging.0,
            self.crew.count,
            self.complement.min,
        )
    }

    /// The hands she wants signed on: a navy's to the full, any other to
    /// work her. Her business may want more, and hire them itself.
    fn wants_hands(&self) -> u32 {
        let hands = match self.upkeep {
            Upkeep::Navy => self.complement.full,
            Upkeep::Purse => self.complement.min,
        };
        hands.saturating_sub(self.crew.count)
    }
}

/// A ship whose business has the helm: not in battle, and not being
/// refitted. She may be running, and her business may change its plans for
/// that, but leaves the steering to her seamanship.
pub type AtHerBusiness = (Without<Engaged>, Without<Struck>, Without<Refitting>);

/// A ship lying in port with her seamanship done with her.
pub type ReadyInPort = (With<Docked>, Without<Refitting>);

/// Ships captained here: not the player's, not prizes, and not while a
/// battle has the helm.
type Captained = (
    Without<Player>,
    Without<Prize>,
    Without<Engaged>,
    Without<Struck>,
);

pub struct SeamanshipPlugin;

impl Plugin for SeamanshipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (put_in_to_refit, make_for_the_yard, refit, run_from_warships)
                .chain()
                .in_set(SimSet::Seamanship),
        );
    }
}

/// Whether `port` would refit a ship kept by `upkeep`, under `colours`.
fn refits(
    port: &Port,
    favour: &Favour,
    upkeep: Upkeep,
    colours: Colours,
    relations: &Relations,
) -> bool {
    has_shipyard(port)
        && match upkeep {
            Upkeep::Navy => port.flag == colours.0,
            Upkeep::Purse => welcomes(port, favour, colours.0, relations),
        }
}

/// An unfit ship puts in at the nearest yard that will refit her: where she
/// lies, if it will.
fn put_in_to_refit(
    relations: Res<Relations>,
    ports: Query<(Entity, &Port, &Favour, &Transform)>,
    ships: Query<Ship, (Captained, Without<Refitting>)>,
    mut commands: Commands,
) {
    for ship in &ships {
        if !ship.unfit() {
            continue;
        }
        let will_refit = |(_, port, favour, _): &(Entity, &Port, &Favour, &Transform)| {
            refits(port, favour, *ship.upkeep, *ship.colours, &relations)
        };
        let here = ship
            .docked
            .and_then(|docked| ports.get(docked.port).ok())
            .filter(will_refit);
        let yard = here.or_else(|| {
            ports
                .iter()
                .filter(will_refit)
                .min_by_key(|(.., anchorage)| {
                    FloatOrd(anchorage.translation.distance(ship.at.translation))
                })
        });
        if let Some((yard, ..)) = yard {
            commands.entity(ship.entity).insert(Refitting {
                yard,
                hired_at: None,
            });
        }
    }
}

/// A refitting ship sails for her yard, unless it won't have her now.
fn make_for_the_yard(
    relations: Res<Relations>,
    ports: Query<(&Port, &Favour, &Transform)>,
    ships: Query<(Ship, &Refitting), (Captained, Without<Docked>)>,
    mut orders: MessageWriter<SailOrder>,
    mut commands: Commands,
) {
    for (ship, refitting) in &ships {
        let yard = refitting.yard;
        let Some((.., anchorage)) = ports.get(yard).ok().filter(|(port, favour, _)| {
            refits(port, favour, *ship.upkeep, *ship.colours, &relations)
        }) else {
            // It's changed hands, or gone: she'll find another.
            commands.entity(ship.entity).remove::<Refitting>();
            continue;
        };
        if ship
            .route
            .is_none_or(|route| route.on_arrival != Arrival::Dock(yard))
        {
            orders.write(SailOrder {
                ship: ship.entity,
                destination: anchorage.translation,
                on_arrival: Arrival::Dock(yard),
            });
        }
    }
}

/// A refitting ship in port signs on hands while the yard works, and is
/// handed back to her business once she's fit.
fn refit(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    markets: Query<&Market>,
    mut ships: Query<(Ship, &mut Refitting, &Docked, &Purse), Captained>,
    mut trades: MessageWriter<Trade>,
    mut commands: Commands,
) {
    let now = clock.elapsed;
    for (ship, mut refitting, docked, purse) in &mut ships {
        let wanted = ship.wants_hands();
        if !ship.unfit() && wanted == 0 {
            commands.entity(ship.entity).remove::<Refitting>();
            continue;
        }
        if wanted == 0 || refitting.hired_at.is_some_and(|at| now - at < HIRING_EVERY) {
            continue;
        }
        refitting.hired_at = Some(now);
        match ship.upkeep {
            Upkeep::Navy => {
                let mut crew = *ship.crew;
                crew.count += wanted;
                commands.entity(ship.entity).insert(crew);
            }
            Upkeep::Purse => {
                let base_price = goods.get(goods.sailors).base_price;
                let hired = markets
                    .get(docked.port)
                    .ok()
                    .and_then(|market| market.entry(goods.sailors))
                    .map_or(0, |entry| affordable(entry, base_price, wanted, purse.0));
                if hired > 0 {
                    trades.write(Trade {
                        ship: ship.entity,
                        good: goods.sailors,
                        quantity: hired,
                        side: Side::Buy,
                    });
                }
            }
        }
    }
}

/// A ship with no navy's guns behind her runs from a warship that would
/// hunt her, straight away or as near it as open water allows, until she's
/// out of sight or safe in port.
fn run_from_warships(
    land: Res<Land>,
    sea: Sea,
    ships: Query<Ship, Captained>,
    mut orders: MessageWriter<SailOrder>,
    mut commands: Commands,
) {
    for ship in &ships {
        if *ship.upkeep == Upkeep::Navy {
            continue;
        }
        let at_sea = ship.docked.is_none();
        let hunter = sea.nearest_hunter(ship.entity).filter(|_| at_sea);
        let Some(hunter) = hunter else {
            if ship.running {
                commands.entity(ship.entity).remove::<Running>();
            }
            continue;
        };
        if !ship.running {
            commands.entity(ship.entity).insert(Running);
        }
        let heading_for = ship.route.and_then(|route| route.waypoints.back().copied());
        if let Some(away) = evade(&land, ship.at.translation, heading_for, hunter) {
            orders.write(SailOrder {
                ship: ship.entity,
                destination: away,
                on_arrival: Arrival::HeaveTo,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::DAY;
    use crate::factions::{Nation, WAR};
    use crate::merchants::Merchant;
    use crate::new_game::new_game;
    use crate::patrols::Patrol;
    use crate::raiders::Raider;
    use crate::test_support::run_until;
    use crate::time_scale::MAX_STEP;

    #[test]
    fn a_navy_refits_at_its_own_yards_and_anyone_else_where_she_is_welcome() {
        let relations = Relations::default();
        let yard = |flag| Port {
            flag,
            town: Vec3::ZERO,
            locations: vec![crate::port::Location::Shipyard],
        };
        let favour = Favour::held_by(Nation::ENGLAND);
        let english = Colours(Nation::ENGLAND);
        let pirate = Colours(Nation::PIRATES);
        let spanish = Colours(Nation::SPAIN);
        let port_royal = yard(Nation::ENGLAND);
        assert!(refits(
            &port_royal,
            &favour,
            Upkeep::Navy,
            english,
            &relations
        ));
        assert!(!refits(
            &port_royal,
            &favour,
            Upkeep::Navy,
            spanish,
            &relations
        ));
        assert!(
            refits(&port_royal, &favour, Upkeep::Purse, pirate, &relations),
            "not at war yet"
        );
        let santiago = yard(Nation::SPAIN);
        assert!(
            !refits(&santiago, &favour, Upkeep::Purse, pirate, &relations),
            "at war"
        );
        let no_yard = Port {
            locations: vec![crate::port::Location::Market],
            ..yard(Nation::ENGLAND)
        };
        assert!(!refits(
            &no_yard,
            &favour,
            Upkeep::Purse,
            english,
            &relations
        ));
    }

    /// A new world at full time compression, a moment in.
    fn world() -> App {
        let (mut app, _) = new_game();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        app.update();
        app
    }

    /// The first ship of each business: a merchant, a raider, a patrol.
    fn one_of_each(app: &mut App) -> [Entity; 3] {
        let first = |app: &mut App, has: fn(&EntityRef) -> bool| {
            app.world()
                .iter_entities()
                .filter(|ship| has(ship))
                .map(|ship| ship.id())
                .min()
                .unwrap()
        };
        [
            first(app, |ship| ship.contains::<Merchant>()),
            first(app, |ship| ship.contains::<Raider>()),
            first(app, |ship| ship.contains::<Patrol>()),
        ]
    }

    #[test]
    fn every_ship_whatever_her_business_is_made_good_when_she_is_unfit() {
        let mut app = world();
        let ships = one_of_each(&mut app);
        for ship in ships {
            let min = app.world().get::<Complement>(ship).unwrap().min;
            app.world_mut()
                .entity_mut(ship)
                .insert((Timbers(0.05), Crew::new(min - 1)));
        }
        app.update();
        for ship in ships {
            assert!(
                app.world().get::<Refitting>(ship).is_some(),
                "{ship} put in"
            );
        }
        let all_fit = run_until(&mut app, (5.0 * DAY) as f32, |world| {
            ships
                .iter()
                .all(|&ship| world.get::<Refitting>(ship).is_none())
        });
        assert!(all_fit, "not all made good");
        for ship in ships {
            let world = app.world();
            let (timbers, crew) = (
                world.get::<Timbers>(ship).unwrap(),
                world.get::<Crew>(ship).unwrap(),
            );
            let min = world.get::<Complement>(ship).unwrap().min;
            assert!(
                timbers.0 > 0.4 && crew.count >= min,
                "{ship}: {timbers:?} {crew:?}"
            );
        }
    }

    #[test]
    fn a_merchant_or_raider_runs_from_a_warship_that_would_hunt_her_and_a_patrol_does_not() {
        let mut app = world();
        app.world_mut()
            .resource_mut::<Relations>()
            .set(Nation::ENGLAND, Nation::SPAIN, WAR - 1.0);
        let [merchant, raider, patrol] = one_of_each(&mut app);
        app.world_mut().get_mut::<Colours>(patrol).unwrap().0 = Nation::ENGLAND;
        // Out at sea, each in sight of a man-of-war at war with her.
        let mut at = 0.0;
        for ship in [merchant, raider, patrol] {
            let enemy = match app.world().get::<Colours>(ship).unwrap().0 {
                Nation::SPAIN => Nation::ENGLAND,
                _ => Nation::SPAIN,
            };
            at += 50_000.0;
            let spot = Vec3::new(at, 0.0, 300_000.0);
            app.world_mut()
                .entity_mut(ship)
                .remove::<Docked>()
                .insert(Transform::from_translation(spot));
            app.world_mut().spawn((
                Patrol::bound_for(0),
                Colours(enemy),
                Transform::from_translation(spot + Vec3::new(3_000.0, 0.0, 0.0)),
            ));
        }
        app.update();
        app.update();
        let world = app.world();
        assert!(world.get::<Running>(merchant).is_some());
        assert!(world.get::<Running>(raider).is_some());
        assert!(
            world.get::<Running>(patrol).is_none(),
            "a man-of-war stands"
        );
    }
}
