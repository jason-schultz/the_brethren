//! Sheets: the panels the captain opens from the menu or by key. Only one
//! is open at a time; opening another replaces it, and its key or Escape
//! closes it.

mod captain;
mod crew;
mod fleet;
mod hold;
mod keys;
mod nations;

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::panel::{Row, Style, screen};
use crate::controls::{Bindings, Control, ControlUsed, Rebinding, Sheet, key_name};
use crate::factions::{HoistColours, Nation};
use crate::officers::{OfficerAct, OfficerOrder};
use crate::player::Player;
use crate::schedule::FrameSet;
use crate::visuals::portrait::PortraitCamera;

/// Over the port screen, which may be open underneath.
const SHEET_LAYER: i32 = 1;

/// The sheet that's open, if any.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
struct OpenSheet(Option<Sheet>);

/// A sheet's root node.
#[derive(Component)]
struct SheetScreen(Sheet);

/// What a sheet's button does. The fleet sheet has its own.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum SheetAction {
    Close,
    Officer {
        officer: Entity,
        act: OfficerAct,
    },
    /// Wait for a new key for this control.
    Rebind(Control),
    ResetBindings,
    /// Fly this nation's colours.
    Hoist(Nation),
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<OpenSheet>()
        .add_plugins((
            captain::plugin,
            crew::plugin,
            fleet::plugin,
            hold::plugin,
            keys::plugin,
            nations::plugin,
        ))
        .add_systems(
            Update,
            (
                (toggle_sheets, show_open_sheet)
                    .chain()
                    .in_set(FrameSet::Input),
                wake_portrait.run_if(resource_changed::<OpenSheet>),
            ),
        );
}

/// A run condition: whether `sheet` is open.
fn is_open(sheet: Sheet) -> impl FnMut(Res<OpenSheet>) -> bool + Clone {
    move |open: Res<OpenSheet>| open.0 == Some(sheet)
}

/// A sheet's key opens it, or closes it if it's open; Escape closes it.
fn toggle_sheets(mut controls: MessageReader<ControlUsed>, mut open: ResMut<OpenSheet>) {
    for &ControlUsed(control) in controls.read() {
        match control {
            Control::Open(sheet) if open.0 == Some(sheet) => open.0 = None,
            Control::Open(sheet) => open.0 = Some(sheet),
            Control::Close => open.0 = None,
            _ => {}
        }
    }
}

/// Spawns the open sheet and despawns the others. Leaving the controls
/// sheet gives up any rebinding in progress.
fn show_open_sheet(
    open: Res<OpenSheet>,
    screens: Query<(Entity, &SheetScreen)>,
    mut rebinding: ResMut<Rebinding>,
    mut commands: Commands,
) {
    if !open.is_changed() {
        return;
    }
    if open.0 != Some(Sheet::Controls) {
        rebinding.0 = None;
    }
    let mut showing = false;
    for (entity, screen) in &screens {
        if Some(screen.0) == open.0 {
            showing = true;
        } else {
            commands.entity(entity).despawn();
        }
    }
    let Some(sheet) = open.0.filter(|_| !showing) else {
        return;
    };
    let root = (SheetScreen(sheet), GlobalZIndex(SHEET_LAYER));
    match sheet {
        Sheet::Fleet => commands.spawn(screen::<fleet::FleetAction>(root)),
        _ => commands.spawn(screen::<SheetAction>(root)),
    };
}

/// The portrait is only rendered while the captain sheet shows it.
fn wake_portrait(open: Res<OpenSheet>, mut camera: Single<&mut Camera, With<PortraitCamera>>) {
    camera.is_active = open.0 == Some(Sheet::Captain);
}

/// How the sheet's subtitle says to close it.
fn closing_hint(bindings: &Bindings, sheet: Sheet) -> String {
    match bindings.key(Control::Open(sheet)) {
        Some(key) => format!("{} or Esc to close", key_name(key)),
        None => "Esc to close".to_string(),
    }
}

fn close_row() -> Row<SheetAction> {
    Row::text(Style::Body, "").with_button("Close", SheetAction::Close)
}

fn act(
    activate: On<Activate>,
    actions: Query<&SheetAction>,
    player: Single<Entity, With<Player>>,
    mut open: ResMut<OpenSheet>,
    mut rebinding: ResMut<Rebinding>,
    mut bindings: ResMut<Bindings>,
    mut orders: MessageWriter<OfficerOrder>,
    mut hoists: MessageWriter<HoistColours>,
) {
    let Ok(&action) = actions.get(activate.entity) else {
        return;
    };
    match action {
        SheetAction::Close => open.0 = None,
        SheetAction::Officer { officer, act } => {
            orders.write(OfficerOrder {
                ship: *player,
                officer,
                act,
            });
        }
        SheetAction::Rebind(control) => rebinding.0 = Some(control),
        SheetAction::ResetBindings => *bindings = Bindings::default(),
        SheetAction::Hoist(nation) => {
            hoists.write(HoistColours {
                ship: *player,
                nation,
            });
        }
    }
}
