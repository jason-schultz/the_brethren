//! The Brethren: an Age-of-Sail sandbox.
//!
//! [`SimulationPlugin`] is the headless world (safe for tests). [`GamePlugin`]
//! adds input and presentation on top and requires Bevy's `DefaultPlugins`.

pub mod camera;
pub mod geometry;
pub mod map;
pub mod math;
pub mod navigation;
pub mod new_game;
pub mod player;
pub mod port;
pub mod sailing;
pub mod schedule;
pub mod time_control;
pub mod ui;
pub mod visuals;
pub mod wind;

#[cfg(test)]
mod test_support;

use bevy::prelude::*;

/// The world simulation, with no rendering, windowing, or input.
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        schedule::configure_sets(app);
        app.add_plugins((
            wind::WindPlugin,
            map::MapPlugin,
            sailing::SailingPlugin,
            navigation::NavigationPlugin,
            port::PortPlugin,
        ));
    }
}

/// The full game: simulation plus player input and presentation.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            SimulationPlugin,
            new_game::NewGamePlugin,
            time_control::TimeControlPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            visuals::VisualsPlugin,
            ui::UiPlugin,
        ));
    }
}
