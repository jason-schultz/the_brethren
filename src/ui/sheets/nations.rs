//! The nations sheet: standing with each nation and its groups, the ways
//! to a pardon, how the nations stand with each other, how each port feels
//! about its masters, and what moves it all.

use bevy::prelude::*;

use super::{SheetAction, act, close_row, closing_hint, is_open};
use crate::allegiance::Favour;
use crate::clock::{GameDate, WorldClock};
use crate::controls::{Bindings, Sheet};
use crate::factions::{DeedKind, Nation, Relations, Standing, WANTED, nations, pairs};
use crate::pardons::{GRACE_DAYS, NOTORIOUS, grace_until, next_grace};
use crate::player::Player;
use crate::port::Port;
use crate::schedule::FrameSet;
use crate::service::{RANKS, Service};
use crate::ui::mood_line;
use crate::ui::panel::{Panel, Row, Shown, Style, show};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        refresh
            .after(FrameSet::Orders)
            .run_if(is_open(Sheet::Nations)),
    );
}

/// Standing in a word.
fn regard(standing: &Standing, nation: Nation) -> &'static str {
    match standing.with(nation) {
        _ if standing.wanted_by(nation) => {
            "WANTED: their ports turn you away, and their navy hunts you"
        }
        value if value < 0.0 => "distrusted: their informers watch you in port",
        value if value < 20.0 => "neutral",
        _ => "friendly",
    }
}

/// A nation's groups, and the player's standing and rank with each.
fn groups_line(standing: &Standing, service: &Service, nation: Nation) -> String {
    let each: Vec<String> = nation
        .groups()
        .into_iter()
        .map(|group| {
            let rank = service
                .post(group)
                .map_or(String::new(), |post| format!(" ({})", post.title()));
            format!("{group} {:+.0}{rank}", standing.of(group))
        })
        .collect();
    format!("    {}", each.join(", "))
}

/// When a crown's Act of Grace is, in words.
fn grace_line(nation: Nation, now: f64) -> Option<String> {
    let day = |at: f64| GameDate::at(at).day_name();
    if let Some(until) = grace_until(nation, now) {
        Some(format!(
            "{nation}: an Act of Grace is in force until {}",
            day(until)
        ))
    } else {
        next_grace(nation, now).map(|next| format!("{nation}: next Act of Grace on {}", day(next)))
    }
}

/// A port as the sheet shows it: its name, flag, and favour.
type Town<'a> = (&'a str, Nation, &'a Favour);

fn build(
    standing: &Standing,
    service: &Service,
    relations: &Relations,
    now: f64,
    ports: &[Town],
    hint: String,
) -> Panel<SheetAction> {
    let mut rows = vec![Row::text(Style::Heading, "Your standing")];
    for nation in nations() {
        rows.push(Row::text(
            Style::Body,
            format!(
                "{nation} {:+.0}: {}",
                standing.with(nation),
                regard(standing, nation)
            ),
        ));
        rows.push(Row::text(
            Style::Faded,
            groups_line(standing, service, nation),
        ));
    }
    rows.push(Row::text(
        Style::Faded,
        format!(
            "A group takes captains at {:+.0} at its office in one of its ports. You serve one nation at a time.",
            RANKS[0]
        ),
    ));
    rows.push(Row::text(Style::Heading, "Pardons"));
    rows.extend(
        nations()
            .filter_map(|nation| grace_line(nation, now))
            .map(|line| Row::text(Style::Body, line)),
    );
    rows.push(Row::text(
        Style::Faded,
        format!(
            "Under an Act ({GRACE_DAYS:.0} days), come into a port with a governor to be pardoned, unless you're at {NOTORIOUS:.0}. A lawyer in any tavern will carry a petition for a fee."
        ),
    ));
    rows.push(Row::text(Style::Heading, "Between the nations"));
    for [a, b] in pairs() {
        rows.push(Row::text(
            Style::Body,
            format!(
                "{a} and {b}: {} ({:+.0})",
                relations.terms(a, b).name(),
                relations.between(a, b)
            ),
        ));
    }
    rows.push(Row::text(Style::Heading, "The ports"));
    for &(name, flag, favour) in ports {
        rows.push(Row::text(
            Style::Body,
            format!(
                "{name} ({}): {}",
                favour.tier.name(),
                mood_line(flag, favour)
            ),
        ));
    }
    rows.push(Row::text(
        Style::Faded,
        "Ports favour whoever brings what they need, under the colours she flies. A restless port goes over to the nation it favours when one of her armed ships lies in its roads. Capitals hold through all but the worst neglect; havens change hands often.",
    ));
    rows.push(Row::text(Style::Heading, "What moves standing"));
    let points = |kind: DeedKind| kind.standing();
    rows.push(Row::text(
        Style::Faded,
        format!(
            "Attacking a ship {:+.0}, taking her {:+.0}, sinking her {:+.0}: with her navy if she's a warship, else her merchants",
            points(DeedKind::Attacked),
            points(DeedKind::Captured),
            points(DeedKind::Sank)
        ),
    ));
    rows.push(Row::text(
        Style::Faded,
        format!(
            "Delivering a contract {:+.0} with the port's merchants, or its governor for soldiers. Losing your ship {:+.0} with every governor. Lying in a port as it rises {:+.0} with its governor",
            points(DeedKind::Delivered),
            points(DeedKind::LostCommand),
            points(DeedKind::RaisedPort)
        ),
    ));
    rows.push(Row::text(
        Style::Faded,
        format!(
            "The rest of the nation takes half. Blows earn a third as much with its enemies' navies, and cost that with its allies'. Bad standing fades a little every day. At {WANTED:.0} you're wanted."
        ),
    ));
    rows.push(close_row());
    Panel::new("Nations", hint, rows)
}

fn refresh(
    player: Single<(&Standing, &Service), With<Player>>,
    relations: Res<Relations>,
    clock: Res<WorldClock>,
    ports: Query<(&Name, &Port, &Favour)>,
    bindings: Res<Bindings>,
    panel: Single<(Entity, &mut Shown<SheetAction>)>,
    mut commands: Commands,
) {
    let mut ports: Vec<Town> = ports
        .iter()
        .map(|(name, port, favour)| (name.as_str(), port.flag, favour))
        .collect();
    ports.sort_by_key(|&(name, ..)| name);
    let (standing, service) = *player;
    let content = build(
        standing,
        service,
        &relations,
        clock.elapsed,
        &ports,
        closing_hint(&bindings, Sheet::Nations),
    );
    let (entity, mut shown) = panel.into_inner();
    show(&mut commands, (entity, &mut shown), content, act);
}
