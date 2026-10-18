//! Everything drawn in the 3D world. Reads simulation state and never writes it.

pub mod influence;
mod labels;
mod land;
mod ocean;
mod overlay;
pub mod portrait;
mod ports;
mod ships;

use bevy::prelude::*;

use crate::factions::Nation;

/// The colour a nation is shown in: on the influence overlay, ship markers,
/// and the overlay's legend.
pub fn nation_colour(nation: Nation) -> Color {
    match nation {
        Nation::England => Color::srgb(0.9, 0.1, 0.1),
        Nation::Spain => Color::srgb(1.0, 0.8, 0.0),
        // Skull and bones: the black flag would vanish on a dark sea.
        Nation::Pirates => Color::srgb(0.95, 0.93, 0.85),
    }
}

pub struct VisualsPlugin;

impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ocean::plugin,
            land::plugin,
            ports::plugin,
            ships::plugin,
            overlay::plugin,
            labels::plugin,
            portrait::plugin,
            influence::plugin,
        ));
    }
}
