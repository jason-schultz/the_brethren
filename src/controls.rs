//! Controls: the things a captain can do from the keyboard or the HUD's
//! buttons, and the keys bound to them.
//!
//! Pressing a bound key and clicking a HUD button both send the same
//! [`ControlUsed`], so every control has one path however it's used. Keys
//! can be rebound, and the bindings are kept in a settings file beside the
//! quicksave. Escape always closes what's open, and the camera's WASD and
//! arrow keys aren't bindable.

use std::fs;

use bevy::prelude::*;

use crate::schedule::FrameSet;
use crate::user_files::user_file;

/// The panels a captain can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sheet {
    /// The captain, his ship, and her condition.
    Captain,
    /// The men, their morale, and the officers.
    Crew,
    /// What she carries, and the contracts.
    Hold,
    /// The flagship and her prizes.
    Fleet,
    /// Standing with every nation.
    Nations,
    /// The key bindings.
    Controls,
}

/// Something the captain does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    FirePort,
    FireStarboard,
    Board,
    CycleBattleStyle,
    FollowShip,
    Pause,
    Slower,
    Faster,
    /// Pick the speed at this place in [`crate::time_scale::SPEEDS`]. The
    /// HUD's buttons only.
    Speed(usize),
    Save,
    Load,
    Open(Sheet),
    /// Close whatever's open. Always Escape.
    Close,
    /// Show or hide each nation's sphere of influence on the sea.
    Influence,
    /// Strike her colours to the enemy she's fighting.
    Strike,
}

/// Every control that takes a key, in the order the controls sheet lists
/// them. The settings file names controls by their place here, so new ones
/// go at the end.
pub const BINDABLE: [Control; 19] = [
    Control::FirePort,
    Control::FireStarboard,
    Control::Board,
    Control::CycleBattleStyle,
    Control::FollowShip,
    Control::Pause,
    Control::Slower,
    Control::Faster,
    Control::Save,
    Control::Load,
    Control::Open(Sheet::Captain),
    Control::Open(Sheet::Crew),
    Control::Open(Sheet::Hold),
    Control::Open(Sheet::Fleet),
    Control::Open(Sheet::Nations),
    Control::Open(Sheet::Controls),
    Control::Close,
    Control::Influence,
    Control::Strike,
];

/// The controls on the action bar, in order.
pub const ACTION_BAR: [Control; 6] = [
    Control::FirePort,
    Control::FireStarboard,
    Control::Board,
    Control::CycleBattleStyle,
    Control::FollowShip,
    Control::Strike,
];

/// The menu's buttons, in order.
pub const MENU: [Control; 7] = [
    Control::Open(Sheet::Captain),
    Control::Open(Sheet::Crew),
    Control::Open(Sheet::Hold),
    Control::Open(Sheet::Fleet),
    Control::Open(Sheet::Nations),
    Control::Influence,
    Control::Open(Sheet::Controls),
];

impl Control {
    pub fn name(self) -> &'static str {
        match self {
            Self::FirePort => "Fire port",
            Self::FireStarboard => "Fire starboard",
            Self::Board => "Board",
            Self::CycleBattleStyle => "Battle style",
            Self::FollowShip => "Follow ship",
            Self::Pause => "Pause",
            Self::Slower => "Slower",
            Self::Faster => "Faster",
            Self::Speed(_) => "Speed",
            Self::Save => "Quicksave",
            Self::Load => "Quickload",
            Self::Open(sheet) => sheet.name(),
            Self::Close => "Close",
            Self::Influence => "Influence",
            Self::Strike => "Strike colours",
        }
    }

    /// The key this control is fixed to, if it can't be rebound.
    fn fixed_key(self) -> Option<KeyCode> {
        (self == Self::Close).then_some(KeyCode::Escape)
    }
}

impl Sheet {
    pub fn name(self) -> &'static str {
        match self {
            Self::Captain => "Captain",
            Self::Crew => "Crew",
            Self::Hold => "Hold",
            Self::Fleet => "Fleet",
            Self::Nations => "Nations",
            Self::Controls => "Controls",
        }
    }
}

/// Keys that can be bound, with the names shown for them and kept in the
/// settings file. The camera's keys and Escape aren't here.
const KEYS: [(KeyCode, &str); 50] = [
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::Digit0, "0"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F9, "F9"),
    (KeyCode::F10, "F10"),
    (KeyCode::Space, "Space"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="),
    (KeyCode::BracketLeft, "["),
    (KeyCode::BracketRight, "]"),
    (KeyCode::Comma, ","),
    (KeyCode::Period, "."),
];

