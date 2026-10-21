//! The fleet sheet: the flagship and her prizes, their orders, and moving
//! cargo and men between ships that are together.

use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::{OpenSheet, closing_hint, is_open};
use crate::camera::CameraRig;
use crate::cargo::Hold;
use crate::combat::Timbers;
use crate::controls::{Bindings, Sheet};
use crate::crew::{Complement, Crew};
use crate::fleet::{Aboard, Load, Transfer, check_transfer, together};
use crate::goods::{GoodId, Goods};
use crate::player::Player;
use crate::port::{Docked, Port};
use crate::prizes::{OrderPrize, Prize, PrizeOrders};
use crate::sailing::Rigging;
use crate::schedule::FrameSet;
use crate::ui::panel::{Panel, Row, Shown, Style, show};

/// Lots offered for moving cargo and men.
const LOTS: [u32; 2] = [1, 10];

/// What a fleet sheet button does.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FleetAction {
    Order(OrderPrize),
    Move(Transfer),
    /// Look at this ship.
    Center(Entity),
    Close,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        // After orders, so the sheet shows their results the same frame.
        refresh
            .after(FrameSet::Orders)
            .run_if(is_open(Sheet::Fleet)),
    );
}

/// A ship as the fleet screen shows her.
struct ShipView<'a> {
    entity: Entity,
    name: &'a str,
    /// Where she is, in words.
    place: String,
    aboard: Aboard<'a>,
    timbers: f32,
    rigging: f32,
    /// Her orders, if she's a prize.
    orders: Option<PrizeOrders>,
    /// Whether she's together with the flagship, to trade cargo and men.
    alongside: bool,
}

struct Context<'a> {
    goods: &'a Goods,
    hint: String,
    flagship: ShipView<'a>,
    prizes: Vec<ShipView<'a>>,
    ports: Vec<(Entity, &'a str)>,
}

fn port_name<'a>(ctx: &Context<'a>, port: Entity) -> &'a str {
    ctx.ports
        .iter()
        .find(|(entity, _)| *entity == port)
        .map_or("port", |(_, name)| name)
}

fn summary(ctx: &Context, ship: &ShipView) -> String {
    format!(
        "{}. {} men, hold {:.0}/{:.0} t, timbers {:.0}%, rigging {:.0}%",
        ship.place,
        ship.aboard.crew.count,
        ship.aboard.hold.tons_used(ctx.goods),
        ship.aboard.hold.capacity_tons,
        ship.timbers * 100.0,
        ship.rigging * 100.0
    )
}

fn orders_text(ctx: &Context, orders: PrizeOrders) -> String {
    match orders {
        PrizeOrders::Follow => "Following you".to_string(),
        PrizeOrders::Wait => "Waiting where she is".to_string(),
        PrizeOrders::SailTo(port) => format!("Bound for {}", port_name(ctx, port)),
    }
}

/// Buttons for moving `load`s each way between the flagship and a prize,
/// the ones that would go through.
fn transfer_row(
    ctx: &Context,
    label: String,
    prize: &ShipView,
    load: impl Fn(u32) -> Load,
) -> Row<FleetAction> {
    let mut row = Row::text(Style::Body, label);
    let flagship = &ctx.flagship;
    for (verb, from, to) in [("Send", flagship, prize), ("Take", prize, flagship)] {
        for quantity in LOTS {
            if check_transfer(ctx.goods, load(quantity), from.aboard, to.aboard).is_ok() {
                row = row.with_button(
                    format!("{verb} {quantity}"),
                    FleetAction::Move(Transfer {
                        from: from.entity,
                        to: to.entity,
                        load: load(quantity),
                    }),
                );
            }
        }
    }
    row
}

fn goods_aboard(hold: &Hold) -> impl Iterator<Item = GoodId> + '_ {
    hold.owned.iter().map(|(good, _)| good)
}

