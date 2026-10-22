//! Screen-space interface: the HUD, the panels the captain can open, and
//! the port and battle screens. Reads simulation state, and changes it only
//! by sending order messages.

mod battle_screen;
mod hud;
#[cfg(feature = "chronicle")]
mod note;
mod panel;
mod port_screen;
mod sheets;
mod ship_card;

use bevy::ecs::system::SystemParam;
use bevy::math::FloatOrd;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::Button;

use crate::allegiance::{Favour, OWNER_REST, Stakes};
use crate::clock::{DAY, HOUR, WorldClock};
use crate::crew::days_of_provisions;
use crate::economy::market::Trader;
use crate::factions::{DeedKind, GroupKind, Nation, Standing, nations};
use crate::goods::{Carriage, GoodId, Goods};
use crate::math::normalize_heading;
use crate::port::{Location, Port};
use crate::ship_class::ShipClass;
use crate::word::{Fate, Happening, Place, Purpose, Tale};

const BUTTON_IDLE: Color = Color::srgb(0.3, 0.2, 0.12);
const BUTTON_HOVER: Color = Color::srgb(0.42, 0.29, 0.17);
const BUTTON_PRESSED: Color = Color::srgb(0.2, 0.13, 0.08);

/// How long a notice stays on screen, real seconds.
const NOTICE_SECONDS: f32 = 10.0;
/// The most notices shown at once; older ones make way.
const NOTICES_SHOWN: usize = 5;
/// A port whose favour for its owner falls this low is said to grumble.
const GRUMBLING: f32 = OWNER_REST - 15.0;

/// Passing messages for the captain, such as "Game saved." or a sail in
/// sight, shown in the HUD for a few seconds of real time.
#[derive(Resource, Debug, Default)]
pub struct Notices(Vec<(String, f32)>);

/// The notices, and the real time they're posted at.
#[derive(SystemParam)]
pub struct Noticeboard<'w> {
    notices: ResMut<'w, Notices>,
    time: Res<'w, Time<Real>>,
}

impl Noticeboard<'_> {
    /// Shows `text`, from now.
    pub fn post(&mut self, text: impl Into<String>) {
        self.notices.post(text, self.time.elapsed_secs());
    }
}

impl Notices {
    /// Shows `text`, as of real time `now`.
    pub fn post(&mut self, text: impl Into<String>, now: f32) {
        self.0.push((text.into(), now));
        if self.0.len() > NOTICES_SHOWN {
            self.0.remove(0);
        }
    }

    /// The notices still fresh at `now`, oldest first.
    pub fn current(&self, now: f32) -> impl Iterator<Item = &str> {
        self.0
            .iter()
            .filter(move |(_, at)| now - at < NOTICE_SECONDS)
            .map(|(text, _)| text.as_str())
    }
}

pub struct UiPlugin;

/// The full Fira Mono. Bevy's built-in default is an ASCII-only subset of
/// it, which draws `°`, `×`, dashes, and accented place names as boxes.
const FONT: &[u8] = include_bytes!("../../assets/fonts/FiraMono-Regular.ttf");

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        // Replacing the default font asset gives every text node, world
        // labels included, the full font without naming it anywhere.
        app.world_mut()
            .resource_mut::<Assets<Font>>()
            .insert(AssetId::default(), Font::from_bytes(FONT.to_vec()))
            .expect("the default font id is always valid");
        app.init_resource::<Notices>()
            .add_plugins((
                hud::plugin,
                sheets::plugin,
                port_screen::plugin,
                battle_screen::plugin,
                ship_card::plugin,
            ))
            .add_systems(Update, color_buttons);
        #[cfg(feature = "chronicle")]
        app.add_plugins(note::plugin);
    }
}

/// The look shared by every button, laid out as `node`. Pair it with a
/// `Button` and a text child.
fn button_style(node: Node) -> impl Bundle {
    (node, Hovered::default(), BackgroundColor(BUTTON_IDLE))
}