/// The name shown for a key.
pub fn key_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::Escape => "Esc",
        _ => KEYS
            .iter()
            .find(|(k, _)| *k == key)
            .map_or("?", |(_, name)| name),
    }
}

fn key_named(name: &str) -> Option<KeyCode> {
    KEYS.iter().find(|(_, n)| *n == name).map(|(key, _)| *key)
}

/// The key bound to each bindable control.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Bindings(Vec<(Control, KeyCode)>);

impl Default for Bindings {
    /// The action bar on the number keys, and the panels on their initials.
    fn default() -> Self {
        Self(vec![
            (Control::FirePort, KeyCode::Digit1),
            (Control::FireStarboard, KeyCode::Digit2),
            (Control::Board, KeyCode::Digit3),
            (Control::CycleBattleStyle, KeyCode::Digit4),
            (Control::FollowShip, KeyCode::Digit5),
            (Control::Pause, KeyCode::Space),
            (Control::Slower, KeyCode::Minus),
            (Control::Faster, KeyCode::Equal),
            (Control::Save, KeyCode::F5),
            (Control::Load, KeyCode::F9),
            (Control::Open(Sheet::Captain), KeyCode::KeyC),
            (Control::Open(Sheet::Crew), KeyCode::KeyR),
            (Control::Open(Sheet::Hold), KeyCode::KeyH),
            (Control::Open(Sheet::Fleet), KeyCode::Tab),
            (Control::Open(Sheet::Nations), KeyCode::KeyN),
            (Control::Open(Sheet::Controls), KeyCode::F1),
            (Control::Influence, KeyCode::KeyM),
            (Control::Strike, KeyCode::Digit6),
        ])
    }
}

impl Bindings {
    /// The key for `control`, if it has one.
    pub fn key(&self, control: Control) -> Option<KeyCode> {
        control.fixed_key().or_else(|| {
            self.0
                .iter()
                .find(|(c, _)| *c == control)
                .map(|(_, key)| *key)
        })
    }

    /// The control `key` is bound to.
    pub fn control(&self, key: KeyCode) -> Option<Control> {
        if key == KeyCode::Escape {
            return Some(Control::Close);
        }
        self.0.iter().find(|(_, k)| *k == key).map(|(c, _)| *c)
    }

    /// Binds `key` to `control`. A control that had `key` takes
    /// `control`'s old key instead, so no key does two things. Keys that
    /// can't be bound, and fixed controls, are left alone.
    pub fn bind(&mut self, control: Control, key: KeyCode) {
        if control.fixed_key().is_some() || key_named(key_name(key)) != Some(key) {
            return;
        }
        let old = self.key(control);
        self.0.retain(|(c, _)| *c != control);
        if let Some(entry) = self.0.iter_mut().find(|(_, k)| *k == key) {
            match old {
                Some(old) => entry.1 = old,
                None => {
                    let other = entry.0;
                    self.0.retain(|(c, _)| *c != other);
                }
            }
        }
        self.0.push((control, key));
        self.0
            .sort_by_key(|(control, _)| BINDABLE.iter().position(|c| c == control));
    }

    /// The settings file's text: one control per line, by its place in
    /// [`BINDABLE`], then its key.
    pub fn to_text(&self) -> String {
        BINDABLE
            .iter()
            .enumerate()
            .filter_map(|(i, &control)| {
                let key = self.0.iter().find(|(c, _)| *c == control)?.1;
                Some(format!("{i} {}\n", key_name(key)))
            })
            .collect()
    }

    /// Bindings from a settings file's text. Lines that don't make sense
    /// are skipped, keeping the default for that control.
    pub fn from_text(text: &str) -> Self {
        let mut bindings = Self::default();
        for line in text.lines() {
            let Some((index, key)) = line.trim().split_once(' ') else {
                continue;
            };
            let control = index.parse::<usize>().ok().and_then(|i| BINDABLE.get(i));
            if let (Some(&control), Some(key)) = (control, key_named(key)) {
                bindings.bind(control, key);
            }
        }
        bindings
    }
}

/// A control was used, by key or by button.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlUsed(pub Control);

/// The control waiting for its new key, while the captain is rebinding it.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rebinding(pub Option<Control>);

/// A run condition: whether `control` was used this frame.
pub fn used(control: Control) -> impl FnMut(MessageReader<ControlUsed>) -> bool + Clone {
    move |mut controls: MessageReader<ControlUsed>| controls.read().any(|used| used.0 == control)
}

