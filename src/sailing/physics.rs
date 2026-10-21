//! Pure sailing physics: wind effect on speed, and turning. No ECS types here.

use super::Hull;
use crate::math::{angle_between, normalize_heading};

/// Fraction of full rudder authority a ship keeps at zero speed. Real ships
/// barely steer without way on. Keeping a little lets a ship turn while
/// getting under way.
pub const MIN_STEERAGE: f32 = 0.25;

/// Angle between the bow and the direction the wind comes from, in `[0, π]`.
/// `0` is head to wind and `π` is dead downwind. Port and starboard are symmetric.
pub fn true_wind_angle(heading: f32, wind_from: f32) -> f32 {
    angle_between(heading, wind_from).abs()
}

/// Speed multiplier at a true wind angle. A game simplification rather than
/// a speed polar: ships sail any heading, a little slower into the wind and a
/// little faster with it. `headwind_factor` head to wind, `1.0` with the wind
/// on the beam, `tailwind_factor` dead downwind, and smooth in between.
pub fn wind_factor(hull: &Hull, true_wind_angle: f32) -> f32 {
    let into_wind = true_wind_angle.cos();
    let extreme = if into_wind >= 0.0 {
        hull.headwind_factor
    } else {
        hull.tailwind_factor
    };
    1.0 + (extreme - 1.0) * into_wind.abs()
}

/// Speed lost with a full hold. A laden merchantman can't outrun a raider
/// sailing light.
pub const LADEN_DRAG: f32 = 0.25;

/// Speed multiplier for a hold `laden` full, from 0 (empty) to 1 (full).
pub fn laden_factor(laden: f32) -> f32 {
    1.0 - LADEN_DRAG * laden.clamp(0.0, 1.0)
}

/// What a ship makes of `speed` m/s under all sail, as handled by her
/// crew, with her rigging as it is and her hold `laden` full.
pub fn sail_speed(speed: f32, handling: f32, rigging: f32, laden: f32) -> f32 {
    speed * handling * rigging * laden_factor(laden)
}

/// Speed the hull settles at with all sail set, in m/s.
pub fn target_speed(hull: &Hull, true_wind_angle: f32) -> f32 {
    hull.speed_mps * wind_factor(hull, true_wind_angle)
}

/// Moves `current` toward `target` by at most `max_delta`.
pub fn approach(current: f32, target: f32, max_delta: f32) -> f32 {
    current + (target - current).clamp(-max_delta, max_delta)
}

/// Maximum turn rate in rad/s. Rudder authority grows with speed through the water.
pub fn turn_rate(hull: &Hull, speed_mps: f32) -> f32 {
    let speed_ratio = (speed_mps / hull.speed_mps).clamp(0.0, 1.0);
    hull.max_turn_rate * (MIN_STEERAGE + (1.0 - MIN_STEERAGE) * speed_ratio)
}

/// Turns `heading` toward `course` the short way, by at most `max_delta` radians.
pub fn turn_toward(heading: f32, course: f32, max_delta: f32) -> f32 {
    let turn = angle_between(heading, course).clamp(-max_delta, max_delta);
    normalize_heading(heading + turn)
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use super::*;
    use crate::ship_class::ShipClass;

    const EPS: f32 = 1e-5;

    fn sloop() -> Hull {
        ShipClass::sloop().hull
    }

    #[test]
    fn wind_factor_hits_its_three_anchors() {
        assert!((wind_factor(&sloop(), 0.0) - sloop().headwind_factor).abs() < EPS);
        assert!((wind_factor(&sloop(), FRAC_PI_2) - 1.0).abs() < EPS);
        assert!((wind_factor(&sloop(), PI) - sloop().tailwind_factor).abs() < EPS);
    }

    #[test]
    fn wind_factor_rises_steadily_from_headwind_to_tailwind() {
        let steps = 1000;
        let mut previous = wind_factor(&sloop(), 0.0);
        for i in 1..=steps {
            let twa = PI * i as f32 / steps as f32;
            let value = wind_factor(&sloop(), twa);
            assert!(value >= previous, "dipped at twa {twa}");
            assert!(value - previous < 0.01, "jump at twa {twa}");
            previous = value;
        }
    }

    #[test]
    fn every_heading_makes_headway() {
        assert!(target_speed(&sloop(), 0.0) > 0.0);
        assert!(target_speed(&sloop(), 0.0) < target_speed(&sloop(), PI));
    }

    #[test]
    fn a_full_hold_slows_a_ship() {
        assert_eq!(laden_factor(0.0), 1.0);
        assert!((laden_factor(1.0) - (1.0 - LADEN_DRAG)).abs() < EPS);
        assert!(laden_factor(0.5) > laden_factor(1.0));
        assert_eq!(
            laden_factor(3.0),
            laden_factor(1.0),
            "overloading is clamped"
        );
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
        assert!((turn_rate(&sloop(), 0.0) - sloop().max_turn_rate * MIN_STEERAGE).abs() < EPS);
        assert!((turn_rate(&sloop(), sloop().speed_mps) - sloop().max_turn_rate).abs() < EPS);
        assert!((turn_rate(&sloop(), sloop().speed_mps * 3.0) - sloop().max_turn_rate).abs() < EPS);
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
