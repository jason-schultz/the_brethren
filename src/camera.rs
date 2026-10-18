//! The top-down camera, and cursor-to-ocean picking.
//!
//! The camera looks at a focus point on the ocean. It follows the player's
//! ship until the captain pans away (WASD, arrow keys, or dragging with the
//! left mouse button), and the follow control brings it back. Scrolling zooms toward the
//! point under the cursor, so you zoom into wherever you are looking.
//! Camera controls read the real clock, so they keep working while paused.

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::controls::{Control, used};
use crate::player::Player;
use crate::port::Docked;

const MIN_DISTANCE: f32 = 60.0;
/// Far enough out to see the whole map at once.
const MAX_DISTANCE: f32 = 800_000.0;
/// Fractional zoom per scroll line.
const ZOOM_PER_LINE: f32 = 0.1;
/// Trackpads report pixels; roughly this many pixels make one line.
const PIXELS_PER_LINE: f32 = 20.0;
/// Keyboard panning speed, as a fraction of the camera distance per second,
/// so panning feels the same at every zoom.
const PAN_SPEED: f32 = 0.8;
/// Camera elevation above the horizon. High enough to read as top-down,
/// low enough to keep a sense of depth.
const PITCH: f32 = 60.0_f32.to_radians();

/// Orbit parameters for the main camera.
#[derive(Component, Debug)]
pub struct CameraRig {
    /// The point on the ocean the camera looks at.
    pub focus: Vec3,
    /// Distance from the focus point, meters.
    pub distance: f32,
    /// Whether the focus stays on the player's ship.
    pub following: bool,
    /// The ocean point grabbed by a left-button drag in progress.
    grabbed: Option<Vec3>,
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera).add_systems(
            Update,
            (
                follow_again.run_if(used(Control::FollowShip)),
                pan_with_keys,
                drag_to_pan,
                zoom,
                follow_player,
                place_camera,
            )
                .chain(),
        );
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

/// The new focus that keeps `point` under the cursor while the camera's
/// distance is multiplied by `factor`.
///
/// The camera looks at the focus from a fixed angle, so changing distance
/// scales the visible ocean about the focus: the point under a given pixel
/// moves to `focus + factor * (point - focus)`. Shifting the focus by the
/// difference puts `point` back under the cursor.
pub fn zoom_toward(focus: Vec3, point: Vec3, factor: f32) -> Vec3 {
    point - (point - focus) * factor
}

/// Camera translation for a focus and distance: south of and above the
/// focus, looking north.
fn camera_transform(focus: Vec3, distance: f32) -> Transform {
    let offset = Vec3::new(0.0, PITCH.sin(), PITCH.cos()) * distance;
    Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y)
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        // Other cameras render to images, such as the captain's portrait.
        IsDefaultUiCamera,
        Projection::Perspective(PerspectiveProjection {
            far: MAX_DISTANCE * 10.0,
            ..default()
        }),
        CameraRig {
            focus: Vec3::ZERO,
            distance: 400.0,
            following: true,
            grabbed: None,
        },
    ));
}

fn follow_again(mut rig: Single<&mut CameraRig>) {
    rig.following = true;
}