fn build(ctx: &Context) -> Panel<FleetAction> {
    let flagship = &ctx.flagship;
    let mut rows = vec![
        Row::text(Style::Heading, format!("{} (flagship)", flagship.name)),
        Row::text(Style::Body, summary(ctx, flagship))
            .with_button("Look", FleetAction::Center(flagship.entity)),
    ];
    if ctx.prizes.is_empty() {
        rows.push(Row::text(
            Style::Faded,
            "No prizes. Board a ship that has struck her colours to take one.",
        ));
    }
    for prize in &ctx.prizes {
        let orders = prize.orders.unwrap_or_default();
        let order = |orders| {
            FleetAction::Order(OrderPrize {
                prize: prize.entity,
                orders,
            })
        };
        rows.push(Row::text(Style::Heading, format!("{} (prize)", prize.name)));
        rows.push(
            Row::text(Style::Body, summary(ctx, prize))
                .with_button("Look", FleetAction::Center(prize.entity)),
        );
        let mut orders_row = Row::text(Style::Body, orders_text(ctx, orders));
        if orders != PrizeOrders::Follow {
            orders_row = orders_row.with_button("Follow me", order(PrizeOrders::Follow));
        }
        if orders != PrizeOrders::Wait {
            orders_row = orders_row.with_button("Wait", order(PrizeOrders::Wait));
        }
        rows.push(orders_row);
        let mut sail_to = Row::text(Style::Body, "Send her to");
        for &(port, name) in &ctx.ports {
            if orders != PrizeOrders::SailTo(port) {
                sail_to = sail_to.with_button(name, order(PrizeOrders::SailTo(port)));
            }
        }
        rows.push(sail_to);

        if !prize.alongside {
            rows.push(Row::text(
                Style::Faded,
                "Bring her alongside, or into port with you, to move cargo or men.",
            ));
            continue;
        }
        rows.push(transfer_row(
            ctx,
            format!(
                "Hands: {} ours, {} hers",
                flagship.aboard.crew.count, prize.aboard.crew.count
            ),
            prize,
            Load::Crew,
        ));
        let mut cargo: Vec<GoodId> = goods_aboard(flagship.aboard.hold)
            .chain(goods_aboard(prize.aboard.hold))
            .collect();
        cargo.sort();
        cargo.dedup();
        for good in cargo {
            rows.push(transfer_row(
                ctx,
                format!(
                    "{}: {:.0} ours, {:.0} hers",
                    ctx.goods.get(good).name,
                    flagship.aboard.hold.owned.get(good).floor(),
                    prize.aboard.hold.owned.get(good).floor()
                ),
                prize,
                |quantity| Load::Cargo { good, quantity },
            ));
        }
    }
    rows.push(Row::text(Style::Body, "").with_button("Close", FleetAction::Close));
    Panel::new("Your fleet", ctx.hint.clone(), rows)
}

/// What the screen reads from each ship.
type ShipData = (
    Entity,
    &'static Name,
    &'static Transform,
    Option<&'static Docked>,
    (&'static Hold, &'static Crew, &'static Complement),
    (&'static Timbers, &'static Rigging),
    Option<(&'static Prize, &'static PrizeOrders)>,
);

/// Where she lies, in words, seen from the flagship at `flagship_at`.
fn place(
    ports: &[(Entity, &str)],
    flagship: bool,
    at: Vec3,
    docked: Option<&Docked>,
    flagship_at: Vec3,
) -> String {
    match docked {
        Some(docked) => format!(
            "Lying at {}",
            ports
                .iter()
                .find(|(port, _)| *port == docked.port)
                .map_or("port", |(_, name)| name)
        ),
        None if flagship => "At sea".to_string(),
        None => format!(
            "At sea, {:.1} km from you",
            at.distance(flagship_at) / 1000.0
        ),
    }
}

fn view<'a>(
    ship: QueryItem<'a, '_, ShipData>,
    ports: &[(Entity, &str)],
    flagship: Entity,
    (flagship_at, flagship_docked): (Vec3, Option<&Docked>),
) -> ShipView<'a> {
    let (entity, name, at, docked, (hold, crew, complement), (timbers, rigging), prize) = ship;
    ShipView {
        entity,
        name: name.as_str(),
        place: place(
            ports,
            entity == flagship,
            at.translation,
            docked,
            flagship_at,
        ),
        aboard: Aboard {
            hold,
            crew,
            complement,
        },
        timbers: timbers.0,
        rigging: rigging.0,
        orders: prize.map(|(_, orders)| *orders),
        alongside: together((at.translation, docked), (flagship_at, flagship_docked)),
    }
}

fn refresh(
    goods: Res<Goods>,
    player: Single<Entity, With<Player>>,
    ships: Query<ShipData>,
    ports: Query<(Entity, &Name), With<Port>>,
    bindings: Res<Bindings>,
    panel: Option<Single<(Entity, &mut Shown<FleetAction>)>>,
    mut commands: Commands,
) {
    let Some(panel) = panel else {
        return;
    };
    let Ok(flagship) = ships.get(*player) else {
        return;
    };
    let flagship_at = (flagship.2.translation, flagship.3.copied());
    let ports: Vec<(Entity, &str)> = ports.iter().map(|(e, n)| (e, n.as_str())).collect();
    let station = (flagship_at.0, flagship_at.1.as_ref());
    let prizes = ships
        .iter()
        .filter(|ship| ship.6.is_some_and(|(prize, _)| prize.captor == *player))
        .map(|ship| view(ship, &ports, *player, station))
        .collect();
    let ctx = Context {
        goods: &goods,
        hint: closing_hint(&bindings, Sheet::Fleet),
        flagship: view(flagship, &ports, *player, station),
        prizes,
        ports: ports.clone(),
    };
    let content = build(&ctx);
    let (entity, mut shown) = panel.into_inner();
    show(&mut commands, (entity, &mut shown), content, act);
}

