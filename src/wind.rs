//! The wind. It is global and constant for now; weather systems will drive it later.

use bevy::prelude::*;

/// The true wind over the whole map.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Wind {
    /// Heading the wind blows *from*, in radians (meteorological convention).
    pub from: f32,
    /// True wind speed in meters per second.
    pub speed_mps: f32,
}

impl Default for Wind {
    /// The Caribbean trade winds: east-northeast at about 14 knots.
    fn default() -> Self {
        Self {
            from: 67.5_f32.to_radians(),
            speed_mps: 7.0,
        }
    }
}

pub struct WindPlugin;

impl Plugin for WindPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wind>();
    }
}
