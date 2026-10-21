//! A playtester's note: F8 pauses the game and opens a box to type in.
//! Enter keeps the note in the chronicle, with a snapshot of the moment;
//! Escape drops it. While the box is open, keys type and do nothing else.
//! Only with the `chronicle` feature.

use bevy::input::ButtonState;
use bevy::input::InputSystems;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use super::panel::{INK, PARCHMENT};
use crate::chronicle::Note;

/// The key that opens the box. Not bindable: it's for playtests, not play.
const NOTE_KEY: KeyCode = KeyCode::F8;

/// The note being typed, if any.
#[derive(Resource, Debug, Default)]
struct Typing(Option<Draft>);

#[derive(Debug)]
struct Draft {
    text: String,
    /// Whether the game was paused before, to leave it as it was.
    was_paused: bool,
}

#[derive(Component)]
struct NoteBox;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Typing>()
        // After input is read and before anything acts on it, so the
        // game's controls never see a key typed into the note.
        .add_systems(PreUpdate, type_note.after(InputSystems))
        .add_systems(Update, show_note.run_if(resource_changed::<Typing>));
}

fn type_note(
    mut typed: MessageReader<KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut typing: ResMut<Typing>,
    mut time: ResMut<Time<Virtual>>,
    mut notes: MessageWriter<Note>,
) {
    let Some(draft) = &mut typing.0 else {
        if keys.just_pressed(NOTE_KEY) {
            typing.0 = Some(Draft {
                text: String::new(),
                was_paused: time.is_paused(),
            });
            time.pause();
            typed.clear();
            keys.reset_all();
        }
        return;
    };
    let mut done = None;
    for key in typed.read().filter(|key| key.state == ButtonState::Pressed) {
        match &key.logical_key {
            Key::Enter => done = Some(true),
            Key::Escape => done = Some(false),
            Key::Backspace => {
                draft.text.pop();
            }
            _ => {
                let text = key.text.as_deref().unwrap_or_default();
                draft.text.extend(text.chars().filter(|c| !c.is_control()));
            }
        }
        if done.is_some() {
            break;
        }
    }
    keys.reset_all();
    let Some(keep) = done else {
        return;
    };
    let text = draft.text.trim().to_string();
    if keep && !text.is_empty() {
        notes.write(Note(text));
    }
    if !draft.was_paused {
        time.unpause();
    }
    typing.0 = None;
}

fn show_note(typing: Res<Typing>, boxes: Query<Entity, With<NoteBox>>, mut commands: Commands) {
    for open in &boxes {
        commands.entity(open).despawn();
    }
    let Some(draft) = &typing.0 else {
        return;
    };
    commands.spawn((
        NoteBox,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(120),
            left: percent(20),
            width: percent(60),
            padding: UiRect::all(px(12)),
            ..default()
        },
        BackgroundColor(PARCHMENT),
        children![(
            Text::new(format!(
                "Playtest note (Enter keeps it, Escape drops it):\n{}_",
                draft.text
            )),
            TextColor(INK),
        )],
    ));
}
