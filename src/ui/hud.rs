//! The HUD, kept to the edges of the screen so the sea stays clear: a slim
//! bar along the top with the date, the time controls, and the essentials;
//! passing messages on the right; a battle box only while engaged; and
//! along the bottom, the action bar and the menu of panels. Every button
//! sends the same control its key does.

use std::fmt::Write;

use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};

use super::{Notices, button_style};
use crate::alert::{Alert, AlertKind};
use crate::cargo::{Hold, Purse};
use crate::clock::WorldClock;
use crate::combat::gunnery::Broadside;
use crate::combat::tactics::BOARDING_RANGE;
use crate::combat::{Battery, Engaged, Struck, Timbers};
use crate::controls::{ACTION_BAR, Bindings, Control, ControlUsed, MENU, key_name};
use crate::crew::{Crew, days_of_provisions};
use crate::factions::{NATIONS, Standing, Terms};
use crate::goods::Goods;
use crate::loyalty::Rumour;
use crate::math::{MPS_TO_KNOTS, normalize_heading};
use crate::player::{BattleStyle, Player};
use crate::port::Refused;
use crate::sailing::{Heading, Rigging, Speed};
use crate::time_control::LatestAlert;
use crate::time_scale::{SPEEDS, TimeScale};
use crate::visuals::influence::ShowInfluence;
use crate::visuals::nation_colour;
use crate::wind::Wind;

/// Over the world and the port screen, so the HUD stays usable in port.
const HUD_LAYER: i32 = 2;
const BAR: Color = Color::srgba(0.05, 0.04, 0.03, 0.72);
const TEXT: Color = Color::srgb(0.93, 0.9, 0.82);
/// The current speed's button label, keys on buttons, and warnings.
const GOLD: Color = Color::srgb(1.0, 0.82, 0.35);
const WARNING: Color = Color::srgb(1.0, 0.45, 0.35);

/// The date and the ship's essentials, in the top bar.
#[derive(Component)]
struct StatusText;

/// Warnings in the top bar: paused, wanted, starving.
#[derive(Component)]
struct WarningText;

/// The label of a button that sends this control.
#[derive(Component)]
struct ControlLabel(Control);

/// The key shown on a button that sends this control.
#[derive(Component)]
struct KeyLabel(Control);

/// A button that sends a control.
#[derive(Component, Clone, Copy)]
struct ControlButton(Control);

/// The call that stopped the clock, until the captain resumes.
#[derive(Component)]
struct PinnedText;

/// Passing notices.
#[derive(Component)]
struct NoticesText;

/// The battle box, shown only while engaged.
#[derive(Component)]
struct BattleBox;

/// The influence overlay's key to its colours, shown with it.
#[derive(Component)]
struct Legend;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_hud).add_systems(
        Update,
        (
            note_alerts,
            update_status,
            update_labels,
            update_messages,
            update_battle_box,
            show_legend.run_if(resource_changed::<ShowInfluence>),
        ),
    );
}

fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(color),
        TextLayout::new(Justify::Center, LineBreak::NoWrap),
    )
}

/// A button for `control`, with `children` as its face.
fn control_button(control: Control, children: impl Bundle) -> impl Bundle {
    (
        button_style(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::axes(px(10), px(4)),
            ..default()
        }),
        Button,
        ControlButton(control),
        children,
        observe(press),
    )
}

fn press(
    activate: On<Activate>,
    buttons: Query<&ControlButton>,
    mut controls: MessageWriter<ControlUsed>,
) {
    if let Ok(button) = buttons.get(activate.entity) {
        controls.write(ControlUsed(button.0));
    }
}

/// A button whose face is the control's key over its name.
fn keyed_button(control: Control) -> impl Bundle {
    control_button(
        control,
        children![
            (label("", 12.0, GOLD), KeyLabel(control)),
            (label(control.name(), 14.0, TEXT), ControlLabel(control)),
        ],
    )
}

