//! Standing orders: where a ship has been told to go, and how its crew gets there.
//!
//! Anyone, whether player input or (later) AI captains, issues a
//! [`SailOrder`]. Orders are planned into a [`Route`] around land every
//! frame, even while paused, and sailed in the fixed simulation step. When a
//! ship completes its route, an [`Arrived`] message tells other domains,
//! such as ports, what the voyage was for.
//!
//! A ship can also keep [`Company`] with another, staying in her wake.

pub mod route;

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::combat::FreeAtSea;
use crate::map::Land;
use crate::math::direction_to_heading;
use crate::sailing::{Helm, Sails};
use crate::schedule::{FrameSet, SimSet};
use route::NavGraph;

/// A ship within this distance (meters) of a waypoint has reached it.
pub const ARRIVAL_RADIUS: f32 = 40.0;
/// A ship keeping company stays within this distance of her leader, meters.
pub const KEEP_UP: f32 = 250.0;
/// She closes to lie this far astern of her leader, meters.
const ASTERN: f32 = 120.0;

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
    pub waypoints: VecDeque<Vec3>,
    pub on_arrival: Arrival,
}

/// Keeping company with another ship at sea: staying in her wake. A ship
/// in port keeps company once she's cast off, and a ship in action fights
/// first.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Company(pub Entity);

/// Order a ship to keep company with another, or with `None`, to stop.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeepCompany {
    pub ship: Entity,
    pub with: Option<Entity>,
}

/// Where a ship at `at`, keeping company with a leader at `leader_at`,
/// should sail, if she's fallen behind and isn't already closing: astern
/// of her leader. `heading_for` is the end of her route, if she has one.
pub fn keep_company(at: Vec3, heading_for: Option<Vec3>, leader_at: Vec3) -> Option<Vec3> {
    let astern = leader_at + (at - leader_at).normalize_or_zero() * ASTERN;
    let closing = heading_for.is_some_and(|end| end.distance(astern) <= KEEP_UP / 2.0);
    (at.distance(leader_at) > KEEP_UP && !closing).then_some(astern)
}

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SailOrder>()
            .add_message::<Arrived>()
            .add_message::<KeepCompany>()
            .init_resource::<NavGraph>()
            .add_systems(
                Update,
                (
                    rebuild_nav_graph.run_if(resource_changed::<Land>),
                    plan_routes,
                    take_company,
                )
                    .chain()
                    .in_set(FrameSet::Orders),
            )
            .add_systems(
                FixedUpdate,
                (sail_in_company, sail_routes)
                    .chain()
                    .in_set(SimSet::Navigation),
            );
    }
}

fn take_company(mut orders: MessageReader<KeepCompany>, mut commands: Commands) {
    for order in orders.read() {
        match order.with {
            Some(leader) if leader != order.ship => {
                commands.entity(order.ship).try_insert(Company(leader));
            }
            _ => {
                commands.entity(order.ship).try_remove::<Company>();
            }
        }
    }
}

/// Ships keeping company that have fallen behind close with their
/// leaders. Company with a ship that's gone is over.
fn sail_in_company(
    ships: Query<(Entity, &Company, &Transform, Option<&Route>), FreeAtSea>,
    leaders: Query<&Transform>,
    mut orders: MessageWriter<SailOrder>,
    mut commands: Commands,
) {
    for (ship, &Company(leader), at, route) in &ships {
        let Ok(leader_at) = leaders.get(leader) else {
            commands.entity(ship).remove::<Company>();
            continue;
        };
        let heading_for = route.and_then(|route| route.waypoints.back().copied());
        if let Some(astern) = keep_company(at.translation, heading_for, leader_at.translation) {
            orders.write(SailOrder {
                ship,
                destination: astern,
                on_arrival: Arrival::HeaveTo,
            });
        }
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
            on_arrival: order.on_arrival,
        });
    }
}

/// Sets each ship's helm along its route and completes the voyage on arrival.
fn sail_routes(
    mut commands: Commands,
    mut arrivals: MessageWriter<Arrived>,
    mut ships: Query<(Entity, &mut Route, &Transform, &mut Helm)>,
) {
    for (ship, mut route, transform, mut helm) in &mut ships {
        let position = transform.translation.with_y(0.0);
        while let Some(next) = route.waypoints.front()
            && position.distance(*next) <= ARRIVAL_RADIUS
        {
            route.waypoints.pop_front();
        }
        let Some(&target) = route.waypoints.front() else {
            helm.sails = Sails::Furled;
            commands.entity(ship).remove::<Route>();
            arrivals.write(Arrived {
                ship,
                on_arrival: route.on_arrival,
            });
            continue;
        };
        helm.course = direction_to_heading(target - position);
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
    use crate::ship_class::ShipClass;
    use crate::test_support::{run_checked, run_seconds, sim_app};
    use crate::wind::Wind;

    #[test]
    fn a_ship_keeping_company_closes_only_when_she_falls_behind() {
        let leader = Vec3::ZERO;
        let far = Vec3::new(1_000.0, 0.0, 0.0);
        let astern = keep_company(far, None, leader).unwrap();
        assert_eq!(astern, Vec3::new(ASTERN, 0.0, 0.0), "astern, on her side");
        assert_eq!(keep_company(far, Some(astern), leader), None, "closing");
        let near = Vec3::new(KEEP_UP / 2.0, 0.0, 0.0);
        assert_eq!(keep_company(near, None, leader), None, "in company");
    }

    #[test]
    fn a_ship_keeps_company_with_her_leader_until_told_otherwise() {
        let mut app = sim_app();
        let (leader, _) = order_sloop(&mut app, 0.0, 5_000.0);
        let follower = app
            .world_mut()
            .spawn((
                ShipClass::sloop().hull,
                Transform::from_xyz(-600.0, 0.0, 0.0),
            ))
            .id();
        app.world_mut().write_message(KeepCompany {
            ship: follower,
            with: Some(leader),
        });
        run_seconds(&mut app, 600.0);
        let gap = |app: &App| {
            let at = |ship| app.world().get::<Transform>(ship).unwrap().translation;
            at(leader).distance(at(follower))
        };
        assert!(gap(&app) <= KEEP_UP * 2.0, "fell behind: {}", gap(&app));
        app.world_mut().write_message(KeepCompany {
            ship: follower,
            with: None,
        });
        app.update();
        assert_eq!(app.world().get::<Company>(follower), None);
    }

    /// Spawns a sloop at the origin and orders it to sail `distance` meters toward `bearing`.
    fn order_sloop(app: &mut App, bearing: f32, distance: f32) -> (Entity, Vec3) {
        let ship = app.world_mut().spawn(ShipClass::sloop().hull).id();
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
    fn order_is_planned_into_a_route_even_while_paused() {
        let mut app = sim_app();
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let (ship, destination) = order_sloop(&mut app, 0.0, 500.0);
        app.update();
        let route = app.world().get::<Route>(ship).expect("route recorded");
        assert_eq!(route.waypoints, [destination]);
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
    fn ship_reaches_a_destination_dead_upwind() {
        let mut app = sim_app();
        let wind_from = app.world().resource::<Wind>().from;
        let (ship, destination) = order_sloop(&mut app, wind_from, 800.0);
        run_seconds(&mut app, 600.0);
        assert_arrived(&app, ship, destination);
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
        let ship = app.world_mut().spawn(ShipClass::sloop().hull).id();
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
        let (ship, _) = order_sloop(&mut app, wind_from + std::f32::consts::FRAC_PI_2, 300.0);
        run_seconds(&mut app, 400.0);
        assert_eq!(app.world().get::<Speed>(ship).unwrap().0, 0.0);
    }
}
