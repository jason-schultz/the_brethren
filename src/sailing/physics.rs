//! Pure sailing physics: points of sail, speed, and turning. No ECS types here.

use std::f32::consts::{FRAC_PI_2, PI};

use super::Hull;
use crate::math::{angle_between, normalize_heading};

/// Fraction of full rudder authority a ship keeps at zero speed. Real ships
/// barely steer without way on. Keeping a little lets a ship in irons recover.
pub const MIN_STEERAGE: f32 = 0.25;

/// Angle between the bow and the direction the wind comes from, in `[0, π]`.
/// `0` is head to wind and `π` is dead downwind. Port and starboard are symmetric.
pub fn true_wind_angle(heading: f32, wind_from: f32) -> f32 {
    angle_between(heading, wind_from).abs()
}

/// The hull's speed polar: the fraction of best speed achievable at a true wind angle.
///
/// Zero inside the no-go zone. Rises to `1.0` at the hull's best point of
/// sail, then falls off to `run_fraction` dead downwind.
pub fn polar_fraction(hull: &Hull, true_wind_angle: f32) -> f32 {
    if true_wind_angle <= hull.no_go_angle {
        0.0
    } else if true_wind_angle <= hull.best_angle {
        let t = (true_wind_angle - hull.no_go_angle) / (hull.best_angle - hull.no_go_angle);
        (t * FRAC_PI_2).sin()
    } else {
        let t = (true_wind_angle - hull.best_angle) / (PI - hull.best_angle);
        1.0 + (hull.run_fraction - 1.0) * t.min(1.0)
    }
}

/// Speed the hull settles at with all sail set, in m/s.
pub fn target_speed(hull: &Hull, true_wind_angle: f32, wind_speed_mps: f32) -> f32 {
    let wind_driven = wind_speed_mps * hull.wind_efficiency * polar_fraction(hull, true_wind_angle);
    wind_driven.min(hull.max_speed_mps)
}

/// Moves `current` toward `target` by at most `max_delta`.
pub fn approach(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}

/// Maximum turn rate in rad/s. Rudder authority grows with speed through the water.
pub fn turn_rate(hull: &Hull, speed_mps: f32) -> f32 {
    let speed_ratio = (speed_mps / hull.max_speed_mps).clamp(0.0, 1.0);
    hull.max_turn_rate * (MIN_STEERAGE + (1.0 - MIN_STEERAGE) * speed_ratio)
}

/// Turns `heading` toward `course` the short way, by at most `max_delta` radians.
pub fn turn_toward(heading: f32, course: f32, max_delta: f32) -> f32 {
    let turn = angle_between(heading, course).clamp(-max_delta, max_delta);
    normalize_heading(heading + turn)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;
    const SLOOP: Hull = Hull::SLOOP;

    #[test]
    fn no_headway_inside_the_no_go_zone() {
        for twa in [0.0, SLOOP.no_go_angle * 0.5, SLOOP.no_go_angle] {
            assert_eq!(target_speed(&SLOOP, twa, 10.0), 0.0, "twa {twa}");
        }
    }

    #[test]
    fn best_point_of_sail_gives_full_polar() {
        assert!((polar_fraction(&SLOOP, SLOOP.best_angle) - 1.0).abs() < EPS);
    }

    #[test]
    fn dead_run_uses_run_fraction() {
        assert!((polar_fraction(&SLOOP, PI) - SLOOP.run_fraction).abs() < EPS);
    }

    #[test]
    fn polar_never_exceeds_one_and_is_continuous() {
        let steps = 1000;
        let mut previous = polar_fraction(&SLOOP, 0.0);
        for i in 1..=steps {
            let twa = PI * i as f32 / steps as f32;
            let value = polar_fraction(&SLOOP, twa);
            assert!((0.0..=1.0).contains(&value), "twa {twa} gave {value}");
            assert!((value - previous).abs() < 0.02, "jump at twa {twa}");
            previous = value;
        }
    }

    #[test]
    fn speed_rises_from_close_hauled_to_best_angle() {
        let close = target_speed(&SLOOP, SLOOP.no_go_angle + 0.1, 7.0);
        let best = target_speed(&SLOOP, SLOOP.best_angle, 7.0);
        assert!(close > 0.0 && close < best);
    }

    #[test]
    fn speed_is_capped_by_hull_speed_in_strong_wind() {
        assert_eq!(
            target_speed(&SLOOP, SLOOP.best_angle, 100.0),
            SLOOP.max_speed_mps
        );
    }

    #[test]
    fn no_wind_no_speed() {
        assert_eq!(target_speed(&SLOOP, SLOOP.best_angle, 0.0), 0.0);
    }

    #[test]
    fn true_wind_angle_is_symmetric_port_and_starboard() {
        let wind_from = 1.0;
        let starboard = true_wind_angle(wind_from + 0.7, wind_from);
        let port = true_wind_angle(wind_from - 0.7, wind_from);
        assert!((starboard - port).abs() < EPS);
        assert!((starboard - 0.7).abs() < EPS);
    }

    #[test]
    fn true_wind_angle_running_downwind_is_pi() {
        assert!((true_wind_angle(0.0, PI) - PI).abs() < EPS);
    }

    #[test]
    fn approach_does_not_overshoot() {
        assert_eq!(approach(0.0, 1.0, 5.0), 1.0);
        assert_eq!(approach(0.0, 1.0, 0.25), 0.25);
        assert_eq!(approach(1.0, 0.0, 0.25), 0.75);
    }

    #[test]
    fn steering_is_weak_at_rest_and_full_at_speed() {
        assert!((turn_rate(&SLOOP, 0.0) - SLOOP.max_turn_rate * MIN_STEERAGE).abs() < EPS);
        assert!((turn_rate(&SLOOP, SLOOP.max_speed_mps) - SLOOP.max_turn_rate).abs() < EPS);
        assert!((turn_rate(&SLOOP, SLOOP.max_speed_mps * 3.0) - SLOOP.max_turn_rate).abs() < EPS);
    }

    #[test]
    fn turn_toward_is_limited_and_goes_the_short_way() {
        let heading = 350_f32.to_radians();
        let course = 20_f32.to_radians();
        let turned = turn_toward(heading, course, 10_f32.to_radians());
        assert!(
            angle_between(turned, 0.0).abs() < EPS,
            "should turn clockwise across north to 0°, got {turned}"
        );
        assert!(
            (turn_toward(heading, course, 1.0) - course).abs() < EPS,
            "reaches course without overshoot"
        );
    }
}
