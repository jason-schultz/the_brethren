//! The HUD, kept to the edges of the screen so the sea stays clear: a slim
//! bar along the top with the date, the time controls, and the essentials;
//! passing messages on the right; a battle box only while engaged; and
//! along the bottom, the action bar and the menu of panels. Every button
//! sends the same control its key does.

use std::fmt::Write;

use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button, observe};

use super::{Notices, button_style, calling, compass_point, purpose_text};
use crate::alert::{Alert, AlertKind};
use crate::cargo::{Hold, Purse};
use crate::clock::WorldClock;
use crate::combat::gunnery::Broadside;
use crate::combat::tactics::BOARDING_RANGE;
use crate::combat::{Battery, Engaged, Struck, Timbers};
use crate::controls::{ACTION_BAR, Bindings, Control, ControlUsed, MENU, key_name};
use crate::crew::{Crew, days_of_provisions};
use crate::factions::{Colours, DeedKind, GroupKind, Serves, Standing, Terms, nations};
use crate::goods::Goods;
use crate::loyalty::Rumour;
use crate::math::MPS_TO_KNOTS;
use crate::pardons::{GRACE_DAYS, Plea, under_grace};
use crate::player::{BattleStyle, Player};
use crate::port::Refused;
use crate::sailing::{Heading, Rigging, Speed};
use crate::service::StruckOff;
use crate::ship_class::ShipClass;
use crate::time_control::LatestAlert;
use crate::time_scale::{SPEEDS, TimeScale};
use crate::visuals::influence::ShowInfluence;
use crate::visuals::nation_colour;
use crate::wind::Wind;

/// Over the world and the port screen, so the HUD stays usable in port.
const HUD_LAYER: i32 = 2;
pub(super) const BAR: Color = Color::srgba(0.05, 0.04, 0.03, 0.72);
pub(super) const TEXT: Color = Color::srgb(0.93, 0.9, 0.82);
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
    let entries: Vec<_> = nations()
        .map(|nation| {
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
        })
        .collect();
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

/// The player's ship, as the status line tells of her.
#[derive(QueryData)]
struct Status {
    heading: &'static Heading,
    speed: &'static Speed,
    purse: &'static Purse,
    crew: &'static Crew,
    hold: &'static Hold,
    standing: &'static Standing,
}

/// A boxed text node: its text, and the box shown or hidden around it.
type TextBox = (&'static mut Text, &'static mut Node);
/// The pinned message's box, and the notices'.
type PinnedBox = (With<PinnedText>, Without<NoticesText>);
type NoticesBox = (With<NoticesText>, Without<PinnedText>);

/// The player's ship in a fight, as the battle box tells of her...
#[derive(QueryData)]
struct OurSide {
    at: &'static Transform,
    battery: &'static Battery,
    timbers: &'static Timbers,
    rigging: &'static Rigging,
    crew: &'static Crew,
    engaged: Option<&'static Engaged>,
}

/// ...and her enemy.
#[derive(QueryData)]
struct TheirSide {
    name: &'static Name,
    at: &'static Transform,
    timbers: &'static Timbers,
    rigging: &'static Rigging,
    crew: &'static Crew,
    struck: Has<Struck>,
}

