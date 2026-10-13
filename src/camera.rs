//! Top-down camera that follows the player's ship, and cursor-to-ocean picking.

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;

use crate::player::Player;

const MIN_DISTANCE: f32 = 60.0;
const MAX_DISTANCE: f32 = 8000.0;
/// Fractional zoom per scroll line.
const ZOOM_PER_LINE: f32 = 0.1;
/// Trackpads report pixels; roughly this many pixels make one line.
const PIXELS_PER_LINE: f32 = 20.0;
/// Camera elevation above the horizon. High enough to read as top-down,
/// low enough to keep a sense of depth.
const PITCH: f32 = 60.0_f32.to_radians();

/// Orbit parameters for the main camera.
#[derive(Component, Debug)]
pub struct CameraRig {
    /// Distance from the focus point, meters.
    pub distance: f32,
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, (zoom, follow_player).chain());
    }
}

/// The point on the ocean surface under the cursor, if any.
pub fn cursor_to_ocean(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Vec3> {
    let cursor = window.cursor_position()?;
    let ray = camera.viewport_to_world(camera_transform, cursor).ok()?;
    ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            far: MAX_DISTANCE * 10.0,
            ..default()
        }),
        CameraRig { distance: 400.0 },
    ));
}

fn zoom(scroll: Res<AccumulatedMouseScroll>, mut rig: Single<&mut CameraRig>) {
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / PIXELS_PER_LINE,
    };
    if lines != 0.0 {
        rig.distance =
            (rig.distance * (1.0 - lines * ZOOM_PER_LINE)).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }
}

/// Places the camera south of and above the player, looking north toward the ship.
fn follow_player(
    player: Single<&Transform, (With<Player>, Without<CameraRig>)>,
    camera: Single<(&mut Transform, &CameraRig)>,
) {
    let (mut transform, rig) = camera.into_inner();
    let focus = player.translation.with_y(0.0);
    let offset = Vec3::new(0.0, PITCH.sin(), PITCH.cos()) * rig.distance;
    *transform = Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y);
}
