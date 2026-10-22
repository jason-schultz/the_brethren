//! Ship motion under sail. Owns the hull, helm, heading, and speed of every vessel.

pub mod physics;

use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use serde::Deserialize;

use crate::alert::{Alert, AlertKind};
use crate::map::Land;
use crate::math::{heading_to_direction, heading_to_rotation};
use crate::schedule::SimSet;
use crate::wind::Wind;
use physics::{approach, sail_speed, target_speed, true_wind_angle, turn_rate, turn_toward};

/// Sailing characteristics of a ship class. Every vessel has one.
#[derive(Component, Deserialize, Debug, Clone, Copy, PartialEq)]
#[require(Heading, Speed, Helm, SailHandling, Rigging, Laden, Aground, Transform)]
pub struct Hull {
    /// Speed with all sail set and the wind on the beam, in m/s.
    pub speed_mps: f32,
    /// Speed multiplier sailing straight into the wind.
    pub headwind_factor: f32,
    /// Speed multiplier running straight before the wind.
    pub tailwind_factor: f32,
    /// Turn rate at full speed, rad/s. The ships data gives it in degrees.
    #[serde(rename = "turn_degrees", deserialize_with = "degrees")]
    pub max_turn_rate: f32,
    /// Rate of speed change, m/s².
    pub acceleration: f32,
}

/// Reads an angle given in degrees as radians.
fn degrees<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    f32::deserialize(deserializer).map(f32::to_radians)
}

/// Direction the bow points: radians clockwise from north.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct Heading(pub f32);

/// Speed through the water, m/s.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct Speed(pub f32);

/// The crew's current orders for rudder and sails.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct Helm {
    /// Heading to steer for. Ignored while the sails are furled.
    pub course: f32,
    pub sails: Sails,
}

/// A grounded ship is afloat again once this much water (meters) lies clear
/// ahead of her. Without the margin, a ship creeping into the shore would
/// ground, float, and ground again every few steps.
const REFLOAT_DISTANCE: f32 = 20.0;

/// Whether the ship is stuck on land, unable to move along her heading.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Aground(pub bool);

/// A ship has run aground at `speed_mps`.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct Grounded {
    pub ship: Entity,
    pub speed_mps: f32,
}

/// Fraction of her sail the crew can work, from 0 (none) to 1 (all plain
/// sail). Short-handed ships are slower and turn more slowly. The crew
/// domain keeps this up to date; ships without a crew sail at full handling.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct SailHandling(pub f32);

impl Default for SailHandling {
    fn default() -> Self {
        Self(1.0)
    }
}

/// What's left of her masts, yards, and sails, from 1 (sound) to 0
/// (dismasted). Damaged rigging carries less sail, so she's slower.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Rigging(pub f32);

impl Default for Rigging {
    fn default() -> Self {
        Self(1.0)
    }
}

/// How full her hold is, from 0 (empty) to 1 (full). A laden ship is
/// slower. The cargo domain keeps this up to date.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct Laden(pub f32);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sails {
    /// Sails taken in. The ship slows to a stop and holds its heading.
    #[default]
    Furled,
    /// All plain sail set.
    Set,
}

pub struct SailingPlugin;

impl Plugin for SailingPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Grounded>()
            .add_systems(FixedUpdate, sail_ships.in_set(SimSet::Physics));
    }
}

/// A ship under way: her hull and how she's handled, and where she is.
#[derive(QueryData)]
#[query_data(mutable)]
struct Sailing {
    entity: Entity,
    hull: &'static Hull,
    helm: &'static Helm,
    handling: &'static SailHandling,
    rigging: &'static Rigging,
    laden: &'static Laden,
    heading: &'static mut Heading,
    speed: &'static mut Speed,
    transform: &'static mut Transform,
    aground: &'static mut Aground,
}

