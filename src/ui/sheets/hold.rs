//! The hold sheet: what she carries, and the contracts she's under.

use bevy::prelude::*;

use super::{SheetAction, act, close_row, closing_hint, is_open};
use crate::cargo::{Hold, Lading};
use crate::clock::GameDate;
use crate::controls::{Bindings, Sheet};
use crate::economy::jobs::{Carrier, Job};
use crate::goods::{Carriage, Goods};
use crate::player::Player;
use crate::schedule::FrameSet;
use crate::ui::panel::{Panel, Row, Shown, Style, show};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        refresh.after(FrameSet::Orders).run_if(is_open(Sheet::Hold)),
    );
}

/// One line per good in `lading`.
fn lines(goods: &Goods, lading: &Lading) -> Vec<Row<SheetAction>> {
    lading
        .iter()
        .map(|(good, quantity)| {
            let unit = match goods.carriage(good) {
                Carriage::Hold => " t",
                Carriage::Berths => "",
            };
            Row::text(
                Style::Body,
                format!("{}: {quantity:.0}{unit}", goods.get(good).name),
            )
        })
        .collect()
}

/// A contract carried, with the name of the port it's bound for.
type Contract<'a> = (&'a Job, &'a str);

fn build(goods: &Goods, hold: &Hold, contracts: &[Contract], hint: String) -> Panel<SheetAction> {
    let mut rows = vec![Row::text(
        Style::Body,
        format!(
            "{:.0} of {:.0} tons used",
            hold.tons_used(goods),
            hold.capacity_tons
        ),
    )];
    rows.push(Row::text(Style::Heading, "Her own cargo"));
    let own = lines(goods, &hold.owned);
    if own.is_empty() {
        rows.push(Row::text(Style::Faded, "Nothing."));
    }
    rows.extend(own);
    rows.push(Row::text(Style::Heading, "Under contract"));
    if contracts.is_empty() {
        rows.push(Row::text(
            Style::Faded,
            "No contracts. Ports in need post them at the governor's house,",
        ));
        rows.push(Row::text(
            Style::Faded,
            "the merchants' exchange, and the tavern.",
        ));
    }
    rows.extend(lines(goods, &hold.consigned));
    for (job, destination) in contracts {
        rows.push(Row::text(
            Style::Faded,
            format!(
                "{} {} to {destination} by {}, for {} pieces of eight",
                job.quantity,
                goods.get(job.good).name,
                GameDate::at(job.deadline),
                job.payment
            ),
        ));
    }
    rows.push(close_row());
    Panel::new("Hold", hint, rows)
}

fn refresh(
    player: Single<(Entity, &Hold), With<Player>>,
    jobs: Query<(&Job, &Carrier)>,
    names: Query<&Name>,
    goods: Res<Goods>,
    bindings: Res<Bindings>,
    panel: Single<(Entity, &mut Shown<SheetAction>)>,
    mut commands: Commands,
) {
    let (ship, hold) = *player;
    let contracts: Vec<Contract> = jobs
        .iter()
        .filter(|(_, carrier)| carrier.0 == ship)
        .map(|(job, _)| (job, names.get(job.destination).map_or("?", Name::as_str)))
        .collect();
    let content = build(
        &goods,
        hold,
        &contracts,
        closing_hint(&bindings, Sheet::Hold),
    );
    let (entity, mut shown) = panel.into_inner();
    show(&mut commands, (entity, &mut shown), content, act);
}
