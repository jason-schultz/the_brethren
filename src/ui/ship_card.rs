//! The ship card: what the captain can tell of another ship at sea, or of a
//! wreck. A left click on a ship or a wreck looks her over; Escape, or a click on open sea, puts the
//! glass down. Her colours, her rig, and her guns can be seen. Her business
//! can only be heard: in a tavern, from a group the captain serves, or from
//! the merchant herself when she's hired the captain to escort her.

use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::hud::{BAR, TEXT, show_text};
use super::{calling, compass_point, nearest_port, stakes_line, tale_text};
use crate::alert::SIGHTING_RANGE;
use crate::allegiance::{Favour, stakes};
use crate::camera::{CameraRig, near_cursor};
use crate::clock::WorldClock;
use crate::combat::Battery;
use crate::controls::{Control, used};
use crate::economy::market::Market;
use crate::escorts::Escort;
use crate::factions::{Colours, GroupKind, Nation, Relations, Serves, nations};
use crate::goods::Goods;
use crate::hunting::{Hunter, Mark, is_quarry};
use crate::math::MPS_TO_KNOTS;
use crate::player::Player;
use crate::port::{Docked, Port};
use crate::sailing::{Heading, Hull, Speed};
use crate::schedule::FrameSet;
use crate::ship_class::ShipClass;
use crate::word::{Happening, Heard, Knowledge, Purpose, Tale};
use crate::wrecks::Wreck;

/// A press and release within this many pixels of each other is a click,
/// not a drag of the map.
const CLICK_SLOP: f32 = 4.0;

/// The ship the captain is looking over.
#[derive(Resource, Debug, Default)]
struct Selected(Option<Entity>);

/// The card's text.
#[derive(Component)]
struct CardText;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Selected>()
        .add_systems(Startup, spawn_card)
        .add_systems(
            Update,
            (
                select_ship.in_set(FrameSet::Input),
                put_the_glass_down
                    .run_if(used(Control::Close))
                    .in_set(FrameSet::Input),
                show_card.after(FrameSet::Input),
            ),
        );
}

