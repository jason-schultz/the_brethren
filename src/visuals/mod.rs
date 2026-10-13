//! Everything drawn in the 3D world. Reads simulation state and never writes it.

mod labels;
mod land;
mod ocean;
mod overlay;
mod ports;
mod ships;

use bevy::prelude::*;

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
        ));
    }
}
