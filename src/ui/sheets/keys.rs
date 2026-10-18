//! The controls sheet: every key binding, to rebind, and the mouse.

use bevy::prelude::*;

use super::{SheetAction, act, close_row, closing_hint, is_open};
use crate::controls::{BINDABLE, Bindings, Control, Rebinding, Sheet, key_name};
use crate::schedule::FrameSet;
use crate::ui::panel::{Panel, Row, Shown, Style, show};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        refresh
            .after(FrameSet::Orders)
            .run_if(is_open(Sheet::Controls)),
    );
}

fn build(bindings: &Bindings, rebinding: Option<Control>, hint: String) -> Panel<SheetAction> {
    let mut rows = Vec::new();
    // Escape always closes, and every sheet says so.
    for control in BINDABLE.into_iter().filter(|&c| c != Control::Close) {
        let key = if rebinding == Some(control) {
            "press a key (Esc to cancel)"
        } else {
            bindings.key(control).map_or("none", key_name)
        };
        rows.push(
            Row {
                cells: vec![control.name().to_string(), key.to_string()],
                buttons: Vec::new(),
                style: Style::Body,
            }
            .with_button("Rebind", SheetAction::Rebind(control)),
        );
    }
    for line in [
        "Right-click the sea to sail there, a port to dock, a ship at sea to attack her.",
        "WASD, the arrow keys, or dragging look around; scrolling zooms.",
    ] {
        rows.push(Row::text(Style::Faded, line));
    }
    rows.push(close_row().with_button("Reset to defaults", SheetAction::ResetBindings));
    Panel::new("Controls", hint, rows)
}

fn refresh(
    bindings: Res<Bindings>,
    rebinding: Res<Rebinding>,
    panel: Single<(Entity, &mut Shown<SheetAction>)>,
    mut commands: Commands,
) {
    let content = build(
        &bindings,
        rebinding.0,
        closing_hint(&bindings, Sheet::Controls),
    );
    let (entity, mut shown) = panel.into_inner();
    show(&mut commands, (entity, &mut shown), content, act);
}
