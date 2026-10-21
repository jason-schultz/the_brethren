//! The captain sheet: the captain beside his ship, how both stand, whom
//! she serves, and the letters of introduction a new captain carries.

use bevy::prelude::*;

use super::{SheetAction, act, close_row, closing_hint, is_open};
use crate::cargo::{Hold, Purse};
use crate::combat::{Battery, Engaged, Timbers};
use crate::controls::{Bindings, Sheet};
use crate::crew::Crew;
use crate::economy::jobs::{Carrier, Job};
use crate::factions::{Colours, Group, Nation, Standing, nations};
use crate::goods::Goods;
use crate::math::MPS_TO_KNOTS;
use crate::officers::{Berth, Officer, Role};
use crate::player::Player;
use crate::port::Port;
use crate::sailing::physics::sail_speed;
use crate::sailing::{Hull, Laden, Rigging, SailHandling};
use crate::schedule::FrameSet;
use crate::service::Service;
use crate::ship_class::ShipClass;
use crate::ui::panel::{Panel, Row, Shown, Style, show};
use crate::ui::{location_name, standing_line};
use crate::visuals::portrait::{PORTRAIT_SIZE, Portrait};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        refresh
            .after(FrameSet::Orders)
            .run_if(is_open(Sheet::Captain)),
    );
}

/// What the captain sheet shows.
struct View<'a> {
    ship: &'a str,
    class: &'a ShipClass,
    timbers: f32,
    rigging: f32,
    guns: u32,
    /// Her best speed as she is, with the wind abeam, knots.
    speed: f32,
    /// Tons in her hold, and its size.
    hold: (f64, f64),
    purse: u32,
    crew: &'a Crew,
    officers: Vec<(&'a str, Role)>,
    standing: &'a Standing,
    service: &'a Service,
    colours: Option<Nation>,
    /// In action, she's been seen for what she is.
    engaged: bool,
    /// The errands of the groups who've written to her, in words.
    letters: Vec<(Group, String)>,
}

/// A group's errand for her, in words: what to carry, and where to go.
fn errand(job: &Job, aboard: bool, goods: &Goods, ports: &Query<(&Name, &Port)>) -> Option<String> {
    let (origin, from) = ports.get(job.origin).ok()?;
    let (destination, _) = ports.get(job.destination).ok()?;
    let what = format!(
        "{} {}",
        job.quantity,
        goods.get(job.good).name.to_lowercase()
    );
    Some(if aboard {
        format!("Their {what} are aboard: deliver them to {destination}.")
    } else {
        let office = job.office(goods, from).map_or("town", location_name);
        format!("Carry {what} from {origin} to {destination}: ask at the {office} in {origin}.")
    })
}

/// The groups who've written to her, with their pitch and errand.
fn letter_rows(view: &View) -> Vec<Row<SheetAction>> {
    if view.letters.is_empty() {
        return Vec::new();
    }
    let mut rows = vec![Row::text(Style::Heading, "Letters of introduction")];
    for (group, errand) in &view.letters {
        rows.push(Row::text(
            Style::Body,
            format!("From {group}: \"{}\"", group.pitch()),
        ));
        rows.push(Row::text(Style::Faded, format!("    {errand}")));
    }
    rows
}

/// The colours she flies, and buttons to hoist the others.
fn colours_row(view: &View) -> Row<SheetAction> {
    let flying = match view.colours {
        Some(nation) => format!("Flying {} colours", nation.adjective()),
        None => "Flying no colours".to_string(),
    };
    let mut row = Row::text(Style::Body, flying);
    if !view.engaged {
        for nation in nations().filter(|&n| Some(n) != view.colours) {
            row = row.with_button(
                format!("Hoist {}", nation.adjective()),
                SheetAction::Hoist(nation),
            );
        }
    }
    row
}

