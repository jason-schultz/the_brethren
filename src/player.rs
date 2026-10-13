//! The player's ship and the player's orders.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::cursor_to_ocean;
use crate::navigation::{Arrival, SailOrder};
use crate::port::{Docked, Port};
use crate::schedule::FrameSet;

/// Clicking within this distance (meters) of a port's town or anchorage
/// orders the ship to dock there.
const PORT_CLICK_RADIUS: f32 = 120.0;

/// Marks the ship the player commands.
#[derive(Component, Debug)]
pub struct Player;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, order_sail_on_click.in_set(FrameSet::Input));
    }
}

/// Right-click on the ocean orders the player's ship to sail there. Right-click
/// near a port sails there to dock. Left click is reserved for selection.
/// Orders aren't given from the ocean view while docked; the port screen
/// handles leaving.
fn order_sail_on_click(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    player: Single<Entity, (With<Player>, Without<Docked>)>,
    ports: Query<(Entity, &Port, &Transform)>,
    mut orders: MessageWriter<SailOrder>,
) {
    if !buttons.just_pressed(MouseButton::Right) {
        return;
    }
    let (camera, camera_transform) = *camera;
    let Some(clicked) = cursor_to_ocean(&window, camera, camera_transform) else {
        return;
    };
    let near = |point: Vec3| point.with_y(0.0).distance(clicked) <= PORT_CLICK_RADIUS;
    let order = match ports
        .iter()
        .find(|(_, port, anchorage)| near(port.town) || near(anchorage.translation))
    {
        Some((port, _, anchorage)) => SailOrder {
            ship: *player,
            destination: anchorage.translation,
            on_arrival: Arrival::Dock(port),
        },
        None => SailOrder {
            ship: *player,
            destination: clicked,
            on_arrival: Arrival::HeaveTo,
        },
    };
    orders.write(order);
}
