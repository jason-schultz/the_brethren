//! Shared helpers for headless simulation tests.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use crate::SimulationPlugin;

/// A headless app running the simulation. Each `update()` advances exactly one fixed step.
pub fn sim_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, SimulationPlugin))
        .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    app
}

/// Advances the app by `seconds` of simulated time.
pub fn run_seconds(app: &mut App, seconds: f32) {
    run_checked(app, seconds, |_| {});
}

/// Advances the app by `seconds`, calling `check` with the world after every fixed step.
pub fn run_checked(app: &mut App, seconds: f32, mut check: impl FnMut(&World)) {
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    for _ in 0..(seconds / step).ceil() as u32 {
        app.update();
        check(app.world());
    }
}
