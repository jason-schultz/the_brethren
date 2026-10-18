//! Screen-space interface: the HUD, the panels the captain can open, and
//! the port and battle screens. Reads simulation state, and changes it only
//! by sending order messages.

mod battle_screen;
mod hud;
mod panel;
mod port_screen;
mod sheets;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::Button;

use crate::allegiance::{Favour, OWNER_REST};
use crate::crew::days_of_provisions;
use crate::economy::market::Trader;
use crate::factions::{NATIONS, Nation, Standing};
use crate::goods::Goods;

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
            ))
            .add_systems(Update, color_buttons);
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

/// The player's standing with every nation, and who wants her.
fn standing_line(standing: &Standing) -> String {
    let each: Vec<String> = NATIONS
        .iter()
        .map(|&nation| {
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
    let each: Vec<String> = NATIONS
        .iter()
        .map(|&nation| format!("{nation} {:.0}", favour.of(nation)))
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
        let mut favour = Favour::held_by(Nation::England);
        assert_eq!(
            mood_line(Nation::England, &favour),
            "loyal to England; favour England 55, Spain 25, the Brethren 25"
        );
        favour.set(Nation::England, 10.0);
        favour.set(Nation::Pirates, 30.0);
        assert!(
            mood_line(Nation::England, &favour)
                .starts_with("RESTLESS, and would go over to the Brethren;")
        );
    }

    #[test]
    fn standing_shows_every_nation_and_who_wants_her() {
        let mut standing = Standing::default();
        standing.set(crate::factions::Nation::England, -40.0);
        assert_eq!(
            standing_line(&standing),
            "Standing: England -40 (WANTED)   Spain +0   the Brethren +0"
        );
    }
}
