//! Route planning around land: shortest paths on a visibility graph.
//!
//! The graph's nodes sit off every convex corner of the coastline. Two nodes
//! are linked when the leg between them keeps [`LAND_CLEARANCE`] from land. A
//! route is the shortest chain of links from start to goal.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use bevy::math::FloatOrd;
use bevy::prelude::*;

use crate::geometry::convex_corners_offset;
use crate::map::Land;
use crate::math::from_plane;

/// Minimum distance (meters) a planned leg keeps from land.
pub const LAND_CLEARANCE: f32 = 60.0;
/// How far off each convex corner the graph nodes sit. Further out than
/// [`LAND_CLEARANCE`], so legs between neighboring corners stay clear.
const CORNER_OFFSET: f32 = LAND_CLEARANCE * 2.0;
/// An endpoint already close to land, such as a ship at a quay, only needs
/// its legs to stay this fraction of its own distance from land.
const ENDPOINT_SLACK: f32 = 0.9;

/// Length in meters of a route starting at `from`.
pub fn route_length(from: Vec3, route: &[Vec3]) -> f32 {
    route
        .iter()
        .fold((from, 0.0), |(start, length), &point| {
            (point, length + start.distance(point))
        })
        .1
}

/// The point `distance` meters along the route from `from`, or its end if
/// the route is shorter.
pub fn point_along(from: Vec3, route: &[Vec3], distance: f32) -> Vec3 {
    let mut start = from;
    let mut left = distance;
    for &point in route {
        let leg = start.distance(point);
        if leg >= left {
            return start.lerp(point, if leg > 0.0 { left / leg } else { 0.0 });
        }
        left -= leg;
        start = point;
    }
    start
}

/// Precomputed visibility graph for the current [`Land`].
#[derive(Resource, Debug, Default)]
pub struct NavGraph {
    nodes: Vec<Vec3>,
    links: Vec<Vec<usize>>,
}

impl NavGraph {
    pub fn build(land: &Land) -> Self {
        let nodes: Vec<Vec3> = land
            .masses
            .iter()
            .flat_map(|mass| convex_corners_offset(&mass.outline, CORNER_OFFSET))
            .map(from_plane)
            // Nodes squeezed between land masses are no use as waypoints.
            .filter(|&node| land.clearance(node, node) >= LAND_CLEARANCE)
            .collect();
        let mut links = vec![Vec::new(); nodes.len()];
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                if land.clearance(nodes[i], nodes[j]) >= LAND_CLEARANCE {
                    links[i].push(j);
                    links[j].push(i);
                }
            }
        }
        Self { nodes, links }
    }

    /// Waypoints from `from` to `to` that keep clear of land, ending at `to`.
    ///
    /// If there's no route, which happens when either end is on land or
    /// enclosed by it, this returns the direct leg and the ship runs aground
    /// trying.
    pub fn plan(&self, land: &Land, from: Vec3, to: Vec3) -> Vec<Vec3> {
        if leg_is_clear(land, from, to) {
            vec![to]
        } else {
            self.shortest_path(land, from, to)
                .unwrap_or_else(|| vec![to])
        }
    }

    /// Dijkstra over the graph plus the two endpoints. The graph has only a
    /// few hundred nodes, so a heuristic (A*) isn't worth the complexity.
    fn shortest_path(&self, land: &Land, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let start = self.nodes.len();
        let goal = start + 1;
        let position = |i: usize| match i {
            i if i == start => from,
            i if i == goal => to,
            i => self.nodes[i],
        };
        let visible_from = |point: Vec3| -> Vec<usize> {
            (0..self.nodes.len())
                .filter(|&i| leg_is_clear(land, point, self.nodes[i]))
                .collect()
        };
        let start_links = visible_from(from);
        let goal_links = visible_from(to);
        let neighbors = |i: usize| -> Vec<usize> {
            let mut out = if i == start {
                start_links.clone()
            } else {
                self.links[i].clone()
            };
            if goal_links.contains(&i) {
                out.push(goal);
            }
            out
        };

        let mut cost = vec![f32::INFINITY; goal + 1];
        let mut previous = vec![usize::MAX; goal + 1];
        let mut frontier = BinaryHeap::new();
        cost[start] = 0.0;
        frontier.push(Reverse((FloatOrd(0.0), start)));
        while let Some(Reverse((FloatOrd(reached), current))) = frontier.pop() {
            if current == goal {
                break;
            }
            if reached > cost[current] {
                continue;
            }
            for next in neighbors(current) {
                let candidate = reached + position(current).distance(position(next));
                if candidate < cost[next] {
                    cost[next] = candidate;
                    previous[next] = current;
                    frontier.push(Reverse((FloatOrd(candidate), next)));
                }
            }
        }
        if previous[goal] == usize::MAX {
            return None;
        }

        let mut path = vec![to];
        let mut current = previous[goal];
        while current != start {
            path.push(position(current));
            current = previous[current];
        }
        path.reverse();
        Some(path)
    }
}

