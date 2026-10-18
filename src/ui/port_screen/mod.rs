//! The port screen shown while the player's ship is docked.
//!
//! Placeholder: lists the town's locations and offers Cast Off. Each
//! location becomes a real screen as its system is built.

use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};

use super::button_style;
use crate::player::Player;
use crate::port::{CastOff, Docked, Port};

#[derive(Component)]
struct PortScreen;

/// Locations every port will offer, with what each will be for.
const LOCATIONS: [(&str, &str); 5] = [
    ("Tavern", "news, rumours, and men looking for a berth"),
    ("Market", "cargo bought and sold"),
    ("Shipyard", "repairs, refits, and new hulls"),
    ("Governor's House", "commissions and letters of marque"),
    (
        "Merchants' Exchange",
        "contracts, credit, and other men's strongboxes",
    ),
];

const PARCHMENT: Color = Color::srgb(0.91, 0.85, 0.7);
const INK: Color = Color::srgb(0.18, 0.12, 0.08);
const FADED_INK: Color = Color::srgb(0.45, 0.38, 0.3);

pub(super) fn plugin(app: &mut App) {
    app.add_observer(open_port_screen)
        .add_observer(close_port_screen);
}

fn open_port_screen(
    add: On<Add<Docked>>,
    players: Query<&Docked, With<Player>>,
    ports: Query<(&Name, &Port)>,
    mut commands: Commands,
) {
    let ship = add.entity;
    let Ok(docked) = players.get(ship) else {
        return;
    };
    let Ok((name, port)) = ports.get(docked.port) else {
        return;
    };

    let locations: Vec<_> = LOCATIONS
        .iter()
        .map(|(place, purpose)| {
            (
                Text::new(format!("{place} — {purpose} (closed)")),
                TextFont::from_font_size(16.0),
                TextColor(FADED_INK),
            )
        })
        .collect();

    commands
        .spawn((
            PortScreen,
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10),
                        padding: UiRect::all(px(28)),
                        min_width: px(460),
                        ..default()
                    },
                    BackgroundColor(PARCHMENT),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(name.as_str()),
                        TextFont::from_font_size(30.0),
                        TextColor(INK),
                    ));
                    panel.spawn((
                        Text::new(format!("A {} port", port.nation)),
                        TextFont::from_font_size(16.0),
                        TextColor(FADED_INK),
                    ));
                    for location in locations {
                        panel.spawn(location);
                    }
                    panel.spawn((
                        button_style(),
                        Button,
                        children![(Text::new("Cast Off"), TextFont::from_font_size(18.0))],
                        observe(move |_: On<Activate>, mut orders: MessageWriter<CastOff>| {
                            orders.write(CastOff { ship });
                        }),
                    ));
                });
        });
}

fn close_port_screen(
    remove: On<Remove<Docked>>,
    players: Query<(), With<Player>>,
    screens: Query<Entity, With<PortScreen>>,
    mut commands: Commands,
) {
    if players.contains(remove.entity) {
        for screen in &screens {
            commands.entity(screen).despawn();
        }
    }
}
