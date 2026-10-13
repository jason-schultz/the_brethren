//! The on-disk map format (RON). Coordinates are real-world latitude/longitude.

use serde::Deserialize;

/// The Port Royal starting map, embedded at compile time.
pub const PORT_ROYAL: &str = include_str!("../../assets/maps/port_royal.ron");

/// `(latitude, longitude)` in degrees. North and east are positive.
pub type LatLon = (f64, f64);

#[derive(Deserialize, Debug, Clone)]
pub struct MapData {
    pub name: String,
    /// Real-world point placed at the world origin.
    pub origin: LatLon,
    /// World meters per real meter. Below 1.0 compresses the geography.
    pub distance_scale: f64,
    pub player_start: StartData,
    pub land: Vec<LandData>,
    pub ports: Vec<PortData>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StartData {
    pub position: LatLon,
    pub heading_degrees: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LandData {
    pub name: String,
    pub terrain: Terrain,
    /// Where to draw the name. Defaults to the outline's centroid.
    pub label: Option<LatLon>,
    pub outline: Vec<LatLon>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terrain {
    /// Bare sand: cays and spits.
    Sand,
    /// Wooded or cultivated high ground.
    Forest,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PortData {
    pub name: String,
    pub nation: String,
    /// The town itself, on land.
    pub town: LatLon,
    /// Where ships lie to dock, in water.
    pub anchorage: LatLon,
}

impl MapData {
    pub fn parse(source: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(source)
    }
}
