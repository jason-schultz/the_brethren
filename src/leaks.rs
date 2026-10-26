//! Leaks: every wooden ship takes water, and her men at the pumps keep it
//! out.
//!
//! A sound ship only seeps, and shot and grounding open her seams wider:
//! the more of her timbers are gone, the faster the water comes in. Her
//! men pump as hard as they work her sails, so a ship with enough hands
//! holds a leak until she's badly battered, and one with too few loses
//! ground to it however sound she is. Water gaining on the pumps eats
//! into her timbers, and at nothing she founders. A ship in port is pumped
//! out by the harbour, and leaks no more.

use bevy::ecs::query::QueryData;
use bevy::prelude::*;

use crate::alert::{Alert, AlertKind};
use crate::clock::HOUR;
use crate::combat::{Engaged, Sunk, Timbers};
use crate::port::Docked;
use crate::sailing::SailHandling;
use crate::schedule::SimSet;
use crate::tuning::TUNING;
use crate::wrecks::{Cause, Founder};

/// The water is gaining on her pumps.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leaking;

/// Timbers lost in `hours` by a ship with `timbers` left, whose men work
/// her pumps as they work her sails: `pumping` of what a whole company
/// can.
pub fn water_gained(timbers: f32, pumping: f32, hours: f32) -> f32 {
    let inflow = TUNING.seep_per_hour + TUNING.leak_per_hour * (1.0 - timbers);
    let pumped = TUNING.pumps_per_hour * pumping.clamp(0.0, 1.0);
    (inflow - pumped).max(0.0) * hours
}

pub struct LeaksPlugin;

impl Plugin for LeaksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, take_on_water.in_set(SimSet::Resolve));
    }
}

/// A ship at sea and afloat.
type Afloat = (Without<Docked>, Without<Sunk>);

/// A ship taking water: her hull, her men at the pumps, and whether it's
/// gaining already, or she's in a fight.
#[derive(QueryData)]
#[query_data(mutable)]
struct Hull {
    entity: Entity,
    timbers: &'static mut Timbers,
    handling: &'static SailHandling,
    leaking: Has<Leaking>,
    engaged: Has<Engaged>,
}

/// The water comes in on every ship at sea, as fast as her pumps let it.
/// She's told when it starts to gain. Out of a fight, she founders when
/// her timbers are gone; in one, the fight decides who sank her.
fn take_on_water(
    time: Res<Time>,
    mut ships: Query<Hull, Afloat>,
    mut alerts: MessageWriter<Alert>,
    mut founders: MessageWriter<Founder>,
    mut commands: Commands,
) {
    let hours = (time.delta_secs_f64() / HOUR) as f32;
    for HullItem {
        entity: ship,
        mut timbers,
        handling,
        leaking: was_leaking,
        engaged,
    } in &mut ships
    {
        let gained = water_gained(timbers.0, handling.0, hours);
        let leaking = gained > 0.0;
        if leaking && !was_leaking {
            commands.entity(ship).insert(Leaking);
            alerts.write(Alert {
                ship,
                kind: AlertKind::Leaking,
            });
        } else if !leaking && was_leaking {
            commands.entity(ship).remove::<Leaking>();
        }
        if !leaking {
            continue;
        }
        timbers.0 = (timbers.0 - gained).max(0.0);
        if timbers.0 <= 0.0 && !engaged {
            founders.write(Founder {
                ship,
                cause: Cause::Foundered,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Purse;
    use crate::factions::{Colours, Nation};
    use crate::ship_class::ShipClass;
    use crate::test_support::{run_checked, run_until, sim_app};
    use crate::time_scale::MAX_STEP;

    #[test]
    fn a_full_company_holds_a_leak_until_she_is_badly_battered() {
        assert_eq!(water_gained(1.0, 1.0, 1.0), 0.0);
        assert_eq!(water_gained(0.3, 1.0, 1.0), 0.0);
        assert!(water_gained(0.1, 1.0, 1.0) > 0.0);
    }

    #[test]
    fn the_fewer_at_the_pumps_and_the_worse_her_hull_the_faster_she_fills() {
        assert!(water_gained(1.0, 0.0, 1.0) > 0.0, "a sound ship seeps");
        assert!(water_gained(0.5, 0.0, 1.0) > water_gained(1.0, 0.0, 1.0));
        assert!(water_gained(0.5, 0.2, 1.0) > water_gained(0.5, 0.4, 1.0));
        assert_eq!(water_gained(0.5, 1.5, 1.0), 0.0, "keen men pump no faster");
    }

    fn sea() -> App {
        let mut app = sim_app();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        app
    }

    fn ship(app: &mut App, crew: u32, timbers: f32) -> Entity {
        let class = ShipClass::sloop();
        app.world_mut()
            .spawn((
                Name::new("Dove"),
                class.build(crew, 4, class.empty_hold()),
                Timbers(timbers),
                Purse(0),
                Colours(Nation::ENGLAND),
                Transform::default(),
            ))
            .id()
    }

    #[test]
    fn a_ship_with_no_one_at_the_pumps_founders_within_a_day_or_so() {
        let mut app = sea();
        let empty = ship(&mut app, 0, 1.0);
        let manned = ship(&mut app, ShipClass::sloop().complement.min, 0.5);
        let gone = run_until(&mut app, 30.0 * HOUR as f32, |world| {
            world.get_entity(empty).is_err()
        });
        assert!(gone, "the empty ship went down");
        let world = app.world();
        assert_eq!(world.get::<Timbers>(manned), Some(&Timbers(0.5)));
        assert!(!world.entity(manned).contains::<Leaking>());
    }

    #[test]
    fn the_water_gaining_is_called_out_once() {
        let mut app = sea();
        let ship = ship(&mut app, 1, 0.6);
        let mut cursor = app.world().resource::<Messages<Alert>>().get_cursor();
        let mut called = 0;
        run_checked(&mut app, HOUR as f32, |world| {
            let messages = world.resource::<Messages<Alert>>();
            called += cursor
                .read(messages)
                .filter(|alert| alert.ship == ship && alert.kind == AlertKind::Leaking)
                .count();
        });
        let world = app.world();
        assert_eq!(called, 1);
        assert!(world.entity(ship).contains::<Leaking>());
        assert!(world.get::<Timbers>(ship).unwrap().0 < 0.6);
    }
}