/// Steers, accelerates, and moves every ship one fixed step. A ship whose
/// next step would put it on land runs aground: it stops dead and stays
/// put until it is steered off.
fn sail_ships(
    time: Res<Time>,
    wind: Res<Wind>,
    land: Res<Land>,
    mut alerts: MessageWriter<Alert>,
    mut groundings: MessageWriter<Grounded>,
    mut ships: Query<Sailing>,
) {
    let dt = time.delta_secs();
    for SailingItem {
        entity: ship,
        hull,
        helm,
        handling,
        rigging,
        laden,
        mut heading,
        mut speed,
        mut transform,
        mut aground,
    } in &mut ships
    {
        let target = match helm.sails {
            Sails::Set => {
                let turn = turn_rate(hull, speed.0) * handling.0 * dt;
                heading.0 = turn_toward(heading.0, helm.course, turn);
                sail_speed(
                    target_speed(hull, true_wind_angle(heading.0, wind.from)),
                    handling.0,
                    rigging.0,
                    laden.0,
                )
            }
            Sails::Furled => 0.0,
        };
        speed.0 = approach(speed.0, target, hull.acceleration * dt);
        let moving = speed.0 > 0.0;
        let ahead = heading_to_direction(heading.0);
        let next = transform.translation + ahead * speed.0 * dt;
        if moving && land.contains(next) {
            if !aground.0 {
                alerts.write(Alert {
                    ship,
                    kind: AlertKind::Aground,
                });
                groundings.write(Grounded {
                    ship,
                    speed_mps: speed.0,
                });
            }
            speed.0 = 0.0;
            aground.0 = true;
        } else {
            if moving && aground.0 && !land.contains(next + ahead * REFLOAT_DISTANCE) {
                aground.0 = false;
            }
            transform.translation = next;
        }
        transform.rotation = heading_to_rotation(heading.0);
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use super::*;
    use crate::map::LandMass;
    use crate::map::data::Terrain;
    use crate::math::{angle_between, direction_to_heading, normalize_heading, to_plane};
    use crate::ship_class::ShipClass;
    use crate::test_support::{run_checked, run_seconds, sim_app};

    fn spawn_sloop(app: &mut App, heading: f32, sails: Sails) -> Entity {
        app.world_mut()
            .spawn((
                ShipClass::sloop().hull,
                Heading(heading),
                Helm {
                    course: heading,
                    sails,
                },
            ))
            .id()
    }

    fn distance_from_origin(app: &App, ship: Entity) -> f32 {
        app.world()
            .get::<Transform>(ship)
            .unwrap()
            .translation
            .length()
    }

    #[test]
    fn ship_sails_slower_into_the_wind_than_before_it() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let upwind = spawn_sloop(&mut app, wind_from, Sails::Set);
        let downwind = spawn_sloop(&mut app, normalize_heading(wind_from + PI), Sails::Set);
        run_seconds(&mut app, 60.0);
        let upwind = distance_from_origin(&app, upwind);
        assert!(upwind > 100.0, "only moved {upwind}");
        assert!(upwind < distance_from_origin(&app, downwind));
    }

    #[test]
    fn ship_sails_away_along_its_heading() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let heading = normalize_heading(wind_from + FRAC_PI_2);
        let ship = spawn_sloop(&mut app, heading, Sails::Set);
        run_seconds(&mut app, 60.0);

        let position = app.world().get::<Transform>(ship).unwrap().translation;
        assert!(position.length() > 100.0, "only moved {position}");
        let travelled = direction_to_heading(position);
        assert!(angle_between(heading, travelled).abs() < 1e-3);
    }

    #[test]
    fn furled_ship_stays_put() {
        let mut app = sim_app();
        let ship = spawn_sloop(&mut app, 3.0, Sails::Furled);
        run_seconds(&mut app, 30.0);
        assert_eq!(distance_from_origin(&app, ship), 0.0);
    }

    #[test]
    fn paused_time_freezes_ships() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let ship = spawn_sloop(&mut app, wind_from + FRAC_PI_2, Sails::Set);
        run_seconds(&mut app, 10.0);
        let before = distance_from_origin(&app, ship);

        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        run_seconds(&mut app, 10.0);
        assert_eq!(distance_from_origin(&app, ship), before);
    }

    #[test]
    fn ship_runs_aground_at_the_shore_and_does_not_enter_land() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let heading = normalize_heading(wind_from + FRAC_PI_2);
        // A wide wall of land 100 m ahead, across the ship's path.
        let ahead = heading_to_direction(heading);
        let across = Vec2::new(-ahead.z, ahead.x) * 1000.0;
        let near = to_plane(ahead * 100.0);
        let far = to_plane(ahead * 300.0);
        app.insert_resource(Land {
            masses: vec![LandMass::new(
                "Wall",
                Terrain::Sand,
                vec![near - across, near + across, far + across, far - across],
                None,
            )],
        });
        let ship = spawn_sloop(&mut app, heading, Sails::Set);
        let mut alerts = Vec::new();
        run_checked(&mut app, 120.0, |world| {
            let messages = world.resource::<Messages<Alert>>();
            alerts.extend(messages.iter_current_update_messages().copied());
        });
        assert_eq!(
            alerts,
            vec![Alert {
                ship,
                kind: AlertKind::Aground
            }],
            "alerted once, not every step against the shore"
        );

        let position = app.world().get::<Transform>(ship).unwrap().translation;
        assert!(!app.world().resource::<Land>().contains(position));
        assert!(
            position.length() > 90.0,
            "stopped short at {}",
            position.length()
        );
        assert_eq!(app.world().get::<Speed>(ship).unwrap().0, 0.0);
        assert_eq!(app.world().get::<Aground>(ship), Some(&Aground(true)));
    }

    #[test]
    fn short_handed_ship_is_slower_and_an_unmanned_one_does_not_move() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let heading = normalize_heading(wind_from + FRAC_PI_2);
        let full = spawn_sloop(&mut app, heading, Sails::Set);
        let half = spawn_sloop(&mut app, heading, Sails::Set);
        let none = spawn_sloop(&mut app, heading, Sails::Set);
        app.world_mut().entity_mut(half).insert(SailHandling(0.5));
        app.world_mut().entity_mut(none).insert(SailHandling(0.0));
        run_seconds(&mut app, 60.0);

        let full = distance_from_origin(&app, full);
        let half = distance_from_origin(&app, half);
        assert!(half > 0.0 && half < full * 0.6, "half {half}, full {full}");
        assert_eq!(distance_from_origin(&app, none), 0.0);
    }

    #[test]
    fn damaged_rigging_slows_a_ship_and_none_stops_her() {
        let mut app = sim_app();
        let heading = FRAC_PI_2;
        let sound = spawn_sloop(&mut app, heading, Sails::Set);
        let damaged = spawn_sloop(&mut app, heading, Sails::Set);
        let dismasted = spawn_sloop(&mut app, heading, Sails::Set);
        app.world_mut().entity_mut(damaged).insert(Rigging(0.5));
        app.world_mut().entity_mut(dismasted).insert(Rigging(0.0));
        run_seconds(&mut app, 60.0);

        let sound = distance_from_origin(&app, sound);
        let damaged = distance_from_origin(&app, damaged);
        assert!(
            damaged > 0.0 && damaged < sound * 0.6,
            "{damaged} vs {sound}"
        );
        assert_eq!(distance_from_origin(&app, dismasted), 0.0);
    }

    #[test]
    fn ship_turns_toward_helm_course() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let start = normalize_heading(wind_from + FRAC_PI_2);
        let course = normalize_heading(start + 0.5);
        let ship = spawn_sloop(&mut app, start, Sails::Set);
        app.world_mut().get_mut::<Helm>(ship).unwrap().course = course;
        run_seconds(&mut app, 30.0);

        let heading = app.world().get::<Heading>(ship).unwrap().0;
        assert!(angle_between(heading, course).abs() < 1e-4);
    }
}
