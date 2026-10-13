//! Pure course selection: how a crew gets toward a bearing given the wind.

use crate::math::{angle_between, normalize_heading};
use crate::sailing::physics::true_wind_angle;

/// How far outside the no-go zone a crew sails when close-hauled, so the sails
/// draw properly instead of luffing at the edge.
pub const CLOSE_HAULED_MARGIN: f32 = 15.0_f32.to_radians();

/// Widest tacking corridor (meters either side of the leg) in open water.
/// Longer boards waste less speed going about.
pub const MAX_TACKING_CORRIDOR: f32 = 300.0;
/// Narrowest corridor. Below this, a sloop spends all its time tacking.
pub const MIN_TACKING_CORRIDOR: f32 = 15.0;
/// Room left between the corridor's edge and the land, for the overshoot
/// while a ship turns through the wind.
pub const TURNING_ROOM: f32 = 40.0;

/// How far either side of a leg a ship may stray while beating, given how
/// close that leg comes to land.
pub fn tacking_corridor(leg_clearance: f32) -> f32 {
    (leg_clearance - TURNING_ROOM).clamp(MIN_TACKING_CORRIDOR, MAX_TACKING_CORRIDOR)
}

/// The course to steer toward `bearing`, beating to windward when needed.
///
/// If the bearing can be sailed directly, it is returned unchanged.
/// Otherwise the ship sails close-hauled. It holds the tack nearest its
/// current heading until it strays more than `corridor` meters from the
/// leg, then tacks back across.
///
/// `cross_track` is the signed distance from the leg in meters, positive to
/// starboard of the track.
pub fn course_to(
    bearing: f32,
    wind_from: f32,
    no_go_angle: f32,
    heading: f32,
    cross_track: f32,
    corridor: f32,
) -> f32 {
    let close_hauled = no_go_angle + CLOSE_HAULED_MARGIN;
    if true_wind_angle(bearing, wind_from) >= close_hauled {
        return bearing;
    }
    let starboard_tack = normalize_heading(wind_from - close_hauled);
    let port_tack = normalize_heading(wind_from + close_hauled);
    // The two tacks straddle the bearing: one leads to its left, one to its right.
    let (left_of_track, right_of_track) = if angle_between(bearing, starboard_tack) < 0.0 {
        (starboard_tack, port_tack)
    } else {
        (port_tack, starboard_tack)
    };
    if cross_track > corridor {
        left_of_track
    } else if cross_track < -corridor {
        right_of_track
    } else if true_wind_angle(heading, starboard_tack) <= true_wind_angle(heading, port_tack) {
        starboard_tack
    } else {
        port_tack
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;
    const NO_GO: f32 = 50.0_f32.to_radians();
    const CLOSE_HAULED: f32 = NO_GO + CLOSE_HAULED_MARGIN;
    const CORRIDOR: f32 = 100.0;

    fn deg(degrees: f32) -> f32 {
        degrees.to_radians()
    }

    #[test]
    fn sailable_bearing_is_steered_directly() {
        // Wind from the north; east is a beam reach and south is a run.
        assert_eq!(
            course_to(deg(90.0), 0.0, NO_GO, 0.0, 0.0, CORRIDOR),
            deg(90.0)
        );
        assert_eq!(
            course_to(deg(180.0), 0.0, NO_GO, 0.0, 0.0, CORRIDOR),
            deg(180.0)
        );
    }

    #[test]
    fn sailable_bearing_ignores_cross_track() {
        assert_eq!(
            course_to(deg(90.0), 0.0, NO_GO, 0.0, 1000.0, CORRIDOR),
            deg(90.0)
        );
    }

    #[test]
    fn upwind_bearing_is_beaten_to_close_hauled() {
        let course = course_to(0.0, 0.0, NO_GO, deg(80.0), 0.0, CORRIDOR);
        assert!((true_wind_angle(course, 0.0) - CLOSE_HAULED).abs() < EPS);
    }

    #[test]
    fn keeps_the_tack_nearest_current_heading_inside_the_corridor() {
        // Heading east of north → port tack (wind over the port bow).
        let east = course_to(0.0, 0.0, NO_GO, deg(70.0), 0.0, CORRIDOR);
        assert!((east - CLOSE_HAULED).abs() < EPS);
        // Heading west of north → starboard tack.
        let west = course_to(0.0, 0.0, NO_GO, deg(290.0), 0.0, CORRIDOR);
        assert!((west - normalize_heading(-CLOSE_HAULED)).abs() < EPS);
    }

    #[test]
    fn tacks_back_when_straying_out_of_the_corridor() {
        // Beating north with wind from the north, currently on port tack (heading east of north).
        let heading = CLOSE_HAULED;
        let strayed_right = course_to(0.0, 0.0, NO_GO, heading, CORRIDOR + 1.0, CORRIDOR);
        assert!(
            angle_between(0.0, strayed_right) < 0.0,
            "right of track must steer left (west of north), got {}°",
            strayed_right.to_degrees()
        );
        let strayed_left = course_to(
            0.0,
            0.0,
            NO_GO,
            normalize_heading(-heading),
            -CORRIDOR - 1.0,
            CORRIDOR,
        );
        assert!(
            angle_between(0.0, strayed_left) > 0.0,
            "left of track must steer right"
        );
    }

    #[test]
    fn corridor_narrows_near_land_and_is_capped_in_open_water() {
        assert_eq!(tacking_corridor(f32::INFINITY), MAX_TACKING_CORRIDOR);
        assert_eq!(tacking_corridor(140.0), 100.0);
        assert_eq!(tacking_corridor(10.0), MIN_TACKING_CORRIDOR);
    }

    #[test]
    fn bearing_just_inside_close_hauled_still_beats() {
        let bearing = CLOSE_HAULED - deg(1.0);
        let course = course_to(bearing, 0.0, NO_GO, bearing, 0.0, CORRIDOR);
        assert!(
            (course - CLOSE_HAULED).abs() < EPS,
            "should sail the close-hauled edge, not pinch"
        );
    }

    #[test]
    fn works_for_wind_from_any_direction() {
        let wind_from = deg(250.0);
        let course = course_to(wind_from, wind_from, NO_GO, deg(200.0), 0.0, CORRIDOR);
        assert!((true_wind_angle(course, wind_from) - CLOSE_HAULED).abs() < EPS);
    }
}