/// The speed buttons: pause, then each speed.
fn speed_buttons() -> impl Bundle {
    let speeds = (0..SPEEDS.len()).map(|index| {
        let control = Control::Speed(index);
        control_button(
            control,
            children![(
                label(format!("×{}", SPEEDS[index]), 14.0, TEXT),
                ControlLabel(control)
            )],
        )
    });
    (
        Node {
            column_gap: px(4),
            ..default()
        },
        Children::spawn((
            Spawn(control_button(
                Control::Pause,
                children![(label("Pause", 14.0, TEXT), ControlLabel(Control::Pause))],
            )),
            SpawnIter(speeds),
        )),
    )
}

fn spawn_hud(mut commands: Commands) {
    // The top bar.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            right: px(0),
            padding: UiRect::axes(px(10), px(5)),
            column_gap: px(18),
            align_items: AlignItems::Center,
            flex_wrap: FlexWrap::Wrap,
            ..default()
        },
        BackgroundColor(BAR),
        GlobalZIndex(HUD_LAYER),
        children![
            (label("", 15.0, TEXT), StatusText),
            speed_buttons(),
            (label("", 15.0, WARNING), WarningText),
        ],
    ));
    // Messages, down the right.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(52),
            right: px(10),
            max_width: px(460),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            row_gap: px(6),
            ..default()
        },
        GlobalZIndex(HUD_LAYER),
        children![
            (
                Text::default(),
                TextFont::from_font_size(16.0),
                TextColor(GOLD),
                Node {
                    padding: UiRect::axes(px(10), px(6)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(BAR),
                PinnedText,
            ),
            (
                Text::default(),
                TextFont::from_font_size(14.0),
                TextColor(TEXT),
                TextLayout::justify(Justify::Right),
                Node {
                    padding: UiRect::axes(px(10), px(6)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(BAR),
                NoticesText,
            ),
        ],
    ));
    // The battle box, under the top bar in the middle.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(52),
            left: px(0),
            right: px(0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        GlobalZIndex(HUD_LAYER),
        bevy::picking::Pickable::IGNORE,
        children![(
            Text::default(),
            TextFont::from_font_size(15.0),
            TextColor(TEXT),
            TextLayout::justify(Justify::Center),
            Node {
                padding: UiRect::axes(px(14), px(8)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BAR),
            BattleBox,
        )],
    ));
    // The influence overlay's legend, over the bottom left corner.
    let entries = NATIONS.map(|nation| {
        (
            Node {
                column_gap: px(6),
                align_items: AlignItems::Center,
                ..default()
            },
            children![
                (
                    Node {
                        width: px(12),
                        height: px(12),
                        ..default()
                    },
                    BackgroundColor(nation_colour(nation)),
                ),
                label(nation.to_string(), 14.0, TEXT),
            ],
        )
    });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(72),
            left: px(10),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            padding: UiRect::axes(px(10), px(6)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(BAR),
        GlobalZIndex(HUD_LAYER),
        bevy::picking::Pickable::IGNORE,
        Legend,
        Children::spawn((
            Spawn(label("Spheres of influence", 14.0, GOLD)),
            SpawnIter(entries.into_iter()),
        )),
    ));
    // The action bar and the menu, along the bottom.
    let group = |buttons: Vec<Control>| {
        (
            Node {
                column_gap: px(4),
                padding: UiRect::all(px(4)),
                ..default()
            },
            BackgroundColor(BAR),
            Children::spawn(SpawnIter(buttons.into_iter().map(keyed_button))),
        )
    };
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(8),
            left: px(8),
            right: px(8),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::End,
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(24),
            row_gap: px(6),
            ..default()
        },
        GlobalZIndex(HUD_LAYER),
        bevy::picking::Pickable::IGNORE,
        children![group(ACTION_BAR.to_vec()), group(MENU.to_vec()),],
    ));
}

fn show_legend(show: Res<ShowInfluence>, mut legend: Single<&mut Node, With<Legend>>) {
    legend.display = if show.0 { Display::Flex } else { Display::None };
}

