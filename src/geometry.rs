//! 2D polygon geometry on the ocean plane. Polygons are slices of vertices in
//! `(x, z)` plane coordinates (see [`crate::math::to_plane`]). They are
//! implicitly closed, and either winding order is accepted.

use bevy::prelude::*;

/// Twice the signed area. Positive when the vertices wind counter-clockwise in
/// `(x, z)` coordinates, which is clockwise when seen from above.
pub fn signed_area_doubled(polygon: &[Vec2]) -> f32 {
    edges(polygon).map(|(a, b)| a.perp_dot(b)).sum()
}

/// Whether `point` lies inside `polygon` (even-odd rule).
pub fn polygon_contains(polygon: &[Vec2], point: Vec2) -> bool {
    let mut inside = false;
    for (a, b) in edges(polygon) {
        if (a.y > point.y) != (b.y > point.y) {
            let crossing_x = a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if point.x < crossing_x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Whether segments `p1–p2` and `q1–q2` properly cross. Touching at an
/// endpoint or being collinear does not count.
pub fn segments_cross(p1: Vec2, p2: Vec2, q1: Vec2, q2: Vec2) -> bool {
    let d1 = (p2 - p1).perp_dot(q1 - p1);
    let d2 = (p2 - p1).perp_dot(q2 - p1);
    let d3 = (q2 - q1).perp_dot(p1 - q1);
    let d4 = (q2 - q1).perp_dot(p2 - q1);
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Whether the segment `from–to` touches the inside of `polygon`: it crosses
/// an edge, or its start, end, or midpoint is inside. The midpoint check
/// catches segments that slip in exactly through a vertex.
pub fn segment_enters_polygon(polygon: &[Vec2], from: Vec2, to: Vec2) -> bool {
    [from, to, (from + to) / 2.0]
        .iter()
        .any(|&p| polygon_contains(polygon, p))
        || edges(polygon).any(|(a, b)| segments_cross(from, to, a, b))
}

/// Distance from `point` to the segment `a–b`.
pub fn point_segment_distance(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((point - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    point.distance(a + ab * t)
}

/// Shortest distance between segments `p1–p2` and `q1–q2`. Zero if they cross.
pub fn segment_distance(p1: Vec2, p2: Vec2, q1: Vec2, q2: Vec2) -> f32 {
    if segments_cross(p1, p2, q1, q2) {
        return 0.0;
    }
    point_segment_distance(p1, q1, q2)
        .min(point_segment_distance(p2, q1, q2))
        .min(point_segment_distance(q1, p1, p2))
        .min(point_segment_distance(q2, p1, p2))
}

/// How close the segment `from–to` comes to `polygon`. Zero if it enters it.
pub fn segment_polygon_distance(polygon: &[Vec2], from: Vec2, to: Vec2) -> f32 {
    if segment_enters_polygon(polygon, from, to) {
        return 0.0;
    }
    edges(polygon)
        .map(|(a, b)| segment_distance(from, to, a, b))
        .fold(f32::INFINITY, f32::min)
}

/// Points just outside each convex corner of `polygon`, pushed outward by
/// `clearance`. Shortest paths around an obstacle only bend at convex corners.
pub fn convex_corners_offset(polygon: &[Vec2], clearance: f32) -> Vec<Vec2> {
    let n = polygon.len();
    // Normalize so "outward" is to the right of each edge: walk counter-clockwise.
    let winding = signed_area_doubled(polygon).signum();
    (0..n)
        .filter_map(|i| {
            let prev = polygon[(i + n - 1) % n];
            let corner = polygon[i];
            let next = polygon[(i + 1) % n];
            let incoming = (corner - prev).normalize_or_zero();
            let outgoing = (next - corner).normalize_or_zero();
            // Convex when the walk turns left (for counter-clockwise winding).
            if incoming.perp_dot(outgoing) * winding <= 0.0 {
                return None;
            }
            // Outward normals are to the right of each edge for CCW winding.
            let right = |v: Vec2| Vec2::new(v.y, -v.x) * winding;
            let bisector = (right(incoming) + right(outgoing)).normalize_or_zero();
            Some(corner + bisector * clearance)
        })
        .collect()
}

/// Consecutive vertex pairs, including the closing edge.
fn edges(polygon: &[Vec2]) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
    polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .map(|(a, b)| (*a, *b))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10×10 square, counter-clockwise in (x, z).
    fn square() -> Vec<Vec2> {
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ]
    }

    /// An L shape: the square with its top-right quadrant cut out.
    fn l_shape() -> Vec<Vec2> {
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 5.0),
            Vec2::new(5.0, 5.0),
            Vec2::new(5.0, 10.0),
            Vec2::new(0.0, 10.0),
        ]
    }

    fn reversed(mut polygon: Vec<Vec2>) -> Vec<Vec2> {
        polygon.reverse();
        polygon
    }

    #[test]
    fn signed_area_reflects_winding() {
        assert_eq!(signed_area_doubled(&square()), 200.0);
        assert_eq!(signed_area_doubled(&reversed(square())), -200.0);
        assert_eq!(signed_area_doubled(&l_shape()), 150.0);
    }

    #[test]
    fn contains_inside_and_outside_points() {
        let square = square();
        assert!(polygon_contains(&square, Vec2::new(5.0, 5.0)));
        assert!(!polygon_contains(&square, Vec2::new(15.0, 5.0)));
        assert!(!polygon_contains(&square, Vec2::new(-1.0, -1.0)));
    }

    #[test]
    fn contains_respects_concavity() {
        let l = l_shape();
        assert!(polygon_contains(&l, Vec2::new(2.0, 8.0)));
        assert!(
            !polygon_contains(&l, Vec2::new(8.0, 8.0)),
            "the notch is water"
        );
    }

    #[test]
    fn contains_ignores_winding() {
        assert!(polygon_contains(&reversed(l_shape()), Vec2::new(2.0, 8.0)));
    }

    #[test]
    fn crossing_segments_are_detected() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(10.0, 10.0);
        assert!(segments_cross(
            a,
            b,
            Vec2::new(0.0, 10.0),
            Vec2::new(10.0, 0.0)
        ));
        assert!(!segments_cross(
            a,
            b,
            Vec2::new(20.0, 0.0),
            Vec2::new(30.0, 10.0)
        ));
        assert!(
            !segments_cross(a, b, b, Vec2::new(20.0, 0.0)),
            "shared endpoint is not a crossing"
        );
    }

    #[test]
    fn segment_through_polygon_enters_it() {
        let square = square();
        assert!(segment_enters_polygon(
            &square,
            Vec2::new(-5.0, 5.0),
            Vec2::new(15.0, 5.0)
        ));
        assert!(!segment_enters_polygon(
            &square,
            Vec2::new(-5.0, -5.0),
            Vec2::new(15.0, -5.0)
        ));
    }

    #[test]
    fn segment_ending_inside_enters() {
        assert!(segment_enters_polygon(
            &square(),
            Vec2::new(-5.0, 5.0),
            Vec2::new(5.0, 5.0)
        ));
    }

    #[test]
    fn segment_slipping_in_through_a_vertex_enters() {
        // Aimed exactly through the (0, 0) corner toward the center.
        assert!(segment_enters_polygon(
            &square(),
            Vec2::new(-5.0, -5.0),
            Vec2::new(5.0, 5.0)
        ));
    }

    #[test]
    fn point_segment_distance_handles_ends_and_middle() {
        let (a, b) = (Vec2::ZERO, Vec2::new(10.0, 0.0));
        assert_eq!(point_segment_distance(Vec2::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(point_segment_distance(Vec2::new(-4.0, 3.0), a, b), 5.0);
        assert_eq!(
            point_segment_distance(Vec2::new(1.0, 1.0), a, a),
            2_f32.sqrt()
        );
    }

    #[test]
    fn segment_distance_is_zero_when_crossing() {
        let d = segment_distance(
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
            Vec2::new(10.0, 0.0),
        );
        assert_eq!(d, 0.0);
    }

    #[test]
    fn segment_distance_between_parallel_segments() {
        let d = segment_distance(
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
            Vec2::new(0.0, 4.0),
            Vec2::new(10.0, 4.0),
        );
        assert_eq!(d, 4.0);
    }

    #[test]
    fn segment_polygon_distance_measures_the_gap() {
        let square = square();
        let passing =
            segment_polygon_distance(&square, Vec2::new(-5.0, -3.0), Vec2::new(15.0, -3.0));
        assert!((passing - 3.0).abs() < 1e-5);
        assert_eq!(
            segment_polygon_distance(&square, Vec2::new(-5.0, 5.0), Vec2::new(15.0, 5.0)),
            0.0
        );
    }

    #[test]
    fn convex_corners_are_offset_outside_for_either_winding() {
        for polygon in [square(), reversed(square())] {
            let corners = convex_corners_offset(&polygon, 2.0);
            assert_eq!(corners.len(), 4);
            for corner in corners {
                assert!(!polygon_contains(&polygon, corner), "{corner} is inside");
                // Diagonal offset from a square corner.
                let nearest = polygon
                    .iter()
                    .map(|v| v.distance(corner))
                    .fold(f32::MAX, f32::min);
                assert!((nearest - 2.0).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn reflex_corner_is_skipped() {
        let corners = convex_corners_offset(&l_shape(), 1.0);
        assert_eq!(corners.len(), 5, "the inner corner of the L is reflex");
        assert!(
            corners
                .iter()
                .all(|c| c.distance(Vec2::new(5.0, 5.0)) > 1.0)
        );
    }
}
