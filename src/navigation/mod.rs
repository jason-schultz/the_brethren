//! Standing orders: where a ship has been told to go, and how its crew gets there.
//!
//! Anyone, whether player input or (later) AI captains, issues a
//! [`SailOrder`]. Orders are planned into a [`Route`] around land every
//! frame, even while paused, and sailed in the fixed simulation step. When a
//! ship completes its route, an [`Arrived`] message tells other domains,
//! such as ports, what the voyage was for.

pub mod course;
pub mod route;

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::map::Land;
use crate::math::{direction_to_heading, to_plane};
use crate::sailing::{Heading, Helm, Hull, Sails};
use crate::schedule::{FrameSet, SimSet};
use crate::wind::Wind;
use course::{course_to, tacking_corridor};
use route::{NavGraph, Waypoint};

/// A ship within this distance (meters) of a waypoint has reached it.
pub const ARRIVAL_RADIUS: f32 = 40.0;

/// What a ship does when it completes a voyage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Arrival {
    /// Take in sail and stop.
    #[default]
    HeaveTo,
    /// Dock at this port.
    Dock(Entity),
}

/// Order a ship to sail to a point on the ocean.
#[derive(Message, Debug, Clone, Copy)]
pub struct SailOrder {
    pub ship: Entity,
    pub destination: Vec3,
    pub on_arrival: Arrival,
}

/// Sent when a ship completes its route.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrived {
    pub ship: Entity,
    pub on_arrival: Arrival,
}

/// The voyage a ship is sailing. Removed on arrival.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Route {
    /// Remaining waypoints, next first. The last is the destination.
    pub waypoints: VecDeque<Waypoint>,
    /// Where the current leg began. Used to keep tacks near the leg.
    pub leg_start: Vec3,
    pub on_arrival: Arrival,
}

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SailOrder>()
            .add_message::<Arrived>()
            .init_resource::<NavGraph>()
            .add_systems(
                Update,
                (
                    rebuild_nav_graph.run_if(resource_changed::<Land>),
                    plan_routes,
                )
                    .chain()
                    .in_set(FrameSet::Orders),
            )
            .add_systems(FixedUpdate, sail_routes.in_set(SimSet::Navigation));
    }
}

fn rebuild_nav_graph(land: Res<Land>, mut graph: ResMut<NavGraph>) {
    *graph = NavGraph::build(&land);
}

fn plan_routes(
    mut orders: MessageReader<SailOrder>,
    land: Res<Land>,
    graph: Res<NavGraph>,
    ships: Query<&Transform>,
    mut commands: Commands,
) {
    for order in orders.read() {
        let Ok(transform) = ships.get(order.ship) else {
            continue;
        };
        let start = transform.translation.with_y(0.0);
        let waypoints = graph.plan(&land, start, order.destination.with_y(0.0));
        commands.entity(order.ship).try_insert(Route {
            waypoints: waypoints.into(),
            leg_start: start,
            on_arrival: order.on_arrival,
        });
    }
}

/// Signed distance (meters) of `position` from the line `leg_start → target`,
/// positive to starboard of the track.
fn cross_track(leg_start: Vec3, target: Vec3, position: Vec3) -> f32 {
    let track = to_plane(target - leg_start).normalize_or_zero();
    let starboard = Vec2::new(-track.y, track.x);
    to_plane(position - leg_start).dot(starboard)
}

