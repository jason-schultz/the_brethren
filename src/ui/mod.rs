//! Screen-space interface: the HUD and the port screen. Reads simulation
//! state, and changes it only by sending order messages.

mod hud;
mod port_screen;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::Button;

const BUTTON_IDLE: Color = Color::srgb(0.3, 0.2, 0.12);
const BUTTON_HOVER: Color = Color::srgb(0.42, 0.29, 0.17);
const BUTTON_PRESSED: Color = Color::srgb(0.2, 0.13, 0.08);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((hud::plugin, port_screen::plugin))
            .add_systems(Update, color_buttons);
    }
}

/// The look shared by every button. Pair it with a `Button` and a text child.
fn button_style() -> impl Bundle {
    (
        Node {
            margin: UiRect::top(px(12)),
            padding: UiRect::axes(px(20), px(10)),
            align_self: AlignSelf::Center,
            ..default()
        },
        Hovered::default(),
        BackgroundColor(BUTTON_IDLE),
    )
}

fn color_buttons(mut buttons: Query<(&Hovered, Has<Pressed>, &mut BackgroundColor), With<Button>>) {
    for (hovered, pressed, mut background) in &mut buttons {
        background.0 = match (pressed, hovered.get()) {
            (true, _) => BUTTON_PRESSED,
            (false, true) => BUTTON_HOVER,
            (false, false) => BUTTON_IDLE,
        };
    }
}
