//! The battle screen, for the abstract battle style: when the player is
//! engaged, the world pauses and the battle is fought by rounds here.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::panel::{Panel, Row, Shown, Style, screen, show};
use crate::combat::rounds::{Move, Range, Round};
use crate::combat::{Engaged, RoundOrder, Rounds, Struck, Timbers};
use crate::crew::Crew;
use crate::player::{BattleStyle, Player};
use crate::prizes::BoardOrder;
use crate::sailing::Rigging;
use crate::schedule::FrameSet;

/// The screen's root node.
#[derive(Component)]
struct BattleScreen;

/// What a battle screen button does.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum BattleAction {
    Play(Move),
    Board,
}

pub(super) fn plugin(app: &mut App) {
    app.add_observer(open_battle_screen)
        .add_observer(close_battle_screen)
        .add_systems(
            Update,
            // After orders, so the screen shows a round's results at once.
            (fight_by_rounds, refresh).chain().after(FrameSet::Orders),
        );
}

/// Starts a battle by rounds when the player is engaged under the abstract
/// style, and ends it when she isn't. The world waits while it lasts.
fn fight_by_rounds(
    style: Res<BattleStyle>,
    player: Single<(Entity, &Transform, Option<&Engaged>, Has<Rounds>), With<Player>>,
    ships: Query<&Transform>,
    mut time: ResMut<Time<Virtual>>,
    mut commands: Commands,
) {
    let (ship, at, engaged, by_rounds) = *player;
    let abstract_style = *style == BattleStyle::Abstract;
    match (engaged, by_rounds) {
        (Some(engaged), false) if abstract_style => {
            let distance = ships.get(engaged.enemy).map_or(f32::INFINITY, |enemy| {
                enemy.translation.distance(at.translation)
            });
            commands.entity(ship).insert(Rounds::starting_at(distance));
        }
        (None, true) => {
            commands.entity(ship).remove::<Rounds>();
        }
        (Some(_), true) if !abstract_style => {
            commands.entity(ship).remove::<Rounds>();
        }
        (Some(_), true) => time.pause(),
        _ => {}
    }
}

fn open_battle_screen(
    add: On<Add<Rounds>>,
    players: Query<(), With<Player>>,
    mut commands: Commands,
) {
    if players.contains(add.entity) {
        commands.spawn(screen::<BattleAction>(BattleScreen));
    }
}

/// Closing the screen lets the world go on.
fn close_battle_screen(
    remove: On<Remove<Rounds>>,
    players: Query<(), With<Player>>,
    screens: Query<Entity, With<BattleScreen>>,
    mut time: ResMut<Time<Virtual>>,
    mut commands: Commands,
) {
    if !players.contains(remove.entity) {
        return;
    }
    for screen in &screens {
        commands.entity(screen).despawn();
    }
    time.unpause();
}

/// A ship's state, as the screen describes it.
fn condition(timbers: &Timbers, rigging: &Rigging, crew: &Crew) -> String {
    format!(
        "timbers {:.0}%, rigging {:.0}%, {} men",
        timbers.0 * 100.0,
        rigging.0 * 100.0,
        crew.count
    )
}

fn range_name(range: Range) -> &'static str {
    match range {
        Range::Alongside => "alongside",
        Range::Gunshot => "within gunshot",
        Range::Distant => "at long range",
    }
}

/// What the last round amounted to, in a line.
fn report(play: Move, round: &Round) -> String {
    let what = match play {
        Move::Close => "We bore down on her",
        Move::Broadside => "We gave her a broadside",
        Move::Rake => "We raked her rigging",
        Move::BreakOff => "We tried to break off",
    };
    let range = round.range.map_or("out of reach", range_name);
    format!(
        "{what}. Now {range}. We did {:.0}% to her timbers and {:.0}% to her rigging; she did {:.0}% and {:.0}% to ours.",
        round.dealt.timbers * 100.0,
        round.dealt.rigging * 100.0,
        round.taken.timbers * 100.0,
        round.taken.rigging * 100.0,
    )
}

fn refresh(
    player: Single<(&Engaged, &Rounds, &Timbers, &Rigging, &Crew), With<Player>>,
    enemies: Query<(&Name, &Timbers, &Rigging, &Crew, Has<Struck>)>,
    panel: Single<(Entity, &mut Shown<BattleAction>)>,
    mut commands: Commands,
) {
    let (engaged, rounds, timbers, rigging, crew) = *player;
    let Ok((name, her_timbers, her_rigging, her_crew, struck)) = enemies.get(engaged.enemy) else {
        return;
    };
    let mut rows = vec![
        Row::text(
            Style::Body,
            format!("Our ship: {}", condition(timbers, rigging, crew)),
        ),
        Row::text(
            Style::Body,
            format!(
                "{name}: {}{}",
                condition(her_timbers, her_rigging, her_crew),
                if struck {
                    ". She has struck her colours!"
                } else {
                    ""
                }
            ),
        ),
    ];
    if let Some((play, round)) = &rounds.last {
        rows.push(Row::text(Style::Faded, report(*play, round)));
    }
    let alongside = rounds.range == Range::Alongside;
    let mut moves = Row::text(Style::Body, "");
    if !alongside {
        moves = moves.with_button("Close in", BattleAction::Play(Move::Close));
    }
    if !struck && rounds.range != Range::Distant {
        moves = moves
            .with_button("Broadside", BattleAction::Play(Move::Broadside))
            .with_button("Rake her rigging", BattleAction::Play(Move::Rake));
    }
    if alongside {
        moves = moves.with_button("Board her", BattleAction::Board);
    }
    if !struck {
        moves = moves.with_button("Break off", BattleAction::Play(Move::BreakOff));
    }
    rows.push(moves);
    let content = Panel::new(
        format!("Battle with {name}"),
        format!("She lies {}.", range_name(rounds.range)),
        rows,
    );
    let (entity, mut shown) = panel.into_inner();
    show(&mut commands, (entity, &mut shown), content, act);
}

fn act(
    activate: On<Activate>,
    actions: Query<&BattleAction>,
    player: Single<Entity, With<Player>>,
    mut rounds: MessageWriter<RoundOrder>,
    mut boardings: MessageWriter<BoardOrder>,
) {
    let Ok(&action) = actions.get(activate.entity) else {
        return;
    };
    let ship = *player;
    match action {
        BattleAction::Play(play) => {
            rounds.write(RoundOrder { ship, play });
        }
        BattleAction::Board => {
            boardings.write(BoardOrder { ship });
        }
    }
}