/// Where the bindings are kept.
const SETTINGS_FILE: &str = "keys.txt";

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        let bindings = fs::read_to_string(user_file(SETTINGS_FILE))
            .map(|text| Bindings::from_text(&text))
            .unwrap_or_default();
        app.add_message::<ControlUsed>()
            .insert_resource(bindings)
            .init_resource::<Rebinding>()
            .add_systems(
                Update,
                (
                    read_keys.in_set(FrameSet::Controls),
                    keep_bindings.run_if(resource_changed::<Bindings>),
                ),
            );
    }
}

/// Turns key presses into controls. While rebinding, the next key press
/// is the new key instead, and Escape gives up.
fn read_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut bindings: ResMut<Bindings>,
    mut rebinding: ResMut<Rebinding>,
    mut controls: MessageWriter<ControlUsed>,
) {
    for &key in keys.get_just_pressed() {
        if let Some(control) = rebinding.0 {
            if key != KeyCode::Escape {
                bindings.bind(control, key);
            }
            rebinding.0 = None;
            return;
        }
        if let Some(control) = bindings.control(key) {
            controls.write(ControlUsed(control));
        }
    }
}

fn keep_bindings(bindings: Res<Bindings>) {
    if bindings.is_added() {
        return;
    }
    let path = user_file(SETTINGS_FILE);
    let written = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&path, bindings.to_text()));
    if let Err(error) = written {
        warn!("couldn't keep the key bindings: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bindable_control_has_its_own_key() {
        let bindings = Bindings::default();
        let mut keys: Vec<KeyCode> = BINDABLE
            .iter()
            .map(|&control| {
                bindings
                    .key(control)
                    .unwrap_or_else(|| panic!("{control:?}"))
            })
            .collect();
        let count = keys.len();
        keys.sort_by_key(|key| key_name(*key));
        keys.dedup();
        assert_eq!(keys.len(), count, "two controls share a key");
    }

    #[test]
    fn the_action_bar_is_on_the_number_keys() {
        let bindings = Bindings::default();
        let names: Vec<&str> = ACTION_BAR
            .iter()
            .map(|&control| key_name(bindings.key(control).unwrap()))
            .collect();
        assert_eq!(names, ["1", "2", "3", "4", "5", "6"]);
    }

    #[test]
    fn binding_a_taken_key_swaps_the_two() {
        let mut bindings = Bindings::default();
        bindings.bind(Control::FirePort, KeyCode::KeyQ);
        assert_eq!(bindings.control(KeyCode::KeyQ), Some(Control::FirePort));
        assert_eq!(
            bindings.control(KeyCode::Digit1),
            None,
            "the old key is free"
        );
        bindings.bind(Control::FireStarboard, KeyCode::KeyQ);
        assert_eq!(bindings.key(Control::FireStarboard), Some(KeyCode::KeyQ));
        assert_eq!(
            bindings.key(Control::FirePort),
            Some(KeyCode::Digit2),
            "took starboard's old key"
        );
    }

    #[test]
    fn camera_keys_and_escape_cant_be_taken() {
        let mut bindings = Bindings::default();
        bindings.bind(Control::Board, KeyCode::KeyW);
        bindings.bind(Control::Close, KeyCode::KeyX);
        bindings.bind(Control::Board, KeyCode::Escape);
        assert_eq!(bindings, Bindings::default());
        assert_eq!(bindings.control(KeyCode::Escape), Some(Control::Close));
    }

    #[test]
    fn bindings_survive_the_settings_file() {
        let mut bindings = Bindings::default();
        bindings.bind(Control::Board, KeyCode::KeyB);
        bindings.bind(Control::Open(Sheet::Captain), KeyCode::F2);
        assert_eq!(Bindings::from_text(&bindings.to_text()), bindings);
        assert_eq!(
            Bindings::from_text("nonsense\n99 Q\n2 Nope\n"),
            Bindings::default()
        );
    }

    #[test]
    fn a_bound_key_uses_its_control_and_rebinding_takes_the_next_key() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<ControlUsed>()
            .init_resource::<Bindings>()
            .init_resource::<Rebinding>()
            .add_systems(Update, read_keys);
        let press = |app: &mut App, key| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            keys.release_all();
            keys.press(key);
            app.world_mut()
                .resource_mut::<Messages<ControlUsed>>()
                .clear();
            app.update();
            app.world()
                .resource::<Messages<ControlUsed>>()
                .iter_current_update_messages()
                .copied()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            press(&mut app, KeyCode::KeyC),
            [ControlUsed(Control::Open(Sheet::Captain))]
        );
        app.world_mut().resource_mut::<Rebinding>().0 = Some(Control::Board);
        assert_eq!(press(&mut app, KeyCode::KeyB), [], "taken as the new key");
        assert_eq!(
            press(&mut app, KeyCode::KeyB),
            [ControlUsed(Control::Board)]
        );
    }
}
