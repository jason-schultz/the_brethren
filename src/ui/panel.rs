//! The parchment panel shared by the port and battle screens and the sheets: a title, then
//! rows of text cells with buttons. Each screen describes its content as a
//! [`Panel`] of its own button actions, and the panel is only rebuilt when
//! that content changes.

use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};

use super::button_style;

pub(super) const PARCHMENT: Color = Color::srgb(0.91, 0.85, 0.7);
pub(super) const INK: Color = Color::srgb(0.18, 0.12, 0.08);
const FADED_INK: Color = Color::srgb(0.45, 0.38, 0.3);
/// Width of a row's first cell; later cells are narrower.
const FIRST_CELL: f32 = 260.0;
const CELL: f32 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Style {
    Heading,
    Body,
    Faded,
}

/// One line of the panel: text cells, then buttons that carry an `A`.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Row<A> {
    pub cells: Vec<String>,
    pub buttons: Vec<(String, A)>,
    pub style: Style,
}

impl<A> Row<A> {
    pub fn text(style: Style, text: impl Into<String>) -> Self {
        Self {
            cells: vec![text.into()],
            buttons: Vec::new(),
            style,
        }
    }

    pub fn with_button(mut self, label: impl Into<String>, action: A) -> Self {
        self.buttons.push((label.into(), action));
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Panel<A> {
    pub title: String,
    pub subtitle: String,
    /// A picture under the title, and the size to show it at, pixels.
    pub picture: Option<(Handle<Image>, Vec2)>,
    pub rows: Vec<Row<A>>,
}

impl<A> Panel<A> {
    pub fn new(title: impl Into<String>, subtitle: impl Into<String>, rows: Vec<Row<A>>) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            picture: None,
            rows,
        }
    }
}

/// The parchment panel, and the content it is showing.
#[derive(Component)]
pub(super) struct Shown<A: Send + Sync + 'static>(Option<Panel<A>>);

/// Room kept clear above and below a screen's panel for the HUD's bars, pixels.
const HUD_TOP: f32 = 40.0;
const HUD_BOTTOM: f32 = 64.0;

/// A screen: the world dimmed behind a parchment panel showing `A`s,
/// between the HUD's bars. `marker` goes on the root, for despawning the
/// screen.
pub(super) fn screen<A: Send + Sync + 'static>(marker: impl Bundle) -> impl Bundle {
    (
        marker,
        Node {
            width: percent(100),
            height: percent(100),
            padding: UiRect::new(px(0), px(0), px(HUD_TOP), px(HUD_BOTTOM)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        children![(
            Shown::<A>(None),
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                padding: UiRect::axes(px(24), px(18)),
                min_width: px(640),
                ..default()
            },
            BackgroundColor(PARCHMENT),
        )],
    )
}

/// Shows `content` on the panel `entity`, unless it's showing already.
/// Pressing a button triggers `act` with the button's entity, which holds
/// its `A`.
pub(super) fn show<A, M>(
    commands: &mut Commands,
    (entity, shown): (Entity, &mut Shown<A>),
    content: Panel<A>,
    act: impl IntoObserverSystem<Activate, M> + Clone + Sync,
) where
    A: Component + Copy + PartialEq,
    M: Send + Sync + 'static,
{
    if shown.0.as_ref() == Some(&content) {
        return;
    }
    commands.entity(entity).despawn_children();
    commands.entity(entity).with_children(|panel| {
        panel.spawn(text(&content.title, 30.0, INK));
        panel.spawn(text(&content.subtitle, 16.0, FADED_INK));
        if let Some((image, size)) = &content.picture {
            panel.spawn((
                ImageNode::new(image.clone()),
                Node {
                    width: px(size.x),
                    height: px(size.y),
                    margin: UiRect::vertical(px(6)),
                    ..default()
                },
            ));
        }
        for row in &content.rows {
            panel.spawn(row_bundle(row, act.clone()));
        }
    });
    shown.0 = Some(content);
}

/// Panel text never wraps. Each cell is one line, which keeps table columns
/// aligned, and Bevy 0.20 squeezes wrapping text in a flex row down to its
/// longest word.
fn text(content: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(content),
        TextFont::from_font_size(size),
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
    )
}

fn row_bundle<A, M>(
    row: &Row<A>,
    act: impl IntoObserverSystem<Activate, M> + Clone + Sync,
) -> impl Bundle
where
    A: Component + Copy,
    M: Send + Sync + 'static,
{
    let (size, color) = match row.style {
        Style::Heading => (20.0, INK),
        Style::Body => (15.0, INK),
        Style::Faded => (14.0, FADED_INK),
    };
    let cells: Vec<_> = row
        .cells
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            let width = if i == 0 && row.cells.len() > 1 {
                FIRST_CELL
            } else {
                CELL
            };
            (
                text(cell, size, color),
                Node {
                    min_width: px(width),
                    // A lone cell takes the slack, lining its buttons up on the right.
                    flex_grow: if row.cells.len() == 1 { 1.0 } else { 0.0 },
                    ..default()
                },
            )
        })
        .collect();
    let buttons: Vec<_> = row
        .buttons
        .iter()
        .map(|(label, action)| {
            (
                button_style(Node {
                    padding: UiRect::axes(px(12), px(5)),
                    ..default()
                }),
                Button,
                *action,
                children![text(label, 14.0, Color::WHITE)],
                observe(act.clone()),
            )
        })
        .collect();
    (
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            margin: UiRect::top(px(if row.style == Style::Heading { 10 } else { 0 })),
            ..default()
        },
        Children::spawn((SpawnIter(cells.into_iter()), SpawnIter(buttons.into_iter()))),
    )
}