/// The 16-point compass name for a heading.
fn compass_point(heading: f32) -> &'static str {
    const POINTS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    let sector = std::f32::consts::TAU / POINTS.len() as f32;
    let index = (normalize_heading(heading) / sector).round() as usize % POINTS.len();
    POINTS[index]
}

fn update_status(
    player: Single<(&Heading, &Speed, &Purse, &Crew, &Hold), With<Player>>,
    clock: Res<WorldClock>,
    wind: Res<Wind>,
    goods: Res<Goods>,
    standing: Res<Standing>,
    time: Res<Time<Virtual>>,
    mut status: Single<&mut Text, (With<StatusText>, Without<WarningText>)>,
    mut warning: Single<&mut Text, (With<WarningText>, Without<StatusText>)>,
) {
    let (heading, speed, purse, crew, hold) = *player;
    let people = crew.count as f64 + hold.passengers(&goods);
    let days = days_of_provisions(hold.owned.get(goods.provisions), people).min(999.0);
    let text = format!(
        "{}   {:03.0}° {:.1} kn   Wind {} {:.0} kn   {} pieces of eight   {} men, {}   {:.0} days' stores",
        clock.date(),
        heading.0.to_degrees(),
        speed.0 * MPS_TO_KNOTS,
        compass_point(wind.from),
        wind.speed_mps * MPS_TO_KNOTS,
        purse.0,
        crew.count,
        crew.spirits().name(),
        days,
    );
    if status.0 != text {
        status.0 = text;
    }
    let mut warnings = Vec::new();
    if time.is_paused() {
        warnings.push("PAUSED".to_string());
    }
    for nation in NATIONS {
        if standing.wanted_by(nation) {
            warnings.push(format!("WANTED by {nation}"));
        }
    }
    if crew.is_starving() {
        warnings.push("STARVING".to_string());
    }
    let text = warnings.join("   ");
    if warning.0 != text {
        warning.0 = text;
    }
}

/// Keys and names on the buttons, kept to the bindings, the battle style,
/// and the current speed.
fn update_labels(
    bindings: Res<Bindings>,
    style: Res<BattleStyle>,
    scale: Res<TimeScale>,
    time: Res<Time<Virtual>>,
    mut keys: Query<(&KeyLabel, &mut Text), Without<ControlLabel>>,
    mut names: Query<(&ControlLabel, &mut Text, &mut TextColor), Without<KeyLabel>>,
) {
    for (key, mut text) in &mut keys {
        let name = bindings.key(key.0).map_or("", key_name);
        if text.0 != name {
            text.0 = name.to_string();
        }
    }
    for (label, mut text, mut color) in &mut names {
        let (name, current) = match label.0 {
            Control::CycleBattleStyle => (format!("Style: {}", style.name()), false),
            Control::Speed(index) => (
                format!("×{}", SPEEDS[index]),
                !time.is_paused() && scale.speed == SPEEDS[index],
            ),
            Control::Pause => ("Pause".to_string(), time.is_paused()),
            control => (control.name().to_string(), false),
        };
        if text.0 != name {
            text.0 = name;
        }
        color.set_if_neq(TextColor(if current { GOLD } else { TEXT }));
    }
}

/// Notes the player's passing alerts. Those that stop the clock are pinned
/// instead, until she resumes.
fn note_alerts(
    mut alerts: MessageReader<Alert>,
    player: Single<Entity, With<Player>>,
    names: Query<&Name>,
    real: Res<Time<Real>>,
    mut notices: ResMut<Notices>,
) {
    for alert in alerts.read() {
        if alert.ship == *player && !alert.kind.needs_decision() {
            notices.post(
                describe(*alert, |entity| name_of(&names, entity)),
                real.elapsed_secs(),
            );
        }
    }
}

fn name_of(names: &Query<&Name>, entity: Entity) -> String {
    names.get(entity).map_or("a ship", Name::as_str).to_string()
}