/// WASD or the arrow keys pan the view.
fn pan_with_keys(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    mut rig: Single<&mut CameraRig>,
) {
    let pressed = |a, b| keys.pressed(a) || keys.pressed(b);
    // North is −Z and east is +X.
    let mut direction = Vec3::ZERO;
    if pressed(KeyCode::KeyW, KeyCode::ArrowUp) {
        direction.z -= 1.0;
    }
    if pressed(KeyCode::KeyS, KeyCode::ArrowDown) {
        direction.z += 1.0;
    }
    if pressed(KeyCode::KeyA, KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if pressed(KeyCode::KeyD, KeyCode::ArrowRight) {
        direction.x += 1.0;
    }
    if direction != Vec3::ZERO {
        let step = direction.normalize() * rig.distance * PAN_SPEED * time.delta_secs();
        rig.focus += step;
        rig.following = false;
    }
}

/// Dragging with the left button moves the ocean with the cursor. Not while
/// docked: the port screen covers the view and uses the mouse.
fn drag_to_pan(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    docked: Query<(), (With<Player>, With<Docked>)>,
    camera: Single<(&Camera, &GlobalTransform, &mut CameraRig)>,
) {
    let (camera, camera_transform, mut rig) = camera.into_inner();
    if !buttons.pressed(MouseButton::Left) || !docked.is_empty() {
        rig.grabbed = None;
        return;
    }
    let Some(under_cursor) = cursor_to_ocean(&window, camera, camera_transform) else {
        return;
    };
    match rig.grabbed {
        None => rig.grabbed = Some(under_cursor),
        Some(grabbed) if grabbed != under_cursor => {
            rig.focus += grabbed - under_cursor;
            rig.following = false;
        }
        Some(_) => {}
    }
}

/// Scrolling zooms toward the point under the cursor. While following the
/// ship, it zooms on the ship instead, so the view doesn't drift off her.
fn zoom(
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform, &mut CameraRig)>,
) {
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / PIXELS_PER_LINE,
    };
    if lines == 0.0 {
        return;
    }
    let (camera, camera_transform, mut rig) = camera.into_inner();
    let distance = (rig.distance * (1.0 - lines * ZOOM_PER_LINE)).clamp(MIN_DISTANCE, MAX_DISTANCE);
    if !rig.following
        && let Some(point) = cursor_to_ocean(&window, camera, camera_transform)
    {
        rig.focus = zoom_toward(rig.focus, point, distance / rig.distance);
    }
    rig.distance = distance;
}

fn follow_player(
    player: Single<&Transform, (With<Player>, Without<CameraRig>)>,
    mut rig: Single<&mut CameraRig>,
) {
    if rig.following {
        rig.focus = player.translation.with_y(0.0);
    }
}

fn place_camera(camera: Single<(&mut Transform, &CameraRig)>) {
    let (mut transform, rig) = camera.into_inner();
    *transform = camera_transform(rig.focus, rig.distance);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ocean point seen through a pixel, for a camera at `focus` and
    /// `distance`. Pixels are given as a ray direction from the camera.
    fn seen(focus: Vec3, distance: f32, ray: Vec3) -> Vec3 {
        let camera = camera_transform(focus, distance);
        let direction = camera.rotation * ray;
        let t = -camera.translation.y / direction.y;
        camera.translation + direction * t
    }

    #[test]
    fn zooming_keeps_the_point_under_the_cursor_fixed() {
        let focus = Vec3::new(100.0, 0.0, -50.0);
        // A ray off to one side of the view's center, as for an off-center cursor.
        let ray = Vec3::new(0.3, -0.2, -1.0).normalize();
        let point = seen(focus, 1_000.0, ray);
        for factor in [0.5, 0.9, 1.5] {
            let new_focus = zoom_toward(focus, point, factor);
            let after = seen(new_focus, 1_000.0 * factor, ray);
            assert!(
                after.distance(point) < 0.05,
                "factor {factor}: {after} vs {point}"
            );
        }
    }

    #[test]
    fn zooming_on_the_focus_leaves_it_alone() {
        let focus = Vec3::new(5.0, 0.0, 5.0);
        assert_eq!(zoom_toward(focus, focus, 0.5), focus);
    }

    #[test]
    fn the_camera_looks_north_at_its_focus() {
        let focus = Vec3::new(1_000.0, 0.0, 2_000.0);
        let camera = camera_transform(focus, 500.0);
        assert!(camera.translation.z > focus.z, "south of the focus");
        assert!(camera.translation.y > 0.0, "above the water");
        assert!(seen(focus, 500.0, Vec3::NEG_Z).distance(focus) < 0.01);
    }
}