fn spawn_card(mut commands: Commands) {
    commands.spawn((
        Text::default(),
        TextFont::from_font_size(14.0),
        TextColor(TEXT),
        Node {
            position_type: PositionType::Absolute,
            top: px(52),
            left: px(10),
            max_width: px(440),
            padding: UiRect::axes(px(10), px(6)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(BAR),
        // About the world, so under the panels and the port screen.
        GlobalZIndex(0),
        CardText,
    ));
}

/// A left click on a ship at sea looks her over, and a click on open sea
/// puts the glass down. Not in port, where the port screen has the mouse,
/// nor through the HUD.
fn select_ship(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<CameraRig>>,
    hovered: Res<HoverMap>,
    at_sea: Query<(), (With<Player>, Without<Docked>)>,
    ships: Query<(Entity, &Transform), (With<Hull>, Without<Player>, Without<Docked>)>,
    wrecks: Query<(Entity, &Transform), With<Wreck>>,
    mut pressed_at: Local<Option<Vec2>>,
    mut selected: ResMut<Selected>,
) {
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if buttons.just_pressed(MouseButton::Left) {
        *pressed_at = Some(cursor);
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    let dragged = pressed_at
        .take()
        .is_none_or(|from| from.distance(cursor) > CLICK_SLOP);
    let over_hud = hovered
        .get(&PointerId::Mouse)
        .is_some_and(|hits| !hits.is_empty());
    if dragged || over_hud || at_sea.is_empty() {
        return;
    }
    let (camera, camera_transform) = *camera;
    selected.0 = ships
        .iter()
        .chain(&wrecks)
        .find(|(_, at)| near_cursor(camera, camera_transform, cursor, at.translation))
        .map(|(ship, _)| ship);
}

fn put_the_glass_down(mut selected: ResMut<Selected>) {
    selected.0 = None;
}

/// The nations whose merchantmen a raider under `colours` preys on.
fn prey(colours: Nation, relations: &Relations) -> Vec<Nation> {
    nations()
        .filter(|&nation| {
            let merchantman = Mark {
                colours: Some(nation),
                warship: false,
                attacking: None,
                standing: None,
                salvaging: None,
            };
            is_quarry(Hunter::Raider(colours), &merchantman, relations)
        })
        .collect()
}

/// A ship's card, from what can be seen of her and what's been heard.
#[derive(Debug)]
struct Card<'a> {
    name: &'a str,
    class: &'a ShipClass,
    guns: u32,
    colours: Option<Nation>,
    serves: GroupKind,
    /// From the player, meters.
    distance: f32,
    heading: f32,
    /// Knots.
    speed: f32,
    /// Where the player is escorting her, and for what fee.
    ward: Option<(&'a str, u32)>,
    /// The latest heard of her, and where or from whom: "Word in
    /// Kingston: ...".
    word: Option<String>,
    /// What hangs on her voyage.
    stakes: Option<String>,
}

fn card_text(card: &Card) -> String {
    let flag = match card.colours {
        Some(nation) => format!("under {} colours", nation.adjective()),
        None => "under no colours".to_string(),
    };
    let mut lines = vec![
        format!(
            "{}: {} of {} guns, {flag}",
            card.name,
            calling(card.class, card.colours, card.serves),
            card.guns
        ),
        format!(
            "{:.1} km off, heading {} at {:.1} knots",
            card.distance / 1000.0,
            compass_point(card.heading),
            card.speed
        ),
    ];
    if let Some((destination, fee)) = card.ward {
        lines.push(format!(
            "Your ward, bound for {destination}: {fee} pieces of eight to see her in. Right-click her to keep company."
        ));
    }
    lines.push(
        card.word.clone().unwrap_or_else(|| {
            "No word of her business. Taverns hear where ships are bound.".into()
        }),
    );
    lines.extend(card.stakes.clone());
    lines.join("\n")
}

/// A wreck's card: whose she was, where she lies, what's been heard of
/// her, and, within sight, what's left to bring up.
fn wreck_text(name: &str, wreck: &Wreck, distance: f32, word: Option<String>) -> String {
    let whose = wreck
        .nation
        .map_or(String::new(), |nation| format!(", {}", nation.adjective()));
    let mut lines = vec![format!("{name}{whose}: {:.1} km off", distance / 1000.0)];
    lines.extend(word);
    lines.push(if distance <= SIGHTING_RANGE {
        let tons: f64 = wreck.cargo.iter().map(|(_, tons)| tons).sum();
        format!(
            "About {tons:.0} t of cargo left{}. Right-click to lie over her and dive.",
            if wreck.purse > 0 {
                ", and her strongbox"
            } else {
                ""
            }
        )
    } else {
        "Right-click to sail there and dive on her.".to_string()
    });
    lines.join("\n")
}

fn show_card(
    mut selected: ResMut<Selected>,
    ships: Query<(
        (
            &Name,
            &ShipClass,
            &Battery,
            Option<&Colours>,
            Option<&Serves>,
        ),
        (&Transform, &Heading, &Speed),
    )>,
    docked: Query<(), With<Docked>>,
    wrecks: Query<(&Name, &Transform, &Wreck)>,
    player: Single<(Entity, &Transform, &Knowledge), With<Player>>,
    escorts: Query<&Escort>,
    ports: Query<(&Name, &Port, &Market, &Favour)>,
    anchorages: Query<(&Name, &Transform), With<Port>>,
    names: Query<&Name>,
    (goods, relations, clock): (Res<Goods>, Res<Relations>, Res<WorldClock>),
    mut text: Single<(&mut Text, &mut Node), With<CardText>>,
) {
    let (me, my_position, knowledge) = *player;
    // She's gone, or come into port.
    if selected.0.is_some_and(|ship| {
        !(ships.contains(ship) || wrecks.contains(ship)) || docked.contains(ship)
    }) {
        selected.0 = None;
    }
    let Some(ship) = selected.0 else {
        show_text(&mut text, String::new());
        return;
    };
    let port_name = |port| names.get(port).map_or("?", Name::as_str);
    let heard = |tale: &Tale| {
        let source = match tale.heard {
            Heard::Saw => "You saw it".to_string(),
            Heard::Tavern(port) => format!("Word in {}", port_name(port)),
            Heard::Service(group) => format!("Word from {group}"),
        };
        let told = tale_text(
            &goods,
            tale,
            clock.elapsed,
            |entity| port_name(entity).to_string(),
            |at| nearest_port(&anchorages, at),
        );
        format!("{source}: {told}")
    };
    if let Ok((name, lies, wreck)) = wrecks.get(ship) {
        let word = knowledge
            .0
            .iter()
            .find(
                |tale| matches!(tale.event, Happening::Wrecked { wreck: Some(w), .. } if w == ship),
            )
            .map(heard);
        let distance = lies.translation.distance(my_position.translation);
        show_text(&mut text, wreck_text(name.as_str(), wreck, distance, word));
        return;
    }
    let Ok(((name, class, battery, colours, serves), (at, heading, speed))) = ships.get(ship)
    else {
        return;
    };
    let colours = colours.map(|colours| colours.0);
    let tale = knowledge.latest_about(ship);
    let sailed = tale.and_then(|tale| match &tale.event {
        Happening::Sailed { purpose, .. } => Some(*purpose),
        _ => None,
    });
    let stakes_text = sailed.and_then(|purpose| match purpose {
        Purpose::Carrying {
            good,
            quantity,
            bound_for,
        } => {
            let (port, held, market, favour) = ports.get(bound_for).ok()?;
            let stakes = stakes(&goods, good, quantity, held.flag, market, favour);
            Some(stakes_line(port.as_str(), held.flag, colours, stakes))
        }
        Purpose::Cruising => {
            let prey: Vec<String> = colours
                .map(|nation| prey(nation, &relations))
                .unwrap_or_default()
                .into_iter()
                .map(|nation| nation.adjective().to_string())
                .collect();
            (!prey.is_empty()).then(|| {
                format!(
                    "While she cruises, {} merchantmen are in danger.",
                    prey.join(" and ")
                )
            })
        }
    });
    let card = Card {
        name: name.as_str(),
        class,
        guns: battery.guns,
        colours,
        serves: serves.map_or(GroupKind::Merchants, |serves| serves.0),
        distance: at.translation.distance(my_position.translation),
        heading: heading.0,
        speed: speed.0 * MPS_TO_KNOTS,
        ward: escorts
            .iter()
            .find(|escort| escort.ward == ship && escort.escort == Some(me))
            .map(|escort| (port_name(escort.destination), escort.fee)),
        word: tale.map(heard),
        stakes: stakes_text,
    };
    show_text(&mut text, card_text(&card));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raiders_endanger_the_shipping_of_nations_hostile_to_them() {
        let relations = Relations::default();
        assert_eq!(
            prey(Nation::PIRATES, &relations),
            [Nation::ENGLAND, Nation::SPAIN]
        );
    }

    #[test]
    fn the_card_tells_only_what_can_be_seen_until_word_is_heard() {
        let mut card = Card {
            name: "Swallow",
            class: &ShipClass::sloop(),
            guns: 4,
            colours: Some(Nation::ENGLAND),
            serves: GroupKind::Merchants,
            distance: 3_200.0,
            heading: 0.0,
            speed: 5.04,
            ward: None,
            word: None,
            stakes: None,
        };
        assert_eq!(
            card_text(&card),
            "Swallow: a merchant sloop of 4 guns, under English colours\n\
             3.2 km off, heading N at 5.0 knots\n\
             No word of her business. Taverns hear where ships are bound."
        );
        card.word = Some("Word in Kingston: Swallow sailed from Kingston.".into());
        card.ward = Some(("Santiago", 60));
        assert!(card_text(&card).ends_with(
            "Your ward, bound for Santiago: 60 pieces of eight to see her in. Right-click her to keep company.\n\
             Word in Kingston: Swallow sailed from Kingston."
        ));
    }
}
