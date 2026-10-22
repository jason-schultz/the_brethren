//! The crew sheet: the men and their morale, the officers, and prisoners
//! who might sign on.

use bevy::ecs::query::QueryData;
use bevy::prelude::*;

use super::{SheetAction, act, close_row, closing_hint, is_open};
use crate::cargo::{Hold, Purse};
use crate::controls::{Bindings, Sheet};
use crate::crew::{Complement, Crew, SHORE_LEAVE_PER_MAN, Spirits, days_of_provisions};
use crate::goods::Goods;
use crate::officers::{Berth, Officer, OfficerAct, Quarterdeck, check_order, posts};
use crate::player::Player;
use crate::port::Docked;
use crate::schedule::FrameSet;
use crate::ui::panel::{Panel, Row, Shown, Style, show};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        refresh.after(FrameSet::Orders).run_if(is_open(Sheet::Crew)),
    );
}

/// What a crew's morale means for the ship.
fn effect(spirits: Spirits) -> &'static str {
    match spirits {
        Spirits::Mutinous => {
            "They work sullenly and desert in port, and a ringleader would find followers."
        }
        Spirits::Sullen => "They work slowly, and desert in port.",
        Spirits::Steady | Spirits::Content => "They work as well as any crew.",
        Spirits::Eager => "They work with a will: faster sailing and quicker reloads.",
    }
}

/// An officer aboard, or a prisoner.
struct Hand<'a> {
    entity: Entity,
    name: &'a str,
    officer: &'a Officer,
    berth: Berth,
}

struct View<'a> {
    crew: &'a Crew,
    complement: &'a Complement,
    provisions_days: f64,
    deck: Quarterdeck<'a>,
    officers: Vec<Hand<'a>>,
    prisoners: Vec<Hand<'a>>,
}

/// The buttons for each of `acts` that `hand` would go through for.
fn with_orders(
    mut row: Row<SheetAction>,
    view: &View,
    hand: &Hand,
    acts: &[(OfficerAct, &str)],
) -> Row<SheetAction> {
    for &(act, label) in acts {
        if check_order(act, hand.officer, hand.berth, &view.deck).is_ok() {
            row = row.with_button(
                label,
                SheetAction::Officer {
                    officer: hand.entity,
                    act,
                },
            );
        }
    }
    row
}

fn build(view: &View, hint: String) -> Panel<SheetAction> {
    let crew = view.crew;
    let spirits = crew.spirits();
    let starving = if crew.is_starving() {
        ". They're STARVING"
    } else {
        ""
    };
    let mut rows = vec![
        Row::text(
            Style::Body,
            format!(
                "{} men: {} to sail her, {} to fight her guns, {} berths",
                crew.count, view.complement.min, view.complement.full, view.complement.berths
            ),
        ),
        Row::text(
            Style::Body,
            format!(
                "Morale {:.0} of 100: {}. {}",
                crew.morale,
                spirits.name(),
                effect(spirits)
            ),
        ),
        Row::text(
            Style::Body,
            format!(
                "Provisions for {:.0} days{starving}",
                view.provisions_days.min(999.0)
            ),
        ),
        Row::text(
            Style::Faded,
            "Morale settles higher in port than at sea. Prizes and victories lift it;",
        ),
        Row::text(
            Style::Faded,
            format!(
                "hunger, losses, and defeats sink it. A run ashore at a tavern ({SHORE_LEAVE_PER_MAN} pieces a man) lifts it."
            ),
        ),
        Row::text(Style::Heading, "Officers"),
    ];
    if view.officers.is_empty() {
        rows.push(Row::text(
            Style::Faded,
            "No officers. Hire them at a tavern, or recruit the mates of ships you take.",
        ));
    }
    let dismiss = if view.deck.docked.is_some() {
        "Put ashore"
    } else {
        "Maroon"
    };
    for hand in &view.officers {
        let irons = if hand.officer.confined {
            " (in irons)"
        } else {
            ""
        };
        let row = Row::text(
            Style::Body,
            format!(
                "{}, {}{irons}: {}",
                hand.name,
                hand.officer.role.name(),
                hand.officer.role.duty()
            ),
        );
        rows.push(with_orders(
            row,
            view,
            hand,
            &[
                (OfficerAct::Confine, "Put in irons"),
                (OfficerAct::Release, "Release"),
                (OfficerAct::Dismiss, dismiss),
            ],
        ));
    }
    if !view.prisoners.is_empty() {
        rows.push(Row::text(Style::Heading, "Prisoners"));
        for hand in &view.prisoners {
            let row = Row::text(
                Style::Body,
                format!(
                    "{}, taken from a prize, would serve as {}",
                    hand.name,
                    hand.officer.role.name()
                ),
            );
            rows.push(with_orders(
                row,
                view,
                hand,
                &[
                    (OfficerAct::SignOn, "Sign him on"),
                    (OfficerAct::Dismiss, dismiss),
                ],
            ));
        }
    }
    for line in [
        "Officers don't show where their loyalty lies. Watch for the signs, and",
        "put a man you doubt in irons: he'll do no duty, and no mischief.",
    ] {
        rows.push(Row::text(Style::Faded, line));
    }
    rows.push(close_row());
    Panel::new("Crew", hint, rows)
}

