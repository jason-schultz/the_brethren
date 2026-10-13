//! Ship motion under sail. Owns the hull, helm, heading, and speed of every vessel.

pub mod physics;

use bevy::prelude::*;

use crate::map::Land;
use crate::math::{heading_to_direction, heading_to_rotation};
use crate::schedule::SimSet;
use crate::wind::Wind;
use physics::{approach, target_speed, true_wind_angle, turn_rate, turn_toward};

/// Sailing characteristics of a ship class. Every vessel has one.
/// These will move to data assets once there is more than one class.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(Heading, Speed, Helm, Transform)]
pub struct Hull {
    /// Hull speed limit in m/s.
    pub max_speed_mps: f32,
    /// Fraction of true wind speed reached at the best point of sail.
    pub wind_efficiency: f32,
    /// Closest true wind angle (radians) at which the sails still draw.
    pub no_go_angle: f32,
    /// True wind angle (radians) of the fastest point of sail.
    pub best_angle: f32,
    /// Fraction of best speed when running dead downwind.
    pub run_fraction: f32,
    /// Turn rate at full speed, rad/s.
    pub max_turn_rate: f32,
    /// Rate of speed change, m/s².
    pub acceleration: f32,
}

impl Hull {
    /// A gaff-rigged sloop. Fore-and-aft rigs point higher than square riggers
    /// and are fastest on a beam reach.
    pub const SLOOP: Self = Self {
        max_speed_mps: 5.5,
        wind_efficiency: 0.8,
        no_go_angle: 50.0_f32.to_radians(),
        best_angle: 105.0_f32.to_radians(),
        run_fraction: 0.75,
        max_turn_rate: 12.0_f32.to_radians(),
        acceleration: 0.25,
    };
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
        app.add_systems(FixedUpdate, sail_ships.in_set(SimSet::Physics));
    }
}

/// Steers, accelerates, and moves every ship one fixed step. A ship whose
/// next step would put it on land runs aground: it stops dead and stays
/// put until it is steered off.
fn sail_ships(
    time: Res<Time>,
    wind: Res<Wind>,
    land: Res<Land>,
    mut ships: Query<(&Hull, &Helm, &mut Heading, &mut Speed, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (hull, helm, mut heading, mut speed, mut transform) in &mut ships {
        let target = match helm.sails {
            Sails::Set => {
                heading.0 = turn_toward(heading.0, helm.course, turn_rate(hull, speed.0) * dt);
                target_speed(hull, true_wind_angle(heading.0, wind.from), wind.speed_mps)
            }
            Sails::Furled => 0.0,
        };
        speed.0 = approach(speed.0, target, hull.acceleration * dt);
        let next = transform.translation + heading_to_direction(heading.0) * speed.0 * dt;
        if land.contains(next) {
            speed.0 = 0.0;
        } else {
            transform.translation = next;
        }
        transform.rotation = heading_to_rotation(heading.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::LandMass;
    use crate::map::data::Terrain;
    use crate::math::{angle_between, direction_to_heading, normalize_heading, to_plane};
    use crate::test_support::{run_seconds, sim_app};

    fn spawn_sloop(app: &mut App, heading: f32, sails: Sails) -> Entity {
        app.world_mut()
            .spawn((
                Hull::SLOOP,
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
    fn ship_head_to_wind_makes_no_headway() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let ship = spawn_sloop(&mut app, wind_from, Sails::Set);
        run_seconds(&mut app, 60.0);
        assert!(distance_from_origin(&app, ship) < 0.01);
    }

    #[test]
    fn ship_on_a_reach_sails_away_along_its_heading() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let heading = normalize_heading(wind_from + Hull::SLOOP.best_angle);
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
        let ship = spawn_sloop(&mut app, wind_from + Hull::SLOOP.best_angle, Sails::Set);
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
        let heading = normalize_heading(wind_from + Hull::SLOOP.best_angle);
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
        run_seconds(&mut app, 120.0);

        let position = app.world().get::<Transform>(ship).unwrap().translation;
        assert!(!app.world().resource::<Land>().contains(position));
        assert!(
            position.length() > 90.0,
            "stopped short at {}",
            position.length()
        );
        assert_eq!(app.world().get::<Speed>(ship).unwrap().0, 0.0);
    }

    #[test]
    fn ship_turns_toward_helm_course() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let start = normalize_heading(wind_from + Hull::SLOOP.best_angle);
        let course = normalize_heading(start + 0.5);
        let ship = spawn_sloop(&mut app, start, Sails::Set);
        app.world_mut().get_mut::<Helm>(ship).unwrap().course = course;
        run_seconds(&mut app, 30.0);

        let heading = app.world().get::<Heading>(ship).unwrap().0;
        assert!(angle_between(heading, course).abs() < 1e-4);
    }
}
