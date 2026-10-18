//! The nations sheet: standing with each nation, how the nations stand
//! with each other, how each port feels about its masters, and what moves
//! it all.

use bevy::prelude::*;

use super::{SheetAction, act, close_row, closing_hint, is_open};
use crate::allegiance::Favour;
use crate::controls::{Bindings, Sheet};
use crate::factions::{DeedKind, NATIONS, Nation, Relations, Standing, WANTED, pairs, reputation};
use crate::port::Port;
use crate::schedule::FrameSet;
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

/// What a deed does to standing, in words, from [`reputation`] itself.
fn deed_line(what: &str, kind: DeedKind) -> String {
    let [(_, against), (_, brethren)] = reputation(kind, Nation::England);
    let mut line = format!("{what}: {against:+.0} with her nation");
    if brethren != 0.0 {
        line.push_str(&format!(", {brethren:+.0} with the Brethren"));
    }
    line
}

/// A port as the sheet shows it: its name, flag, and favour.
type Town<'a> = (&'a str, Nation, &'a Favour);

fn build(
    standing: &Standing,
    relations: &Relations,
    ports: &[Town],
    hint: String,
) -> Panel<SheetAction> {
    let mut rows = vec![Row::text(Style::Heading, "Your standing")];
    for nation in NATIONS {
        rows.push(Row::text(
            Style::Body,
            format!(
                "{nation} {:+.0}: {}",
                standing.with(nation),
                regard(standing, nation)
            ),
        ));
    }
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
            format!("{name}: {}", mood_line(flag, favour)),
        ));
    }
    rows.push(Row::text(
        Style::Faded,
        "Ports favour whoever brings what they need, under the colours she flies.",
    ));
    rows.push(Row::text(
        Style::Faded,
        "A restless port goes over to the nation it favours when her ship is in harbour.",
    ));
    rows.push(Row::text(Style::Heading, "What moves standing"));
    for (what, kind) in [
        ("Attacking a ship", DeedKind::Attacked),
        ("Taking her", DeedKind::Captured),
        ("Sinking her", DeedKind::Sank),
    ] {
        rows.push(Row::text(Style::Faded, deed_line(what, kind)));
    }
    let [(_, delivered), _] = reputation(DeedKind::Delivered, Nation::England);
    let [(_, lost), _] = reputation(DeedKind::LostCommand, Nation::England);
    let [(_, raised), _] = reputation(DeedKind::RaisedPort, Nation::England);
    rows.push(Row::text(
        Style::Faded,
        format!("Delivering a contract: {delivered:+.0} with the port's nation"),
    ));
    rows.push(Row::text(
        Style::Faded,
        format!("Losing your ship, to mutiny or the law: {lost:+.0} with every nation"),
    ));
    rows.push(Row::text(
        Style::Faded,
        format!("Lying in a port as it rises against its nation: {raised:+.0} with her"),
    ));
    rows.push(Row::text(
        Style::Faded,
        format!("Bad standing fades a little every day. At {WANTED:.0} you're wanted."),
    ));
    rows.push(close_row());
    Panel::new("Nations", hint, rows)
}

fn refresh(
    standing: Res<Standing>,
    relations: Res<Relations>,
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
    let content = build(
        &standing,
        &relations,
        &ports,
        closing_hint(&bindings, Sheet::Nations),
    );
    let (entity, mut shown) = panel.into_inner();
    show(&mut commands, (entity, &mut shown), content, act);
}
