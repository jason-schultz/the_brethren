//! Time compression: how fast the world runs against the wall clock.
//!
//! At high speeds the fixed simulation step grows as well as the step
//! rate. Running 5,400 game seconds every real second in 1/64 s steps would
//! need over 300,000 steps a second; with steps capped at [`MAX_STEP`] it
//! needs 2,700, which a handful of ships handles easily. The
//! simulation is tested at [`MAX_STEP`] so it holds up at full speed.

use std::time::Duration;

use bevy::prelude::*;

use crate::schedule::FrameSet;

/// The speeds the captain can choose, in game seconds per real second. The
/// fastest makes the 90-hour passage from Port Royal to Nassau in about a
/// minute.
pub const SPEEDS: [f64; 5] = [1.0, 10.0, 60.0, 600.0, 5400.0];
/// The fixed step at normal speed, seconds.
const BASE_STEP: f64 = 1.0 / 64.0;
/// The longest fixed step, seconds. A sloop moves at most about 13 m in one
/// step, well inside the arrival radius.
pub const MAX_STEP: f64 = 2.0;
/// The most real time one frame may cover (Bevy's default). A longer hitch,
/// such as dragging the window, is skipped rather than simulated.
const MAX_FRAME_TIME: Duration = Duration::from_millis(250);

/// How fast the world runs, in game seconds per real second. Pausing is
/// separate: it stops `Time<Virtual>` without forgetting the speed.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct TimeScale {
    pub speed: f64,
}

impl Default for TimeScale {
    fn default() -> Self {
        Self { speed: SPEEDS[0] }
    }
}

/// The fixed step for a speed: steps grow with speed once the step rate
/// would pass 64 a second, up to [`MAX_STEP`].
pub fn step_for(speed: f64) -> f64 {
    (BASE_STEP * speed).clamp(BASE_STEP, MAX_STEP)
}

/// The most game time one frame may add at a speed. Bevy caps each frame's
/// game time, after speeding it up, at [`MAX_FRAME_TIME`]. Left at that,
/// every speed above about 15× would run at about 15×.
pub fn max_frame_delta(speed: f64) -> Duration {
    MAX_FRAME_TIME.mul_f64(speed)
}

pub struct TimeScalePlugin;

impl Plugin for TimeScalePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimeScale>().add_systems(
            Update,
            // The default speed matches Bevy's default clocks, so nothing
            // needs applying until the speed actually changes.
            apply_time_scale
                .run_if(resource_changed::<TimeScale>)
                .run_if(not(resource_added::<TimeScale>))
                .in_set(FrameSet::Orders),
        );
    }
}

fn apply_time_scale(
    scale: Res<TimeScale>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut fixed_time: ResMut<Time<Fixed>>,
) {
    virtual_time.set_relative_speed_f64(scale.speed);
    virtual_time.set_max_delta(max_frame_delta(scale.speed));
    fixed_time.set_timestep_seconds(step_for(scale.speed));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sim_app;

    #[test]
    fn the_default_speed_matches_the_default_clocks() {
        let app = sim_app();
        assert_eq!(
            app.world()
                .resource::<Time<Fixed>>()
                .timestep()
                .as_secs_f64(),
            step_for(TimeScale::default().speed)
        );
    }

    #[test]
    fn steps_stay_small_until_the_step_rate_gets_high_then_cap() {
        assert_eq!(step_for(1.0), BASE_STEP);
        assert_eq!(step_for(10.0), 10.0 * BASE_STEP);
        assert_eq!(step_for(600.0), MAX_STEP);
        assert_eq!(step_for(SPEEDS[4]), MAX_STEP);
        assert_eq!(step_for(0.5), BASE_STEP, "never smaller than normal");
    }

    #[test]
    fn the_fastest_speed_needs_a_manageable_number_of_steps() {
        let steps_per_real_second = SPEEDS[4] / step_for(SPEEDS[4]);
        assert!(steps_per_real_second <= 3_000.0, "{steps_per_real_second}");
    }

    #[test]
    fn changing_speed_sets_virtual_speed_and_fixed_step() {
        let mut app = sim_app();
        app.update();
        app.world_mut().resource_mut::<TimeScale>().speed = 600.0;
        app.update();
        let world = app.world();
        assert_eq!(
            world.resource::<Time<Virtual>>().relative_speed_f64(),
            600.0
        );
        assert_eq!(
            world.resource::<Time<Fixed>>().timestep().as_secs_f64(),
            MAX_STEP
        );
        assert_eq!(
            world.resource::<Time<Virtual>>().max_delta(),
            max_frame_delta(600.0)
        );
    }

    #[test]
    fn a_frame_at_any_speed_can_cover_a_quarter_second_of_real_time() {
        for speed in SPEEDS {
            let frame = Duration::from_millis(250).mul_f64(speed);
            assert_eq!(max_frame_delta(speed), frame, "speed {speed}");
        }
    }
}