fn color_buttons(mut buttons: Query<(&Hovered, Has<Pressed>, &mut BackgroundColor), With<Button>>) {
    for (hovered, pressed, mut background) in &mut buttons {
        background.0 = match (pressed, hovered.get()) {
            (true, _) => BUTTON_PRESSED,
            (false, true) => BUTTON_HOVER,
            (false, false) => BUTTON_IDLE,
        };
    }
}

fn location_name(location: Location) -> &'static str {
    match location {
        Location::Tavern => "Tavern",
        Location::Market => "Market",
        Location::Shipyard => "Shipyard",
        Location::GovernorsHouse => "Governor's House",
        Location::MerchantsExchange => "Merchants' Exchange",
    }
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

/// What a ship is, by whom she sails for: "a merchant sloop".
fn calling(class: &ShipClass, colours: Option<Nation>, serves: GroupKind) -> String {
    let class = class.name.to_lowercase();
    match (serves, colours) {
        (GroupKind::Navy, Some(nation)) if !nation.is_crown() => {
            format!("a {} {class}", nation.adjective())
        }
        (GroupKind::Navy, _) => format!("a {class} of war"),
        _ => format!("a merchant {class}"),
    }
}

/// What a quantity of `good` is counted in, after the number: tons for
/// freight, and nothing for people, who are counted by head.
fn unit(goods: &Goods, good: GoodId) -> &'static str {
    match goods.carriage(good) {
        Carriage::Hold => " t",
        Carriage::Berths => "",
    }
}

/// A quantity of a good, in a sentence: "20 t sugar", "30 soldiers".
fn amount(goods: &Goods, good: GoodId, quantity: u32) -> String {
    format!(
        "{quantity}{} {}",
        unit(goods, good),
        goods.get(good).name.to_lowercase()
    )
}

/// What a ship sailed to do, to follow her name: "with 20 t sugar for
/// Kingston", where she's bound for `bound_for`.
fn purpose_text(goods: &Goods, purpose: Purpose, bound_for: &str) -> String {
    match purpose {
        Purpose::Carrying { good, quantity, .. } => {
            format!("with {} for {bound_for}", amount(goods, good, quantity))
        }
        Purpose::Cruising => "to cruise for prizes".to_string(),
    }
}

/// How a ship was lost, as it's told: "taken by Ranger", with `blames`
/// naming the ship that did it.
fn fate_text(fate: Fate, blames: &str) -> String {
    match fate {
        Fate::Taken(_) => format!("taken by {blames}"),
        Fate::Sunk(_) => format!("sunk by {blames}"),
        Fate::Wrecked => "wrecked".to_string(),
    }
}

