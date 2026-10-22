//! The geography of the world: land and the ports on it.
//!
//! Map content is data ([`data::MapData`]). [`load_map`] turns it into world
//! state: the [`Land`] resource and [`Port`] entities.

pub mod data;
pub mod projection;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::allegiance::Favour;
use crate::economy::market::{Market, MarketEntry};
use crate::geometry::{polygon_contains, segment_polygon_distance};
use crate::goods::Goods;
use crate::math::to_plane;
use crate::navigation::route::{NavGraph, route_length};
use crate::port::Port;
use data::{MapData, PortData, Terrain, TradeData};
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

    /// How far `at` is from the nearest land of `terrain`, in meters:
    /// infinite if there's none.
    pub fn distance_to(&self, at: Vec3, terrain: Terrain) -> f32 {
        self.masses
            .iter()
            .filter(|mass| mass.terrain == terrain)
            .map(|mass| mass.clearance(at, at))
            .fold(f32::INFINITY, f32::min)
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

/// The charts: the land, the routes round it, and the ports.
#[derive(SystemParam)]
pub struct Charts<'w, 's> {
    pub land: Res<'w, Land>,
    pub graph: Res<'w, NavGraph>,
    pub ports: Query<'w, 's, (Entity, &'static Port, &'static Transform)>,
}

impl Charts<'_, '_> {
    /// Port `port`'s anchorage.
    pub fn anchorage(&self, port: Entity) -> Option<Vec3> {
        self.ports.get(port).ok().map(|(.., at)| at.translation)
    }

    /// How far it is from `from` to `to` by sea, meters.
    pub fn passage(&self, from: Vec3, to: Vec3) -> f32 {
        route_length(from, &self.graph.plan(&self.land, from, to))
    }
}

/// Where and how the player's ship starts on a map: docked at a port.
#[derive(Debug, Clone, Copy)]
pub struct PlayerStart {
    pub port: Entity,
    /// The port's anchorage.
    pub position: Vec3,
    pub heading: f32,
}

/// Map data that refers to something that doesn't exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError {
    UnknownGood(String),
    UnknownStartPort(String),
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Land>();
    }
}

/// Replaces the world's geography with `map` and spawns its ports.
pub fn load_map(
    commands: &mut Commands,
    map: &MapData,
    goods: &Goods,
) -> Result<PlayerStart, MapError> {
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

    let mut start = None;
    for port in &map.ports {
        let anchorage = projection.to_world(port.anchorage);
        let mut favour = Favour::held_by(port.flag);
        favour.tier = port.tier;
        let entity = commands
            .spawn((
                Name::new(port.name.clone()),
                Port {
                    flag: port.flag,
                    town: projection.to_world(port.town),
                    locations: port.locations.clone(),
                },
                favour,
                market(port, goods)?,
                Transform::from_translation(anchorage),
            ))
            .id();
        if port.name == map.player_start.port {
            start = Some(PlayerStart {
                port: entity,
                position: anchorage,
                heading: map.player_start.heading_degrees.to_radians(),
            });
        }
    }
    start.ok_or_else(|| MapError::UnknownStartPort(map.player_start.port.clone()))
}

/// A port's opening market, from what it produces and needs.
fn market(port: &PortData, goods: &Goods) -> Result<Market, MapError> {
    let entry = |trade: &TradeData, sign: f64| {
        Ok(MarketEntry {
            good: goods
                .id(&trade.good)
                .ok_or_else(|| MapError::UnknownGood(trade.good.clone()))?,
            stock: trade.stock,
            target: trade.target,
            per_day: trade.per_day * sign,
            short_for: 0.0,
        })
    };
    let produced = port.produces.iter().map(|trade| entry(trade, 1.0));
    let needed = port.needs.iter().map(|trade| entry(trade, -1.0));
    Ok(Market {
        entries: produced.chain(needed).collect::<Result<_, _>>()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    use crate::geometry::segments_cross;

    fn west_indies() -> (MapData, GeoProjection, Land, World) {
        let map = MapData::parse(data::WEST_INDIES).expect("embedded map must parse");
        let projection = GeoProjection::new(map.origin, map.distance_scale);
        let mut world = World::new();
        world.insert_resource(Goods::standard());
        world
            .run_system_once_with(
                |map: In<MapData>, goods: Res<Goods>, mut commands: Commands| {
                    load_map(&mut commands, &map, &goods).expect("map refers to real things");
                },
                map.clone(),
            )
            .unwrap();
        let land = world.remove_resource::<Land>().unwrap();
        (map, projection, land, world)
    }

    #[test]
    fn coastlines_never_cross_themselves() {
        let (_, _, land, _) = west_indies();
        for mass in &land.masses {
            let n = mass.outline.len();
            let edge = |i: usize| (mass.outline[i], mass.outline[(i + 1) % n]);
            for i in 0..n {
                // Skip each edge's neighbors, which share a vertex with it.
                for j in (i + 2)..n {
                    if i == 0 && j == n - 1 {
                        continue;
                    }
                    let ((a, b), (c, d)) = (edge(i), edge(j));
                    assert!(
                        !segments_cross(a, b, c, d),
                        "{} crosses itself at edges {i} and {j}",
                        mass.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_port_market_is_well_formed() {
        let (_, _, _, mut world) = west_indies();
        let mut markets = world.query::<(&Name, &Market)>();
        assert_eq!(markets.iter(&world).count(), 5);
        for (name, market) in markets.iter(&world) {
            for (i, entry) in market.entries.iter().enumerate() {
                assert!(entry.target > 0.0, "{name}: zero target");
                assert!(entry.stock >= 0.0, "{name}: negative stock");
                assert!(
                    market.entries[..i].iter().all(|e| e.good != entry.good),
                    "{name} lists a good twice"
                );
            }
        }
    }

    #[test]
    fn unknown_goods_in_a_map_are_an_error() {
        let mut map = MapData::parse(data::WEST_INDIES).unwrap();
        map.ports[0].needs[0].good = "unobtainium".into();
        let mut world = World::new();
        let result = world
            .run_system_once_with(
                |map: In<MapData>, mut commands: Commands| {
                    load_map(&mut commands, &map, &Goods::standard()).map(|_| ())
                },
                map,
            )
            .unwrap();
        assert_eq!(result, Err(MapError::UnknownGood("unobtainium".into())));
    }

    #[test]
    fn west_indies_map_is_valid() {
        let (map, projection, land, _) = west_indies();
        assert_eq!(land.masses.len(), map.land.len());
        for mass in &land.masses {
            assert!(
                mass.outline.len() >= 3,
                "{} has a degenerate outline",
                mass.name
            );
        }
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
        let (_, _, land, _) = west_indies();
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
