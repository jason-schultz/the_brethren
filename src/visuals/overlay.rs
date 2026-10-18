//! Tactical overlay drawn over the world: routes, anchorages, and the wind.

use bevy::prelude::*;

use super::nation_colour;
use crate::camera::CameraRig;
use crate::combat::Volley;
use crate::factions::Colours;
use crate::math::heading_to_direction;
use crate::navigation::{ARRIVAL_RADIUS, Route};
use crate::player::Player;
use crate::port::Port;
use crate::sailing::Hull;
use crate::wind::Wind;

/// Overlay height above the water, so lines don't z-fight the ocean.
const OVERLAY_Y: f32 = 0.5;
/// How far upwind of the player the wind arrow starts, meters.
const WIND_ARROW_LENGTH: f32 = 60.0;
/// Ship markers are drawn this fraction of the camera distance across, so
/// ships stay visible when zoomed out to the chart.
const MARKER_SCALE: f32 = 0.012;

/// How long a broadside's line of shot stays on screen, real seconds.
const VOLLEY_SECONDS: f32 = 0.6;

/// Broadsides recently fired, with the real time each was seen.
#[derive(Resource, Debug, Default)]
struct Volleys(Vec<(Volley, f32)>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Volleys>().add_systems(
        Update,
        (
            draw_routes,
            draw_anchorages,
            draw_wind,
            draw_ship_markers,
            draw_volleys,
        ),
    );
}

/// A fading line from each broadside to her target.
fn draw_volleys(
    mut fired: MessageReader<Volley>,
    mut volleys: ResMut<Volleys>,
    time: Res<Time<Real>>,
    mut gizmos: Gizmos,
) {
    let now = time.elapsed_secs();
    volleys.0.extend(fired.read().map(|&volley| (volley, now)));
    volleys.0.retain(|(_, at)| now - at < VOLLEY_SECONDS);
    for (volley, at) in &volleys.0 {
        let fade = 1.0 - (now - at) / VOLLEY_SECONDS;
        gizmos.line(
            volley.from.with_y(OVERLAY_Y),
            volley.to.with_y(OVERLAY_Y),
            Color::srgba(1.0, 0.95, 0.8, fade),
        );
    }
}

fn flat_circle(center: Vec3) -> Isometry3d {
    Isometry3d::new(
        center.with_y(OVERLAY_Y),
        Quat::from_rotation_arc(Vec3::Z, Vec3::Y),
    )
}

/// The player's route. Other captains don't share their plans.
fn draw_routes(ships: Query<(&Transform, &Route), With<Player>>, mut gizmos: Gizmos) {
    let color = Color::srgb(1.0, 0.85, 0.3);
    for (transform, route) in &ships {
        let path = std::iter::once(transform.translation)
            .chain(route.waypoints.iter().copied())
            .map(|p| p.with_y(OVERLAY_Y));
        gizmos.linestrip(path, color.with_alpha(0.5));
        if let Some(destination) = route.waypoints.back() {
            gizmos.circle(flat_circle(*destination), ARRIVAL_RADIUS, color);
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

/// A ring around every ship, sized to the zoom: gold for the player, and
/// the colour of her nation for every other, or grey for none.
fn draw_ship_markers(
    ships: Query<(&Transform, Has<Player>, Option<&Colours>), With<Hull>>,
    rig: Single<&CameraRig>,
    mut gizmos: Gizmos,
) {
    let radius = rig.distance * MARKER_SCALE;
    for (transform, is_player, colours) in &ships {
        let color = match colours {
            _ if is_player => Color::srgb(1.0, 0.85, 0.3),
            Some(colours) => nation_colour(colours.0),
            None => Color::srgb(0.55, 0.55, 0.55),
        };
        gizmos.circle(flat_circle(transform.translation), radius, color);
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