/// The groups she serves, each with a button to resign.
fn service_rows(service: &Service) -> Vec<Row<SheetAction>> {
    if service.0.is_empty() {
        return vec![Row::text(
            Style::Faded,
            "You serve no one. Groups take proven captains at their offices in port.",
        )];
    }
    service
        .0
        .iter()
        .map(|post| {
            Row::text(
                Style::Body,
                format!("{} in {}: {}", post.title(), post.group, post.perk()),
            )
            .with_button("Resign", SheetAction::Resign(post.group))
        })
        .collect()
}

fn build(view: &View, hint: String, picture: Handle<Image>) -> Panel<SheetAction> {
    let officers = if view.officers.is_empty() {
        "none".to_string()
    } else {
        view.officers
            .iter()
            .map(|(name, role)| format!("{name} ({})", role.name()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut rows = vec![
        Row::text(Style::Heading, format!("The {}", view.ship)),
        Row::text(
            Style::Body,
            format!(
                "{}: timbers {:.0}%, rigging {:.0}%, {} guns",
                view.class.name,
                view.timbers * 100.0,
                view.rigging * 100.0,
                view.guns
            ),
        ),
        Row::text(
            Style::Body,
            format!(
                "Best speed {:.1} knots with the wind abeam. Hold {:.0} of {:.0} tons",
                view.speed, view.hold.0, view.hold.1
            ),
        ),
        Row::text(Style::Heading, "Her captain"),
        Row::text(Style::Body, format!("{} pieces of eight", view.purse)),
        Row::text(
            Style::Body,
            format!(
                "{} men, {}. Officers: {officers}",
                view.crew.count,
                view.crew.spirits().name()
            ),
        ),
        Row::text(Style::Body, standing_line(view.standing)),
        colours_row(view),
        Row::text(Style::Heading, "Her service"),
    ];
    rows.extend(service_rows(view.service));
    rows.extend(letter_rows(view));
    rows.push(close_row());
    Panel {
        picture: Some((picture, PORTRAIT_SIZE.as_vec2() / 3.0)),
        ..Panel::new("Captain", hint, rows)
    }
}

fn refresh(
    player: Single<
        (
            Entity,
            &Name,
            &ShipClass,
            (&Battery, &Timbers, &Rigging),
            (&Hull, &SailHandling, &Laden),
            (&Hold, &Purse, &Crew),
            (Option<&Colours>, Has<Engaged>, &Standing, &Service),
        ),
        With<Player>,
    >,
    officers: Query<(&Name, &Officer, &Berth)>,
    goods: Res<Goods>,
    jobs: Query<(&Job, Option<&Carrier>)>,
    ports: Query<(&Name, &Port)>,
    bindings: Res<Bindings>,
    portrait: Res<Portrait>,
    panel: Single<(Entity, &mut Shown<SheetAction>)>,
    mut commands: Commands,
) {
    let (ship, name, class, (battery, timbers, rigging), sailing, (hold, purse, crew), flag) =
        *player;
    let (colours, engaged, standing, service) = flag;
    let (hull, handling, laden) = sailing;
    let view = View {
        ship: name.as_str(),
        class,
        timbers: timbers.0,
        rigging: rigging.0,
        guns: battery.guns,
        speed: sail_speed(hull.speed_mps, handling.0, rigging.0, laden.0) * MPS_TO_KNOTS,
        hold: (hold.tons_used(&goods), hold.capacity_tons),
        purse: purse.0,
        crew,
        officers: officers
            .iter()
            .filter(|(_, _, berth)| **berth == Berth::Aboard(ship))
            .map(|(name, officer, _)| (name.as_str(), officer.role))
            .collect(),
        standing,
        service,
        colours: colours.map(|colours| colours.0),
        engaged,
        letters: jobs
            .iter()
            .filter_map(|(job, carrier)| {
                let group = job.introduction?;
                let aboard = carrier.is_some_and(|carrier| carrier.0 == ship);
                Some((group, errand(job, aboard, &goods, &ports)?))
            })
            .collect(),
    };
    let content = build(
        &view,
        closing_hint(&bindings, Sheet::Captain),
        portrait.0.clone(),
    );
    let (entity, mut shown) = panel.into_inner();
    show(&mut commands, (entity, &mut shown), content, act);
}