fn update_messages(
    notices: Res<Notices>,
    latest: Res<LatestAlert>,
    real: Res<Time<Real>>,
    names: Query<&Name>,
    mut pinned: Single<(&mut Text, &mut Node), (With<PinnedText>, Without<NoticesText>)>,
    mut log: Single<(&mut Text, &mut Node), (With<NoticesText>, Without<PinnedText>)>,
) {
    let call = latest
        .0
        .filter(|alert| alert.kind.needs_decision())
        .map(|alert| describe(alert, |entity| name_of(&names, entity)));
    show_text(&mut pinned, call.unwrap_or_default());
    let lines: Vec<&str> = notices.current(real.elapsed_secs()).collect();
    show_text(&mut log, lines.join("\n"));
}

/// Shows `text` in a boxed text node, hiding the box when there's none.
fn show_text((text, node): &mut (Mut<Text>, Mut<Node>), content: String) {
    let display = if content.is_empty() {
        Display::None
    } else {
        Display::Flex
    };
    if node.display != display {
        node.display = display;
    }
    if text.0 != content {
        text.0 = content;
    }
}

fn update_battle_box(
    player: Single<
        (
            &Transform,
            &Battery,
            &Timbers,
            &Rigging,
            &Crew,
            Option<&Engaged>,
        ),
        With<Player>,
    >,
    enemies: Query<(&Name, &Transform, &Timbers, &Rigging, &Crew, Has<Struck>)>,
    bindings: Res<Bindings>,
    mut text: Single<(&mut Text, &mut Node), With<BattleBox>>,
) {
    let (at, battery, timbers, rigging, crew, engaged) = *player;
    let enemy = engaged.and_then(|engaged| enemies.get(engaged.enemy).ok());
    let line = enemy.map(|enemy| {
        battle_text(
            (at.translation, battery, timbers, rigging, crew),
            enemy,
            bindings.key(Control::Board).map_or("", key_name),
        )
    });
    show_text(&mut text, line.unwrap_or_default());
}

/// The battle box: her state and ours, the guns, and boarding.
fn battle_text(
    (at, battery, timbers, rigging, crew): (Vec3, &Battery, &Timbers, &Rigging, &Crew),
    (name, enemy_at, enemy_timbers, enemy_rigging, enemy_crew, struck): (
        &Name,
        &Transform,
        &Timbers,
        &Rigging,
        &Crew,
        bool,
    ),
    board_key: &str,
) -> String {
    let gun = |side| match battery.reload(side) {
        0.0 => "ready".to_string(),
        seconds => format!("{seconds:.0} s"),
    };
    let distance = at.distance(enemy_at.translation);
    let mut text = format!(
        "{name}, {distance:.0} m{}\nHer timbers {:.0}%, rigging {:.0}%, {} men\nOurs: timbers {:.0}%, rigging {:.0}%, {} men\nPort guns {}   Starboard guns {}",
        if struck { ": she has struck!" } else { "" },
        enemy_timbers.0 * 100.0,
        enemy_rigging.0 * 100.0,
        enemy_crew.count,
        timbers.0 * 100.0,
        rigging.0 * 100.0,
        crew.count,
        gun(Broadside::Port),
        gun(Broadside::Starboard),
    );
    if distance <= BOARDING_RANGE {
        let _ = write!(text, "\n{board_key}: board her!");
    } else if struck {
        text.push_str("\nClose alongside to board");
    }
    text
}