fn act(
    activate: On<Activate>,
    actions: Query<&FleetAction>,
    mut open: ResMut<OpenSheet>,
    ships: Query<&Transform>,
    player: Single<Entity, With<Player>>,
    mut rig: Single<&mut CameraRig>,
    mut orders: MessageWriter<OrderPrize>,
    mut transfers: MessageWriter<Transfer>,
) {
    let Ok(&action) = actions.get(activate.entity) else {
        return;
    };
    match action {
        FleetAction::Order(order) => {
            orders.write(order);
        }
        FleetAction::Move(transfer) => {
            transfers.write(transfer);
        }
        FleetAction::Center(ship) => {
            rig.following = ship == *player;
            if let Ok(at) = ships.get(ship) {
                rig.focus = at.translation;
            }
        }
        FleetAction::Close => open.0 = None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship_class::ShipClass;

    struct Fixture {
        goods: Goods,
        hold: Hold,
        prize_hold: Hold,
        crew: Crew,
        prize_crew: Crew,
        complement: Complement,
    }

    fn fixture() -> Fixture {
        let goods = Goods::standard();
        let mut prize_hold = Hold::new(40.0);
        prize_hold.owned.add(goods.id("rum").unwrap(), 20.0);
        Fixture {
            goods,
            hold: Hold::new(40.0),
            prize_hold,
            crew: Crew::new(12),
            prize_crew: Crew::new(2),
            complement: ShipClass::sloop().complement,
        }
    }

    const FLAGSHIP: Entity = Entity::PLACEHOLDER;

    fn prize_entity() -> Entity {
        Entity::from_raw_u32(5).unwrap()
    }

    fn port() -> Entity {
        Entity::from_raw_u32(9).unwrap()
    }

    impl Fixture {
        fn panel(&self, with_prize: bool, alongside: bool) -> Panel<FleetAction> {
            let ship = |entity, name, hold, crew, orders| ShipView {
                entity,
                name,
                place: "At sea".into(),
                aboard: Aboard {
                    hold,
                    crew,
                    complement: &self.complement,
                },
                timbers: 1.0,
                rigging: 1.0,
                orders,
                alongside,
            };
            let ctx = Context {
                goods: &self.goods,
                hint: String::new(),
                flagship: ship(FLAGSHIP, "Ranger", &self.hold, &self.crew, None),
                prizes: if with_prize {
                    vec![ship(
                        prize_entity(),
                        "Swallow",
                        &self.prize_hold,
                        &self.prize_crew,
                        Some(PrizeOrders::Follow),
                    )]
                } else {
                    Vec::new()
                },
                ports: vec![(port(), "Nassau")],
            };
            build(&ctx)
        }
    }

    fn actions(panel: &Panel<FleetAction>) -> Vec<FleetAction> {
        panel
            .rows
            .iter()
            .flat_map(|row| row.buttons.iter().map(|(_, action)| *action))
            .collect()
    }

    #[test]
    fn without_prizes_the_screen_says_how_to_get_one() {
        let panel = fixture().panel(false, false);
        assert!(
            panel
                .rows
                .iter()
                .any(|row| row.cells[0].contains("No prizes"))
        );
    }

    #[test]
    fn a_prize_can_be_ordered_to_wait_or_sail_to_a_port() {
        let actions = actions(&fixture().panel(true, false));
        let order = |orders| {
            FleetAction::Order(OrderPrize {
                prize: prize_entity(),
                orders,
            })
        };
        assert!(actions.contains(&order(PrizeOrders::Wait)));
        assert!(actions.contains(&order(PrizeOrders::SailTo(port()))));
        assert!(
            !actions.contains(&order(PrizeOrders::Follow)),
            "already following"
        );
        assert!(
            !actions.iter().any(|a| matches!(a, FleetAction::Move(_))),
            "no transfers apart"
        );
    }

    #[test]
    fn alongside_her_cargo_and_men_can_be_moved() {
        let fixture = fixture();
        let actions = actions(&fixture.panel(true, true));
        let rum = fixture.goods.id("rum").unwrap();
        let take_rum = FleetAction::Move(Transfer {
            from: prize_entity(),
            to: FLAGSHIP,
            load: Load::Cargo {
                good: rum,
                quantity: 10,
            },
        });
        let send_hands = FleetAction::Move(Transfer {
            from: FLAGSHIP,
            to: prize_entity(),
            load: Load::Crew(10),
        });
        assert!(actions.contains(&take_rum));
        assert!(actions.contains(&send_hands));
        assert!(
            !actions.contains(&FleetAction::Move(Transfer {
                from: prize_entity(),
                to: FLAGSHIP,
                load: Load::Crew(10),
            })),
            "she has only two"
        );
    }
}
