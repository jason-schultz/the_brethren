//! The geography of the world: land and the ports on it.
//!
//! Map content is data ([`data::MapData`]). [`load_map`] turns it into world
//! state: the [`Land`] resource and [`Port`] entities.

pub mod data;
pub mod projection;

use bevy::prelude::*;

use crate::geometry::{polygon_contains, segment_polygon_distance};
use crate::math::to_plane;
use crate::port::Port;
use data::{MapData, Terrain};
use projection::GeoProjection;

/// All land on the current map. Ships cannot enter it.
#[derive(Resource, Debug, Default)]
pub struct Land {
    pub masses: Vec<LandMass>,
}

/// One island, cay, spit, or coastline.
#[derive(Debug, Clone)]
pub struct LandMass {
    pub name: String,
    pub terrain: Terrain,
    /// Outline in ocean-plane coordinates (see [`to_plane`]).
    pub outline: Vec<Vec2>,
    /// Where the name is drawn.
    pub label: Vec3,
    bounds: Rect,
}

impl LandMass {
    /// `label` defaults to the centroid of the outline's vertices.
    pub fn new(
        name: impl Into<String>,
        terrain: Terrain,
        outline: Vec<Vec2>,
        label: Option<Vec3>,
    ) -> Self {
        let bounds = outline
            .iter()
            .fold(Rect::from_corners(outline[0], outline[0]), |rect, &p| {
                rect.union_point(p)
            });
        let centroid = outline.iter().sum::<Vec2>() / outline.len() as f32;
        Self {
            name: name.into(),
            terrain,
            label: label.unwrap_or(Vec3::new(centroid.x, 0.0, centroid.y)),
            outline,
            bounds,
        }
    }

    pub fn contains(&self, position: Vec3) -> bool {
        let point = to_plane(position);
        self.bounds.contains(point) && polygon_contains(&self.outline, point)
    }

    /// How close the straight leg `from–to` comes to this land, in meters. Zero if it touches.
    pub fn clearance(&self, from: Vec3, to: Vec3) -> f32 {
        segment_polygon_distance(&self.outline, to_plane(from), to_plane(to))
    }
}

impl Land {
    /// Whether a world position is on land.
    pub fn contains(&self, position: Vec3) -> bool {
        self.masses.iter().any(|mass| mass.contains(position))
    }

    /// How close the straight leg `from–to` comes to any land, in meters.
    /// Zero if it touches land; infinite on an empty map. Pass the same point
    /// twice for a point's distance from land.
    pub fn clearance(&self, from: Vec3, to: Vec3) -> f32 {
        self.masses
            .iter()
            .map(|mass| mass.clearance(from, to))
            .fold(f32::INFINITY, f32::min)
    }
}

/// Where and how the player's ship starts on a map.
#[derive(Debug, Clone, Copy)]
pub struct PlayerStart {
    pub position: Vec3,
    pub heading: f32,
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Land>();
    }
}

/// Replaces the world's geography with `map` and spawns its ports.
pub fn load_map(commands: &mut Commands, map: &MapData) -> PlayerStart {
    let projection = GeoProjection::new(map.origin, map.distance_scale);
    let masses = map
        .land
        .iter()
        .map(|land| {
            let outline = land
                .outline
                .iter()
                .map(|&p| to_plane(projection.to_world(p)))
                .collect();
            let label = land.label.map(|p| projection.to_world(p));
            LandMass::new(land.name.clone(), land.terrain, outline, label)
        })
        .collect();
    commands.insert_resource(Land { masses });

    for port in &map.ports {
        commands.spawn((
            Name::new(port.name.clone()),
            Port {
                nation: port.nation.clone(),
                town: projection.to_world(port.town),
            },
            Transform::from_translation(projection.to_world(port.anchorage)),
        ));
    }

    PlayerStart {
        position: projection.to_world(map.player_start.position),
        heading: map.player_start.heading_degrees.to_radians(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn port_royal() -> (MapData, GeoProjection, Land) {
        let map = MapData::parse(data::PORT_ROYAL).expect("embedded map must parse");
        let projection = GeoProjection::new(map.origin, map.distance_scale);
        let mut world = World::new();
        world
            .run_system_once_with(
                |map: In<MapData>, mut commands: Commands| {
                    load_map(&mut commands, &map);
                },
                map.clone(),
            )
            .unwrap();
        let land = world.remove_resource::<Land>().unwrap();
        (map, projection, land)
    }

    #[test]
    fn port_royal_map_is_valid() {
        let (map, projection, land) = port_royal();
        assert_eq!(land.masses.len(), map.land.len());
        for mass in &land.masses {
            assert!(
                mass.outline.len() >= 3,
                "{} has a degenerate outline",
                mass.name
            );
        }
        let start = projection.to_world(map.player_start.position);
        assert!(!land.contains(start), "player starts aground");
        for port in &map.ports {
            assert!(
                land.contains(projection.to_world(port.town)),
                "{} town is in the sea",
                port.name
            );
            assert!(
                !land.contains(projection.to_world(port.anchorage)),
                "{} anchorage is on land",
                port.name
            );
        }
    }

    #[test]
    fn every_label_sits_on_its_land_mass() {
        let (_, _, land) = port_royal();
        for mass in &land.masses {
            assert!(
                mass.contains(mass.label),
                "{} label is off its land",
                mass.name
            );
        }
    }

    #[test]
    fn land_contains_checks_every_mass() {
        let square = |offset: f32| {
            LandMass::new(
                "test",
                Terrain::Sand,
                vec![
                    Vec2::new(offset, 0.0),
                    Vec2::new(offset + 10.0, 0.0),
                    Vec2::new(offset + 10.0, 10.0),
                    Vec2::new(offset, 10.0),
                ],
                None,
            )
        };
        let land = Land {
            masses: vec![square(0.0), square(100.0)],
        };
        assert!(land.contains(Vec3::new(5.0, 0.0, 5.0)));
        assert!(
            land.contains(Vec3::new(105.0, 3.0, 5.0)),
            "height is ignored"
        );
        assert!(!land.contains(Vec3::new(50.0, 0.0, 5.0)));
    }

    #[test]
    fn leg_clearance_measures_distance_to_land() {
        let land = Land {
            masses: vec![LandMass::new(
                "test",
                Terrain::Sand,
                vec![
                    Vec2::ZERO,
                    Vec2::new(10.0, 0.0),
                    Vec2::new(10.0, 10.0),
                    Vec2::new(0.0, 10.0),
                ],
                None,
            )],
        };
        assert_eq!(
            land.clearance(Vec3::new(-5.0, 0.0, 5.0), Vec3::new(15.0, 0.0, 5.0)),
            0.0
        );
        assert_eq!(
            land.clearance(Vec3::new(5.0, 0.0, -5.0), Vec3::new(5.0, 0.0, 15.0)),
            0.0
        );
        let passing = land.clearance(Vec3::new(-5.0, 0.0, -5.0), Vec3::new(15.0, 0.0, -5.0));
        assert!((passing - 5.0).abs() < 1e-5);
        let point = land.clearance(Vec3::new(13.0, 0.0, 5.0), Vec3::new(13.0, 0.0, 5.0));
        assert!((point - 3.0).abs() < 1e-5);
        assert_eq!(
            Land::default().clearance(Vec3::ZERO, Vec3::X),
            f32::INFINITY
        );
    }
}
