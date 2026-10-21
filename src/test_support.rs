//! Shared helpers for headless simulation tests.

use bevy::prelude::*;

/// A headless app running the simulation. Each `update()` advances exactly one fixed step.
pub use crate::stepped_app as sim_app;

/// Advances the app by `seconds` of simulated time.
pub fn run_seconds(app: &mut App, seconds: f32) {
    run_checked(app, seconds, |_| {});
}

/// Advances the app by `seconds`, calling `check` with the world after every fixed step.
pub fn run_checked(app: &mut App, seconds: f32, mut check: impl FnMut(&World)) {
    run_until(app, seconds, |world| {
        check(world);
        false
    });
}

/// Advances the app one fixed step at a time until `done` returns true or
/// `seconds` have passed. Returns whether `done` was reached.
pub fn run_until(app: &mut App, seconds: f32, mut done: impl FnMut(&World) -> bool) -> bool {
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    for _ in 0..(seconds / step).ceil() as u32 {
        app.update();
        if done(app.world()) {
            return true;
        }
    }
    false
}