/// What the lookout or the bosun calls out for an alert. `name` names a ship
/// or an officer.
pub(super) fn describe(alert: Alert, name: impl Fn(Entity) -> String) -> String {
    match alert.kind {
        AlertKind::SailSighted { other } => format!("Sail ho! {} in sight.", name(other)),
        AlertKind::Aground => "We've run aground!".to_string(),
        AlertKind::ProvisionsLow => "Provisions are running low.".to_string(),
        AlertKind::DeadlineNear { .. } => "A contract falls due within a day.".to_string(),
        AlertKind::UnderAttack { by } => format!("{} has opened fire on us!", name(by)),
        AlertKind::EnemyStruck { other } => {
            format!("{} has struck her colours!", name(other))
        }
        AlertKind::EnemySank { other } => format!("{} has gone down.", name(other)),
        AlertKind::Sunk => "We're sinking! All is lost. Load your last save.".to_string(),
        AlertKind::PrizeTaken { other } => format!(
            "{} is ours! A prize crew has her, and her mate is our prisoner.",
            name(other)
        ),
        AlertKind::Plundered { other } => format!(
            "We've plundered {}, but have no hands to spare for a prize crew, so she's let go. Hire more men at a tavern to take prizes.",
            name(other)
        ),
        AlertKind::TurnedAway { port, why } => match why {
            Refused::Wanted => format!(
                "{} won't let us in: we're wanted there. Try a port of another flag.",
                name(port)
            ),
            Refused::Enemy => format!(
                "{} won't let in a ship under the colours of its enemy. Hoist others.",
                name(port)
            ),
        },
        AlertKind::Repulsed { other } => {
            format!("Our boarders were thrown back from {}!", name(other))
        }
        AlertKind::Rumour { officer, rumour } => match rumour {
            Rumour::Grumbling => format!(
                "{} was heard grumbling with the hands about the captain's share.",
                name(officer)
            ),
            Rumour::Ashore => format!(
                "{} was seen ashore with the harbourmaster's clerk.",
                name(officer)
            ),
            Rumour::Letters => format!(
                "{} sits up late writing letters he shows no one.",
                name(officer)
            ),
        },
        AlertKind::MutinyPutDown { leader } => match leader {
            Some(officer) => format!(
                "Mutiny! {} led the men against us, but the loyal hands put it down. He's in irons.",
                name(officer)
            ),
            None => "Mutiny! Some of the hands rose against us, but the loyal men put it down."
                .to_string(),
        },
        AlertKind::Mutiny { leader } => format!(
            "Mutiny! {} taken the ship and set us adrift in the boat with the men who stood by us. Word of it will get around.",
            leader.map_or("The hands have".to_string(), |role| format!(
                "Our {} has",
                role.name()
            ))
        ),
        AlertKind::Betrayed { role, port } => format!(
            "Soldiers were waiting on the quay at {}: our {} sold us out. The ship is seized, we've paid a heavy fine, and two of the men waited with a boat.",
            name(port),
            role.name()
        ),
        AlertKind::Terms {
            nations: [a, b],
            terms,
        } => match terms {
            Terms::War => format!("News: {a} and {b} are at war!"),
            Terms::Hostile => format!("News: {a} and {b} are on hostile terms."),
            Terms::Peace => format!("News: {a} and {b} are at peace."),
            Terms::Alliance => format!("News: {a} and {b} are allies."),
        },
        AlertKind::Restless { port } => format!(
            "News: {} is restless. Its people say their masters have abandoned them.",
            name(port)
        ),
        AlertKind::WentOver { port, from, to } => {
            format!("News: {} has gone over from {from} to {to}!", name(port))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::officers::Role;

    #[test]
    fn alerts_read_as_calls_from_the_crew() {
        let ship = Entity::PLACEHOLDER;
        let alert = |kind| Alert { ship, kind };
        let name = |_| "the Ranger".to_string();
        assert_eq!(
            describe(alert(AlertKind::SailSighted { other: ship }), name),
            "Sail ho! the Ranger in sight."
        );
        assert_eq!(
            describe(alert(AlertKind::Aground), name),
            "We've run aground!"
        );
        assert!(
            describe(
                alert(AlertKind::Mutiny {
                    leader: Some(Role::FirstMate)
                }),
                name
            )
            .starts_with("Mutiny! Our First Mate has taken the ship")
        );
    }

    #[test]
    fn compass_points_match_headings() {
        assert_eq!(compass_point(0.0), "N");
        assert_eq!(compass_point(67.5_f32.to_radians()), "ENE");
        assert_eq!(compass_point(180_f32.to_radians()), "S");
        assert_eq!(
            compass_point(355_f32.to_radians()),
            "N",
            "wraps back to north"
        );
        assert_eq!(compass_point(-90_f32.to_radians()), "W");
    }
}