/// Whether a leg between two arbitrary points (a ship, a destination) keeps
/// clear of land. The required clearance relaxes for endpoints that are
/// themselves close to land, so a ship near the shore can still sail away.
fn leg_is_clear(land: &Land, from: Vec3, to: Vec3) -> bool {
    let endpoint_room = land.clearance(from, from).min(land.clearance(to, to));
    let required = LAND_CLEARANCE.min(endpoint_room * ENDPOINT_SLACK);
    let clearance = land.clearance(from, to);
    clearance > 0.0 && clearance >= required
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::LandMass;
    use crate::map::data::Terrain;

    /// A 200 m square island centered on the origin.
    fn island() -> Land {
        let half = 100.0;
        Land {
            masses: vec![LandMass::new(
                "Test Island",
                Terrain::Sand,
                vec![
                    Vec2::new(-half, -half),
                    Vec2::new(half, -half),
                    Vec2::new(half, half),
                    Vec2::new(-half, half),
                ],
                None,
            )],
        }
    }

    #[test]
    fn a_point_along_a_route_follows_its_legs() {
        let route = [Vec3::new(100.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 100.0)];
        assert_eq!(
            point_along(Vec3::ZERO, &route, 50.0),
            Vec3::new(50.0, 0.0, 0.0)
        );
        assert_eq!(
            point_along(Vec3::ZERO, &route, 150.0),
            Vec3::new(100.0, 0.0, 50.0)
        );
        assert_eq!(point_along(Vec3::ZERO, &route, 999.0), route[1], "the end");
    }

    #[test]
    fn open_water_route_is_direct() {
        let land = island();
        let graph = NavGraph::build(&land);
        let to = Vec3::new(500.0, 0.0, -300.0);
        assert_eq!(graph.plan(&land, Vec3::new(300.0, 0.0, 0.0), to), vec![to]);
    }

    #[test]
    fn route_goes_around_an_island_with_clearance() {
        let land = island();
        let graph = NavGraph::build(&land);
        let from = Vec3::new(-300.0, 0.0, 0.0);
        let to = Vec3::new(300.0, 0.0, 0.0);
        let route = graph.plan(&land, from, to);

        assert!(route.len() >= 3, "should round two corners: {route:?}");
        assert_eq!(route.last(), Some(&to));
        let mut leg_start = from;
        for &waypoint in &route {
            assert!(
                land.clearance(leg_start, waypoint) >= LAND_CLEARANCE,
                "{waypoint}"
            );
            leg_start = waypoint;
        }
    }

    #[test]
    fn route_length_adds_up_every_leg() {
        let land = island();
        let graph = NavGraph::build(&land);
        let from = Vec3::new(-300.0, 0.0, 0.0);
        let to = Vec3::new(300.0, 0.0, 0.0);
        let route = graph.plan(&land, from, to);
        let length = route_length(from, &route);
        assert!(length > from.distance(to), "a detour is longer: {length}");
        assert_eq!(route_length(from, &[]), 0.0);
        assert_eq!(
            route_length(from, &graph.plan(&land, from, from + Vec3::Z * -50.0)),
            50.0
        );
    }

    #[test]
    fn leg_passing_too_close_to_land_is_rerouted() {
        let land = island();
        let graph = NavGraph::build(&land);
        // A straight leg that would pass 20 m off the island's north face.
        let from = Vec3::new(-300.0, 0.0, -120.0);
        let to = Vec3::new(300.0, 0.0, -120.0);
        let route = graph.plan(&land, from, to);
        assert!(route.len() > 1, "skimmed the shore: {route:?}");
    }

    #[test]
    fn ship_close_to_shore_can_still_sail_away() {
        let land = island();
        let graph = NavGraph::build(&land);
        let near_shore = Vec3::new(105.0, 0.0, 0.0);
        let offshore = Vec3::new(400.0, 0.0, 0.0);
        assert_eq!(graph.plan(&land, near_shore, offshore), vec![offshore]);
    }

    #[test]
    fn graph_nodes_keep_clear_of_land() {
        let land = island();
        let graph = NavGraph::build(&land);
        assert_eq!(graph.nodes.len(), 4);
        assert!(
            graph
                .nodes
                .iter()
                .all(|&n| land.clearance(n, n) >= LAND_CLEARANCE)
        );
    }

    #[test]
    fn destination_on_land_falls_back_to_direct() {
        let land = island();
        let graph = NavGraph::build(&land);
        let to = Vec3::ZERO;
        assert_eq!(graph.plan(&land, Vec3::new(-300.0, 0.0, 0.0), to), vec![to]);
    }
}
