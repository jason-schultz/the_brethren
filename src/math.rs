//! The single home for angle conventions and unit conversions.
//!
//! World convention: Y-up, ocean on the XZ plane, North = −Z, East = +X.
//! A *heading* is radians clockwise from north, normalized to `[0, 2π)`.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

/// Meters per second → knots.
pub const MPS_TO_KNOTS: f32 = 1.943_844;

/// Normalizes any angle to a heading in `[0, 2π)`.
pub fn normalize_heading(angle: f32) -> f32 {
    let wrapped = angle.rem_euclid(TAU);
    // `rem_euclid` can return exactly TAU for tiny negative inputs due to rounding.
    if wrapped >= TAU { 0.0 } else { wrapped }
}

/// Signed shortest rotation from heading `from` to heading `to`, in `(-π, π]`.
/// Positive means turning clockwise (to starboard).
pub fn angle_between(from: f32, to: f32) -> f32 {
    let diff = normalize_heading(to - from);
    if diff > PI { diff - TAU } else { diff }
}

/// Unit vector on the ocean plane pointing along `heading`.
pub fn heading_to_direction(heading: f32) -> Vec3 {
    Vec3::new(heading.sin(), 0.0, -heading.cos())
}

/// Heading of a vector on the ocean plane (Y is ignored).
pub fn direction_to_heading(direction: Vec3) -> f32 {
    normalize_heading(direction.x.atan2(-direction.z))
}

/// A world position projected onto the ocean plane as `(x, z)`.
pub fn to_plane(position: Vec3) -> Vec2 {
    Vec2::new(position.x, position.z)
}

/// A point on the ocean plane back in world space, at sea level.
pub fn from_plane(point: Vec2) -> Vec3 {
    Vec3::new(point.x, 0.0, point.y)
}

/// World rotation for a model whose bow faces Bevy's forward (−Z).
pub fn heading_to_rotation(heading: f32) -> Quat {
    // Positive Y rotation is counter-clockwise seen from above; headings are clockwise.
    Quat::from_rotation_y(-heading)
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;

    const EPS: f32 = 1e-5;

    #[test]
    fn compass_directions_map_to_world_axes() {
        assert!(
            heading_to_direction(0.0).abs_diff_eq(Vec3::NEG_Z, EPS),
            "north is -Z"
        );
        assert!(
            heading_to_direction(FRAC_PI_2).abs_diff_eq(Vec3::X, EPS),
            "east is +X"
        );
        assert!(
            heading_to_direction(PI).abs_diff_eq(Vec3::Z, EPS),
            "south is +Z"
        );
        assert!(
            heading_to_direction(3.0 * FRAC_PI_2).abs_diff_eq(Vec3::NEG_X, EPS),
            "west is -X"
        );
    }

    #[test]
    fn direction_and_heading_round_trip() {
        for degrees in (0..360).step_by(15) {
            let heading = (degrees as f32).to_radians();
            let back = direction_to_heading(heading_to_direction(heading));
            assert!(
                angle_between(heading, back).abs() < EPS,
                "{degrees}° round trip gave {back}"
            );
        }
    }

    #[test]
    fn direction_to_heading_ignores_height() {
        let heading = direction_to_heading(Vec3::new(1.0, 50.0, 0.0));
        assert!((heading - FRAC_PI_2).abs() < EPS);
    }

    #[test]
    fn rotation_points_model_forward_along_heading() {
        for degrees in [0.0_f32, 45.0, 90.0, 200.0, 315.0] {
            let heading = degrees.to_radians();
            let forward = heading_to_rotation(heading) * Vec3::NEG_Z;
            assert!(
                forward.abs_diff_eq(heading_to_direction(heading), EPS),
                "{degrees}°"
            );
        }
    }

    #[test]
    fn plane_round_trip_drops_height() {
        let world = Vec3::new(3.0, 7.0, -4.0);
        assert_eq!(to_plane(world), Vec2::new(3.0, -4.0));
        assert_eq!(from_plane(to_plane(world)), Vec3::new(3.0, 0.0, -4.0));
    }

    #[test]
    fn normalize_heading_wraps_into_range() {
        assert!((normalize_heading(-FRAC_PI_2) - 3.0 * FRAC_PI_2).abs() < EPS);
        assert!((normalize_heading(TAU + 1.0) - 1.0).abs() < EPS);
        assert_eq!(normalize_heading(-1e-9), 0.0);
        assert_eq!(normalize_heading(TAU), 0.0);
    }

    #[test]
    fn angle_between_takes_the_short_way_across_north() {
        let from = 350_f32.to_radians();
        let to = 10_f32.to_radians();
        assert!(
            (angle_between(from, to) - 20_f32.to_radians()).abs() < EPS,
            "clockwise across north"
        );
        assert!(
            (angle_between(to, from) + 20_f32.to_radians()).abs() < EPS,
            "counter-clockwise back"
        );
    }

    #[test]
    fn angle_between_opposite_headings_is_half_turn() {
        assert!((angle_between(0.0, PI).abs() - PI).abs() < EPS);
    }
}