fn update_status(
    player: Single<Status, With<Player>>,
    clock: Res<WorldClock>,
    wind: Res<Wind>,
    goods: Res<Goods>,
    time: Res<Time<Virtual>>,
    mut status: Single<&mut Text, (With<StatusText>, Without<WarningText>)>,
    mut warning: Single<&mut Text, (With<WarningText>, Without<StatusText>)>,
) {
    let StatusItem {
        heading,
        speed,
        purse,
        crew,
        hold,
        standing,
    } = player.into_inner();
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
    for nation in nations() {
        if under_grace(standing, nation, clock.elapsed) {
            warnings.push(format!("WANTED by {nation}, but under its Act of Grace"));
        } else if standing.wanted_by(nation) {
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
/// instead, until she resumes. A ship sighted is named with what she is.
fn note_alerts(
    mut alerts: MessageReader<Alert>,
    player: Single<Entity, With<Player>>,
    names: Query<&Name>,
    ships: Query<(&ShipClass, Option<&Colours>, Option<&Serves>)>,
    goods: Res<Goods>,
    real: Res<Time<Real>>,
    mut notices: ResMut<Notices>,
) {
    for alert in alerts.read() {
        if alert.ship != *player || alert.kind.needs_decision() {
            continue;
        }
        let sighting = matches!(alert.kind, AlertKind::SailSighted { .. });
        let name = |entity| {
            let name = name_of(&names, entity);
            match ships.get(entity) {
                Ok((class, colours, serves)) if sighting => {
                    let serves = serves.map_or(GroupKind::Merchants, |serves| serves.0);
                    format!("{name}, {},", calling(class, colours.map(|c| c.0), serves))
                }
                _ => name,
            }
        };
        notices.post(describe(*alert, &goods, name), real.elapsed_secs());
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
    goods: Res<Goods>,
    mut pinned: Single<TextBox, PinnedBox>,
    mut log: Single<TextBox, NoticesBox>,
) {
    let call = latest
        .0
        .filter(|alert| alert.kind.needs_decision())
        .map(|alert| describe(alert, &goods, |entity| name_of(&names, entity)));
    show_text(&mut pinned, call.unwrap_or_default());
    let lines: Vec<&str> = notices.current(real.elapsed_secs()).collect();
    show_text(&mut log, lines.join("\n"));
}

/// Shows `text` in a boxed text node, hiding the box when there's none.
pub(super) fn show_text((text, node): &mut (Mut<Text>, Mut<Node>), content: String) {
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
    player: Single<OurSide, With<Player>>,
    enemies: Query<TheirSide>,
    bindings: Res<Bindings>,
    mut text: Single<(&mut Text, &mut Node), With<BattleBox>>,
) {
    let OurSideItem {
        at,
        battery,
        timbers,
        rigging,
        crew,
        engaged,
    } = player.into_inner();
    let enemy = engaged
        .and_then(|engaged| enemies.get(engaged.enemy).ok())
        .map(|them| {
            (
                them.name,
                them.at,
                them.timbers,
                them.rigging,
                them.crew,
                them.struck,
            )
        });
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

/// What the lookout or the bosun calls out for an alert. `name` names a
/// ship, a port, or an officer.
pub(super) fn describe(alert: Alert, goods: &Goods, name: impl Fn(Entity) -> String) -> String {
    match alert.kind {
        AlertKind::SailSighted { other, hostile } => format!(
            "Sail ho! {} in sight.{}",
            name(other),
            if hostile { " She'll come for us!" } else { "" }
        ),
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
        AlertKind::Promoted { post } => format!(
            "Word from {}: you're made {}, with {}.",
            post.group,
            post.title(),
            post.perk()
        ),
        AlertKind::StruckOff { nation, why } => match why {
            StruckOff::TurnedOnThem => {
                format!(
                    "You've struck at {nation}'s own, and every one of its groups has struck you off."
                )
            }
            StruckOff::Wanted => format!("{nation} wants you, and its groups have struck you off."),
            StruckOff::Deserted => format!(
                "You've hoisted other colours: the navy has struck you off for a deserter. {nation}'s other groups keep you."
            ),
        },
        AlertKind::Bounty { group, amount } => {
            format!(
                "{amount} pieces of eight from {group} for that one, under our letter of marque."
            )
        }
        AlertKind::Grace { nation } => format!(
            "News: {nation} proclaims an Act of Grace! For {GRACE_DAYS:.0} days, any captain it wants may come into its ports with a governor to be pardoned."
        ),
        AlertKind::Pardoned { nation, plea } => match plea {
            Plea::Grace => {
                format!("We've come in under the Act of Grace: {nation} has pardoned us.")
            }
            Plea::Petition => format!("The lawyer has done his work: {nation} has pardoned us."),
        },
        AlertKind::Introduced { group } => format!(
            "Word from {group}: they know us now, and would take us on. Join them at their office in one of {}'s ports.",
            group.nation
        ),
        AlertKind::Intelligence {
            ship,
            from,
            purpose,
            group,
        } => {
            let bound_for = purpose.bound_for().map(&name).unwrap_or_default();
            format!(
                "Word from {group}: {} has sailed from {} {}.",
                name(ship),
                name(from),
                purpose_text(goods, purpose, &bound_for)
            )
        }
        AlertKind::WardSafe { ward, fee } => format!(
            "{} is safe in port: {fee} pieces of eight for seeing her in.",
            name(ward)
        ),
        AlertKind::WardAlone { ward } => format!(
            "{} came in with no sight of us, and her merchants won't pay for that escort.",
            name(ward)
        ),
        AlertKind::WardAttacked { ward, by } => format!(
            "{} is under attack by {}! Right-click the attacker to engage her.",
            name(ward),
            name(by)
        ),
        AlertKind::Holed { timbers, on_reef } => format!(
            "{} We're holed: timbers at {timbers}%.",
            if on_reef {
                "We've struck a reef!"
            } else {
                "We've struck hard!"
            }
        ),
        AlertKind::Wrecked { wreck } => format!(
            "She's breaking up! We've taken to the boat with the men.{}",
            wreck.map_or(String::new(), |wreck| format!(
                " Her wreck lies in shallow water: {}.",
                name(wreck)
            ))
        ),
        AlertKind::Salvaging { wreck } => format!(
            "Our divers are down on the {}. Lie hove to here and they'll bring up what they can.",
            name(wreck)
        ),
        AlertKind::WordOut {
            nation,
            kind,
            victim,
        } => {
            let ship = victim.map_or("a ship".to_string(), &name);
            let what = match kind {
                DeedKind::Attacked => format!("we attacked {ship}"),
                DeedKind::Captured => format!("we took {ship}"),
                DeedKind::Sank => format!("we sank {ship}"),
                DeedKind::RaisedPort => "we lay in port as the town rose".to_string(),
                DeedKind::FalseReport => format!("our account of {ship} was false"),
                _ => "of what we did".to_string(),
            };
            format!("Word that {what} has reached {nation}.")
        }
        AlertKind::WardLost => {
            "We've lost the ship we were escorting. Her merchants will want an account of it in port."
                .to_string()
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
        let goods = Goods::standard();
        assert_eq!(
            describe(
                alert(AlertKind::SailSighted {
                    other: ship,
                    hostile: false
                }),
                &goods,
                name
            ),
            "Sail ho! the Ranger in sight."
        );
        assert_eq!(
            describe(
                alert(AlertKind::SailSighted {
                    other: ship,
                    hostile: true
                }),
                &goods,
                name
            ),
            "Sail ho! the Ranger in sight. She'll come for us!"
        );
        assert_eq!(
            describe(alert(AlertKind::Aground), &goods, name),
            "We've run aground!"
        );
        assert!(
            describe(
                alert(AlertKind::Mutiny {
                    leader: Some(Role::FirstMate)
                }),
                &goods,
                name
            )
            .starts_with("Mutiny! Our First Mate has taken the ship")
        );
    }
}