/// Sets each ship's helm along its route and completes the voyage on arrival.
fn sail_routes(
    mut commands: Commands,
    mut arrivals: MessageWriter<Arrived>,
    wind: Res<Wind>,
    mut ships: Query<(Entity, &mut Route, &Hull, &Heading, &Transform, &mut Helm)>,
) {
    for (ship, mut route, hull, heading, transform, mut helm) in &mut ships {
        let position = transform.translation.with_y(0.0);
        while let Some(next) = route.waypoints.front()
            && position.distance(next.point) <= ARRIVAL_RADIUS
        {
            route.leg_start = next.point;
            route.waypoints.pop_front();
        }
        let Some(&Waypoint {
            point: target,
            leg_clearance,
        }) = route.waypoints.front()
        else {
            helm.sails = Sails::Furled;
            commands.entity(ship).remove::<Route>();
            arrivals.write(Arrived {
                ship,
                on_arrival: route.on_arrival,
            });
            continue;
        };
        let bearing = direction_to_heading(target - position);
        let off_track = cross_track(route.leg_start, target, position);
        let corridor = tacking_corridor(leg_clearance);
        helm.course = course_to(
            bearing,
            wind.from,
            hull.no_go_angle,
            heading.0,
            off_track,
            corridor,
        );
        helm.sails = Sails::Set;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::LandMass;
    use crate::map::data::Terrain;
    use crate::math::heading_to_direction;
    use crate::sailing::Speed;
    use crate::test_support::{run_checked, run_seconds, sim_app};

    /// Spawns a sloop at the origin and orders it to sail `distance` meters toward `bearing`.
    fn order_sloop(app: &mut App, bearing: f32, distance: f32) -> (Entity, Vec3) {
        let ship = app.world_mut().spawn(Hull::SLOOP).id();
        let destination = heading_to_direction(bearing) * distance;
        app.world_mut().write_message(SailOrder {
            ship,
            destination,
            on_arrival: Arrival::HeaveTo,
        });
        (ship, destination)
    }

    fn position(world: &World, ship: Entity) -> Vec3 {
        world.get::<Transform>(ship).unwrap().translation
    }

    fn assert_arrived(app: &App, ship: Entity, destination: Vec3) {
        assert!(
            app.world().get::<Route>(ship).is_none(),
            "route not cleared"
        );
        let position = position(app.world(), ship);
        assert!(
            position.distance(destination) <= ARRIVAL_RADIUS + 5.0,
            "stopped at {position}, destination {destination}"
        );
        assert_eq!(app.world().get::<Helm>(ship).unwrap().sails, Sails::Furled);
    }

    #[test]
    fn cross_track_is_positive_to_starboard() {
        let north = Vec3::new(0.0, 0.0, -100.0);
        assert!((cross_track(Vec3::ZERO, north, Vec3::new(10.0, 0.0, -50.0)) - 10.0).abs() < 1e-4);
        assert!((cross_track(Vec3::ZERO, north, Vec3::new(-7.0, 0.0, -50.0)) + 7.0).abs() < 1e-4);
    }

    #[test]
    fn order_is_planned_into_a_route_even_while_paused() {
        let mut app = sim_app();
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let (ship, destination) = order_sloop(&mut app, 0.0, 500.0);
        app.update();
        let route = app.world().get::<Route>(ship).expect("route recorded");
        let points: Vec<Vec3> = route.waypoints.iter().map(|w| w.point).collect();
        assert_eq!(points, vec![destination]);
    }

    #[test]
    fn ship_reaches_a_downwind_destination() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let (ship, destination) = order_sloop(&mut app, wind_from + std::f32::consts::PI, 800.0);
        run_seconds(&mut app, 600.0);
        assert_arrived(&app, ship, destination);
    }

    #[test]
    fn ship_tacks_to_reach_a_destination_dead_upwind() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let (ship, destination) = order_sloop(&mut app, wind_from, 600.0);
        run_seconds(&mut app, 1800.0);
        assert_arrived(&app, ship, destination);
    }

    #[test]
    fn beating_upwind_stays_near_the_tacking_corridor() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let (ship, destination) = order_sloop(&mut app, wind_from, 1000.0);
        let mut worst: f32 = 0.0;
        run_checked(&mut app, 2400.0, |world| {
            let off = cross_track(Vec3::ZERO, destination, position(world, ship));
            worst = worst.max(off.abs());
        });
        assert_arrived(&app, ship, destination);
        // The ship overshoots the corridor a little while it turns through the wind.
        assert!(
            worst < course::MAX_TACKING_CORRIDOR * 1.5,
            "strayed {worst} m off track"
        );
    }

    #[test]
    fn ship_sails_around_an_island_without_touching_land() {
        let mut app = sim_app();
        let (center, half) = (-400.0, 150.0);
        app.insert_resource(Land {
            masses: vec![LandMass::new(
                "Test Island",
                Terrain::Sand,
                vec![
                    Vec2::new(-half, center - half),
                    Vec2::new(half, center - half),
                    Vec2::new(half, center + half),
                    Vec2::new(-half, center + half),
                ],
                None,
            )],
        });
        // Destination due north, directly behind the island.
        let (ship, destination) = order_sloop(&mut app, 0.0, 800.0);
        run_checked(&mut app, 1500.0, |world| {
            let position = position(world, ship);
            let room = world.resource::<Land>().clearance(position, position);
            assert!(room > 10.0, "came within {room} m of land at {position}");
        });
        assert_arrived(&app, ship, destination);
    }

    #[test]
    fn arrival_is_announced_with_its_purpose() {
        let mut app = sim_app();
        let ship = app.world_mut().spawn(Hull::SLOOP).id();
        let port = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(SailOrder {
            ship,
            destination: Vec3::new(200.0, 0.0, 0.0),
            on_arrival: Arrival::Dock(port),
        });
        let mut seen = Vec::new();
        run_checked(&mut app, 300.0, |world| {
            let messages = world.resource::<Messages<Arrived>>();
            seen.extend(messages.iter_current_update_messages().copied());
        });
        assert_eq!(
            seen,
            vec![Arrived {
                ship,
                on_arrival: Arrival::Dock(port)
            }]
        );
    }

    #[test]
    fn ship_heaves_to_and_stops_after_arriving() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let (ship, _) = order_sloop(&mut app, wind_from + Hull::SLOOP.best_angle, 300.0);
        run_seconds(&mut app, 400.0);
        assert_eq!(app.world().get::<Speed>(ship).unwrap().0, 0.0);
    }
}
