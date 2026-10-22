//! The on-disk map format (RON). Coordinates are real-world latitude/longitude.

use serde::Deserialize;

use crate::allegiance::Tier;
use crate::factions::Nation;
use crate::port::Location;

/// The West Indies starting map, embedded at compile time.
pub const WEST_INDIES: &str = include_str!("../../assets/maps/west_indies.ron");

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

/// The player starts docked at a port.
#[derive(Deserialize, Debug, Clone)]
pub struct StartData {
    /// Name of one of the map's ports.
    pub port: String,
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
    /// Banks too shallow to sail over. Ships ground on them as on land.
    Shoal,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PortData {
    pub name: String,
    /// The flag the port flies.
    pub flag: Nation,
    /// The town itself, on land.
    pub town: LatLon,
    /// Where ships lie to dock, in water.
    pub anchorage: LatLon,
    pub locations: Vec<Location>,
    /// How firmly it's held: a colony, unless the map says otherwise.
    #[serde(default)]
    pub tier: Tier,
    /// Goods the port supplies, whether grown, made, or shipped in from off the map.
    pub produces: Vec<TradeData>,
    /// Goods the port uses up and must have delivered.
    pub needs: Vec<TradeData>,
}

/// One good in a port's market.
#[derive(Deserialize, Debug, Clone)]
pub struct TradeData {
    /// A key from the goods data.
    pub good: String,
    /// Stock the market wants on hand.
    pub target: f64,
    /// Units made (for `produces`) or used (for `needs`) per day.
    pub per_day: f64,
    /// Stock at the start of the game.
    pub stock: f64,
}

impl MapData {
    pub fn parse(source: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(source)
    }
}