/// What a captain was told of something, in a sentence, as of `now`.
/// `name` names ships and ports, and `off` names the port nearest a place
/// at sea.
fn tale_text(
    goods: &Goods,
    tale: &Tale,
    now: f64,
    name: impl Fn(Entity) -> String,
    off: impl Fn(Vec3) -> String,
) -> String {
    let place = match tale.place {
        Place::Port(port) => format!("at {}", name(port)),
        Place::Sea(at) => format!("off {}", off(at)),
    };
    let when = ago(now - tale.at);
    match &tale.event {
        Happening::Sailed {
            name: ship,
            purpose,
            ..
        } => {
            let from = match tale.place {
                Place::Port(port) => name(port),
                Place::Sea(at) => off(at),
            };
            let bound_for = purpose.bound_for().map(&name).unwrap_or_default();
            format!(
                "{ship} sailed from {from} {when}, {}.",
                purpose_text(goods, *purpose, &bound_for)
            )
        }
        Happening::Deed(deed) => {
            let doer = &deed.doer_name;
            let victim = deed
                .victim
                .as_ref()
                .map_or("a ship".to_string(), |(_, victim)| victim.clone());
            let what = match deed.kind {
                DeedKind::Attacked => format!("{doer} attacked {victim}"),
                DeedKind::Captured => format!("{doer} took {victim}"),
                DeedKind::Sank => format!("{doer} sank {victim}"),
                DeedKind::Delivered => format!("{doer} delivered a contract for {}", deed.group),
                DeedKind::LostCommand => format!("The captain of the {doer} lost her ship"),
                DeedKind::RaisedPort => format!(
                    "The town rose against {}, with the {doer} in harbour",
                    deed.group.nation
                ),
                DeedKind::Introduced => format!("{doer} ran an errand for {}", deed.group),
                DeedKind::Escorted => format!("{doer} saw {victim} safe in"),
                DeedKind::LostWard => format!("{doer} lost {victim}, which she was escorting"),
                DeedKind::Salvaged => format!("{doer} salvaged the wreck of {victim}"),
                DeedKind::FalseReport => {
                    format!("{doer}'s account of how {victim} was lost was found false")
                }
            };
            format!("{what} {place} {when}.")
        }
        Happening::Overdue {
            name: ship,
            from,
            bound_for,
            suspects,
            ..
        } => {
            let talk = match suspects.as_slice() {
                [] => "No one knows what became of her.".to_string(),
                [(_, one)] => format!("{one} was seen in those waters about then."),
                [many @ .., (_, last)] => format!(
                    "{} and {last} were seen in those waters about then.",
                    many.iter()
                        .map(|(_, name)| name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
            format!(
                "{ship}, from {}, was posted overdue at {} {when}. {talk}",
                name(*from),
                name(*bound_for),
            )
        }
        Happening::Wrecked {
            name: ship,
            cargo,
            wreck,
            ..
        } => {
            let cargo = cargo.map_or(String::new(), |(good, quantity)| {
                format!(", with {}", amount(goods, good, quantity))
            });
            let lies = if wreck.is_some() {
                "her wreck lies in shallow water"
            } else {
                "she lies too deep to reach"
            };
            format!("{ship} went down {place} {when}{cargo}; {lies}.")
        }
        Happening::Account {
            teller_name,
            name: ship,
            says,
            blames,
            ..
        } => format!(
            "{teller_name} gave an account {place} {when}: {ship} was {}.",
            fate_text(*says, blames.as_deref().unwrap_or("a stranger"))
        ),
    }
}

/// What it takes to tell a tale: the goods, the date, and the names of
/// ships and ports.
#[derive(SystemParam)]
pub(crate) struct Tales<'w, 's> {
    pub goods: Res<'w, Goods>,
    pub clock: Res<'w, WorldClock>,
    names: Query<'w, 's, &'static Name>,
    anchorages: Query<'w, 's, (&'static Name, &'static Transform), With<Port>>,
}

impl Tales<'_, '_> {
    /// `tale`, told.
    pub fn tell(&self, tale: &Tale) -> String {
        tale_text(
            &self.goods,
            tale,
            self.clock.elapsed,
            |entity| self.name(entity).to_string(),
            |at| nearest_port(&self.anchorages, at),
        )
    }

    /// The name of a ship or port, or "?" if it has none.
    pub fn name(&self, entity: Entity) -> &str {
        self.names.get(entity).map_or("?", Name::as_str)
    }
}

/// The name of the port nearest `at`.
fn nearest_port(ports: &Query<(&Name, &Transform), With<Port>>, at: Vec3) -> String {
    ports
        .iter()
        .min_by_key(|(_, anchorage)| FloatOrd(anchorage.translation.distance(at)))
        .map_or("the coast".to_string(), |(name, _)| name.to_string())
}

/// How long ago something `seconds` back was, in words.
fn ago(seconds: f64) -> String {
    let (count, unit) = if seconds >= DAY {
        ((seconds / DAY).floor(), "day")
    } else if seconds >= HOUR {
        ((seconds / HOUR).floor(), "hour")
    } else {
        return "within the hour".to_string();
    };
    let plural = if count == 1.0 { "" } else { "s" };
    format!("{count:.0} {unit}{plural} ago")
}

/// What hangs on a ship under `colours` reaching `port`, held by `owner`,
/// with a cargo whose [`Stakes`] those are.
fn stakes_line(
    port: &str,
    owner: Nation,
    colours: Option<Nation>,
    stakes: Option<Stakes>,
) -> String {
    let Some(stakes) = stakes else {
        return format!("{port} isn't short of them: only her cargo is at stake.");
    };
    let mut line = format!(
        "Arriving, she fills {:.0}% of {port}'s need",
        stakes.fills * 100.0
    );
    if let Some(nation) = colours {
        line.push_str(&format!(
            " and wins {nation} {:.0} favour there",
            stakes.favour
        ));
    }
    line.push('.');
    if stakes.short {
        line.push_str(&format!(
            " Lost, {port} stays short, and {owner} goes on losing favour there."
        ));
    }
    match stakes.rising {
        Some((now, landed)) => line.push_str(&format!(
            " {port} is restless, and may rise: {:.0}% a day, {:.0}% once they land.",
            now * 100.0,
            landed * 100.0
        )),
        None if stakes.restless => line.push_str(&format!(" {port} is restless.")),
        None => {}
    }
    line
}

/// The player's standing with every nation, and who wants her.
fn standing_line(standing: &Standing) -> String {
    let each: Vec<String> = nations()
        .map(|nation| {
            let wanted = if standing.wanted_by(nation) {
                " (WANTED)"
            } else {
                ""
            };
            format!("{nation} {:+.0}{wanted}", standing.with(nation))
        })
        .collect();
    format!("Standing: {}", each.join("   "))
}

/// How a port held by `owner` feels about it, and its favour for every
/// nation: "loyal to England; favour England 55, ...".
fn mood_line(owner: Nation, favour: &Favour) -> String {
    let mood = match favour.pretender(owner) {
        Some(pretender) => format!("RESTLESS, and would go over to {pretender}"),
        None if favour.restless(owner) => "RESTLESS".to_string(),
        None if favour.of(owner) < GRUMBLING => format!("grumbling against {owner}"),
        None => format!("loyal to {owner}"),
    };
    let each: Vec<String> = nations()
        .map(|nation| format!("{nation} {:.0}", favour.of(nation)))
        .collect();
    format!("{mood}; favour {}", each.join(", "))
}

/// Two lines summing up the player's ship: money and cargo, then crew and stores.
fn ship_status_line(goods: &Goods, ship: &Trader) -> String {
    let people = ship.crew.count as f64 + ship.hold.passengers(goods);
    let provisions = ship.hold.owned.get(goods.provisions);
    let hungry = if ship.crew.is_starving() {
        "   STARVING"
    } else {
        ""
    };
    format!(
        "Purse {} pieces of eight   Hold {:.0}/{:.0} t\nCrew {} (needs {}, {} to fight, {} berths), {}   Provisions {:.0} days{hungry}",
        ship.purse.0,
        ship.hold.tons_used(goods),
        ship.hold.capacity_tons,
        ship.crew.count,
        ship.complement.min,
        ship.complement.full,
        ship.complement.berths,
        ship.crew.spirits().name(),
        days_of_provisions(provisions, people).min(999.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_fade_and_make_way() {
        let mut notices = Notices::default();
        notices.post("Game saved.", 0.0);
        assert_eq!(notices.current(1.0).collect::<Vec<_>>(), ["Game saved."]);
        assert_eq!(notices.current(NOTICE_SECONDS + 1.0).count(), 0);
        for i in 0..=NOTICES_SHOWN {
            notices.post(format!("{i}"), 20.0);
        }
        let shown: Vec<&str> = notices.current(20.0).collect();
        assert_eq!(shown.len(), NOTICES_SHOWN);
        assert_eq!(shown[0], "1", "the oldest made way");
    }

    #[test]
    fn a_towns_mood_names_who_it_would_go_over_to() {
        let mut favour = Favour::held_by(Nation::ENGLAND);
        assert_eq!(
            mood_line(Nation::ENGLAND, &favour),
            "loyal to England; favour England 55, Spain 25, the Brethren 25"
        );
        favour.set(Nation::ENGLAND, 10.0);
        favour.set(Nation::PIRATES, 30.0);
        assert!(
            mood_line(Nation::ENGLAND, &favour)
                .starts_with("RESTLESS, and would go over to the Brethren;")
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

    #[test]
    fn a_ship_is_known_by_whom_she_sails_for() {
        let sloop = ShipClass::sloop();
        assert_eq!(
            calling(&sloop, Some(Nation::ENGLAND), GroupKind::Merchants),
            "a merchant sloop"
        );
        assert_eq!(
            calling(&sloop, Some(Nation::ENGLAND), GroupKind::Navy),
            "a sloop of war"
        );
        assert_eq!(
            calling(&sloop, Some(Nation::PIRATES), GroupKind::Navy),
            "a pirate sloop"
        );
    }

    #[test]
    fn an_overdue_ship_names_who_was_about() {
        let goods = Goods::standard();
        let ship = |name: &str| (Entity::PLACEHOLDER, name.to_string());
        let tale = |suspects| Tale {
            news: crate::word::NewsId(0),
            at: 0.0,
            place: Place::Port(Entity::PLACEHOLDER),
            event: Happening::Overdue {
                ship: Entity::PLACEHOLDER,
                name: "Dove".into(),
                from: Entity::PLACEHOLDER,
                bound_for: Entity::PLACEHOLDER,
                suspects,
            },
            heard: crate::word::Heard::Saw,
        };
        let text = |suspects| {
            tale_text(
                &goods,
                &tale(suspects),
                2.0 * DAY,
                |_| "Nassau".into(),
                |_| String::new(),
            )
        };
        assert_eq!(
            text(vec![]),
            "Dove, from Nassau, was posted overdue at Nassau 2 days ago. No one knows what became of her."
        );
        assert!(
            text(vec![ship("Ranger"), ship("Swallow"), ship("Player Sloop")]).ends_with(
                "Ranger, Swallow and Player Sloop were seen in those waters about then."
            )
        );
    }

    #[test]
    fn times_past_read_in_round_words() {
        assert_eq!(ago(10.0 * 60.0), "within the hour");
        assert_eq!(ago(HOUR), "1 hour ago");
        assert_eq!(ago(5.5 * HOUR), "5 hours ago");
        assert_eq!(ago(2.2 * DAY), "2 days ago");
    }

    #[test]
    fn a_ships_purpose_reads_after_her_name() {
        let goods = Goods::standard();
        let carrying = |key: &str| Purpose::Carrying {
            good: goods.id(key).unwrap(),
            quantity: 20,
            bound_for: Entity::PLACEHOLDER,
        };
        assert_eq!(
            purpose_text(&goods, carrying("manufactures"), "Kingston"),
            "with 20 t manufactured goods for Kingston"
        );
        assert_eq!(
            purpose_text(&goods, carrying("soldiers"), "Kingston"),
            "with 20 soldiers for Kingston"
        );
        assert_eq!(
            purpose_text(&goods, Purpose::Cruising, ""),
            "to cruise for prizes"
        );
    }

    #[test]
    fn the_stakes_say_what_arriving_or_not_would_do() {
        let short = Stakes {
            fills: 0.5,
            favour: 12.5,
            short: true,
            restless: true,
            rising: Some((0.4, 0.1)),
        };
        assert_eq!(
            stakes_line("Santiago", Nation::SPAIN, Some(Nation::SPAIN), Some(short)),
            "Arriving, she fills 50% of Santiago's need and wins Spain 12 favour there. \
             Lost, Santiago stays short, and Spain goes on losing favour there. \
             Santiago is restless, and may rise: 40% a day, 10% once they land."
        );
        assert_eq!(
            stakes_line("Santiago", Nation::SPAIN, None, None),
            "Santiago isn't short of them: only her cargo is at stake."
        );
    }

    #[test]
    fn standing_shows_every_nation_and_who_wants_her() {
        let mut standing = Standing::default();
        standing.set(crate::factions::Nation::ENGLAND, -40.0);
        assert_eq!(
            standing_line(&standing),
            "Standing: England -40 (WANTED)   Spain +0   the Brethren +0"
        );
    }
}