/// The player's ship's company, as the crew sheet shows it.
#[derive(QueryData)]
struct Muster {
    entity: Entity,
    crew: &'static Crew,
    complement: &'static Complement,
    hold: &'static Hold,
    purse: &'static Purse,
    docked: Option<&'static Docked>,
}

fn refresh(
    player: Single<Muster, With<Player>>,
    officers: Query<(Entity, &Name, &Officer, &Berth)>,
    goods: Res<Goods>,
    bindings: Res<Bindings>,
    panel: Single<(Entity, &mut Shown<SheetAction>)>,
    mut commands: Commands,
) {
    let MusterItem {
        entity: ship,
        crew,
        complement,
        hold,
        purse,
        docked,
    } = player.into_inner();
    let posts = posts(
        ship,
        officers
            .iter()
            .map(|(_, _, officer, berth)| (officer, berth)),
    );
    let hands = |wanted: fn(Berth, Entity) -> bool| {
        officers
            .iter()
            .filter(move |(_, _, _, berth)| wanted(**berth, ship))
            .map(|(entity, name, officer, berth)| Hand {
                entity,
                name: name.as_str(),
                officer,
                berth: *berth,
            })
            .collect()
    };
    let people = crew.count as f64 + hold.passengers(&goods);
    let view = View {
        crew,
        complement,
        provisions_days: days_of_provisions(hold.owned.get(goods.provisions), people),
        deck: Quarterdeck {
            ship,
            docked: docked.map(|docked| docked.port),
            purse: *purse,
            posts: &posts,
        },
        officers: hands(|berth, ship| berth == Berth::Aboard(ship)),
        prisoners: hands(|berth, ship| berth == Berth::Captive(ship)),
    };
    let content = build(&view, closing_hint(&bindings, Sheet::Crew));
    let (entity, mut shown) = panel.into_inner();
    show(&mut commands, (entity, &mut shown), content, act);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::officers::{Loyalty, Role};
    use crate::ship_class::ShipClass;

    fn actions(panel: &Panel<SheetAction>) -> Vec<SheetAction> {
        panel
            .rows
            .iter()
            .flat_map(|row| row.buttons.iter().map(|(_, action)| *action))
            .collect()
    }

    #[test]
    fn officers_can_be_put_in_irons_and_prisoners_signed_on_unless_the_post_is_filled() {
        let ship = Entity::from_raw_u32(1).unwrap();
        let (gunner, prisoner) = (
            Entity::from_raw_u32(2).unwrap(),
            Entity::from_raw_u32(3).unwrap(),
        );
        let officer = |role| Officer {
            role,
            loyalty: Loyalty::Turncoat,
            confined: false,
        };
        let (aboard, captive) = (officer(Role::Gunner), officer(Role::Gunner));
        let crew = Crew::new(12);
        let complement = ShipClass::sloop().complement;
        let posts = [Role::Gunner];
        let view = View {
            crew: &crew,
            complement: &complement,
            provisions_days: 30.0,
            deck: Quarterdeck {
                ship,
                docked: None,
                purse: Purse(0),
                posts: &posts,
            },
            officers: vec![Hand {
                entity: gunner,
                name: "John Hale",
                officer: &aboard,
                berth: Berth::Aboard(ship),
            }],
            prisoners: vec![Hand {
                entity: prisoner,
                name: "Israel Hands",
                officer: &captive,
                berth: Berth::Captive(ship),
            }],
        };
        let actions = actions(&build(&view, String::new()));
        let order = |officer, act| SheetAction::Officer { officer, act };
        assert!(actions.contains(&order(gunner, OfficerAct::Confine)));
        assert!(!actions.contains(&order(gunner, OfficerAct::Release)));
        assert!(actions.contains(&order(prisoner, OfficerAct::Dismiss)));
        assert!(
            !actions.contains(&order(prisoner, OfficerAct::SignOn)),
            "she has a gunner"
        );
    }
}
