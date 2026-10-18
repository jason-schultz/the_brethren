//! Pure gunnery: which broadside bears, how many shot hit, and what they do.
//! No ECS types here.

use bevy::prelude::*;

use crate::math::{angle_between, direction_to_heading};

/// Longest range a broadside is fired at, meters.
pub const GUN_RANGE: f32 = 400.0;
/// A broadside bears on targets within this angle of abeam.
pub const ARC: f32 = 35.0_f32.to_radians();
/// Seconds to reload a broadside.
pub const RELOAD: f32 = 30.0;
/// Fraction of shot that hit at point blank, falling to [`LONG_RANGE_HIT`]
/// at [`GUN_RANGE`].
const POINT_BLANK_HIT: f32 = 0.8;
const LONG_RANGE_HIT: f32 = 0.25;
/// What one hit does to a sloop-sized ship: timbers and rigging lost (as
/// fractions of sound), and men killed or wounded.
const TIMBERS_PER_HIT: f32 = 0.012;
const RIGGING_PER_HIT: f32 = 0.01;
const MEN_PER_HIT: f64 = 0.35;

/// One side of a ship's guns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Broadside {
    Port,
    Starboard,
}

/// The broadside that bears on `target` from a ship at `from` heading
/// `heading`, if either does.
pub fn bearing_broadside(heading: f32, from: Vec3, target: Vec3) -> Option<Broadside> {
    let relative = angle_between(heading, direction_to_heading(target - from));
    let off_abeam = |abeam: f32| angle_between(abeam, relative).abs() <= ARC;
    if off_abeam(90_f32.to_radians()) {
        Some(Broadside::Starboard)
    } else if off_abeam(-90_f32.to_radians()) {
        Some(Broadside::Port)
    } else {
        None
    }
}

/// Expected hits from a broadside of `guns` fired at `distance` meters.
pub fn hits(guns: u32, distance: f32) -> f32 {
    if distance > GUN_RANGE {
        return 0.0;
    }
    let accuracy = POINT_BLANK_HIT.lerp(LONG_RANGE_HIT, distance / GUN_RANGE);
    guns as f32 * accuracy
}

/// What `hits` do to the ship struck.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Damage {
    pub timbers: f32,
    pub rigging: f32,
    pub men: f64,
}

pub fn damage(hits: f32) -> Damage {
    Damage {
        timbers: hits * TIMBERS_PER_HIT,
        rigging: hits * RIGGING_PER_HIT,
        men: hits as f64 * MEN_PER_HIT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::heading_to_direction;

    fn target_at(heading: f32, relative_degrees: f32) -> Vec3 {
        heading_to_direction(heading + relative_degrees.to_radians()) * 200.0
    }

    #[test]
    fn a_target_abeam_is_under_that_broadside() {
        for heading in [0.0, 1.0, 4.0] {
            let bears =
                |degrees| bearing_broadside(heading, Vec3::ZERO, target_at(heading, degrees));
            assert_eq!(bears(90.0), Some(Broadside::Starboard));
            assert_eq!(bears(-90.0), Some(Broadside::Port));
            assert_eq!(
                bears(90.0 + 30.0),
                Some(Broadside::Starboard),
                "within the arc"
            );
            assert_eq!(bears(0.0), None, "dead ahead");
            assert_eq!(bears(180.0), None, "dead astern");
            assert_eq!(bears(-90.0 - 40.0), None, "outside the arc");
        }
    }

    #[test]
    fn closer_is_deadlier_and_out_of_range_misses() {
        assert!(hits(4, 50.0) > hits(4, 350.0));
        assert!(hits(8, 200.0) > hits(4, 200.0));
        assert_eq!(hits(4, GUN_RANGE + 1.0), 0.0);
        assert!((hits(10, 0.0) - 8.0).abs() < 1e-5);
    }

    #[test]
    fn damage_scales_with_hits() {
        assert_eq!(damage(0.0), Damage::default());
        let two = damage(2.0);
        assert!((two.timbers - 2.0 * damage(1.0).timbers).abs() < 1e-6);
        assert!(two.men > 0.0 && two.rigging > 0.0);
    }
}
