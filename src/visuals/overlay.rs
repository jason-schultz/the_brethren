//! Tactical overlay drawn over the world: routes, anchorages, and the wind.

use bevy::prelude::*;

use crate::math::heading_to_direction;
use crate::navigation::{ARRIVAL_RADIUS, Route};
use crate::player::Player;
use crate::port::Port;
use crate::wind::Wind;

/// Overlay height above the water, so lines don't z-fight the ocean.
const OVERLAY_Y: f32 = 0.5;
/// How far upwind of the player the wind arrow starts, meters.
const WIND_ARROW_LENGTH: f32 = 60.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (draw_routes, draw_anchorages, draw_wind));
}

fn flat_circle(center: Vec3) -> Isometry3d {
    Isometry3d::new(
        center.with_y(OVERLAY_Y),
        Quat::from_rotation_arc(Vec3::Z, Vec3::Y),
    )
}

fn draw_routes(ships: Query<(&Transform, &Route)>, mut gizmos: Gizmos) {
    let color = Color::srgb(1.0, 0.85, 0.3);
    for (transform, route) in &ships {
        let path = std::iter::once(transform.translation)
            .chain(route.waypoints.iter().map(|w| w.point))
            .map(|p| p.with_y(OVERLAY_Y));
        gizmos.linestrip(path, color.with_alpha(0.5));
        if let Some(destination) = route.waypoints.back() {
            gizmos.circle(flat_circle(destination.point), ARRIVAL_RADIUS, color);
        }
    }
}

fn draw_anchorages(ports: Query<&Transform, With<Port>>, mut gizmos: Gizmos) {
    for transform in &ports {
        gizmos.circle(
            flat_circle(transform.translation),
            ARRIVAL_RADIUS,
            Color::srgba(0.6, 1.0, 0.7, 0.6),
        );
    }
}

/// An arrow blowing past the player's ship, pointing the way the wind goes.
fn draw_wind(wind: Res<Wind>, player: Single<&Transform, With<Player>>, mut gizmos: Gizmos) {
    let downwind = -heading_to_direction(wind.from);
    let center = player.translation.with_y(OVERLAY_Y);
    gizmos.arrow(
        center - downwind * WIND_ARROW_LENGTH * 1.5,
        center - downwind * WIND_ARROW_LENGTH * 0.5,
        Color::srgb(0.6, 0.9, 1.0),
    );
}
