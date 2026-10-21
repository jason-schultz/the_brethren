//! Everything drawn in the 3D world. Reads simulation state and never writes it.

pub mod influence;
mod labels;
mod land;
mod ocean;
mod overlay;
pub mod portrait;
mod ports;
mod ships;
mod wrecks;

use bevy::prelude::*;

use crate::factions::Nation;

/// The colour a nation is shown in: on the influence overlay, ship markers,
/// and the overlay's legend.
pub fn nation_colour(nation: Nation) -> Color {
    let (red, green, blue) = nation.colour();
    Color::srgb(red, green, blue)
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
            wrecks::plugin,
        ));
    }
}
