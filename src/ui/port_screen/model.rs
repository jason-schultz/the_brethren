//! What the port screen shows, as plain data. Built fresh from the world
//! each frame and compared with what's on screen, so the screen is only
//! rebuilt when something visible changes.

use bevy::prelude::*;

use crate::allegiance::Favour;
use crate::clock::GameDate;
use crate::crew::shore_leave_cost;
use crate::economy::jobs::{Job, check_acceptance, job_office, shortfall};
use crate::economy::market::{Dealing, Market, Side, Trader, asking_price, bid_price, quote};
use crate::goods::{Carriage, GoodId, Goods};
use crate::officers::{Berth, Officer, OfficerAct, Quarterdeck, check_order};
use crate::port::{Location, Port};
use crate::ui::panel::{self, Style};
use crate::ui::{mood_line, ship_status_line};

/// Batch sizes offered for buying and selling.
const LOTS: [u32; 2] = [1, 10];
/// Batch sizes offered for hiring.
const HIRES: [u32; 2] = [1, 5];

/// Where in town the captain is.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum View {
    /// The town overview, with every location.
    #[default]
    Town,
    At(Location),
}

/// What a button does.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub(super) enum Action {
    Go(View),
    Trade {
        good: GoodId,
        quantity: u32,
        side: Side,
    },
    Accept(Entity),
    CastOff,
    Repair,
    SellPrize(Entity),
    TakeCommand(Entity),
    ShoreLeave,
    /// Hire this officer.
    Hire(Entity),
}

type Row = panel::Row<Action>;
type Panel = panel::Panel<Action>;

/// A job as the screen needs it.
pub(super) struct Listing<'a> {
    pub entity: Entity,
    pub job: &'a Job,
    pub destination: &'a str,
    /// Whether the player's ship is carrying it.
    pub carried: bool,
}

/// A prize lying in port with the player, as the shipyard sees her.
pub(super) struct PrizeListing<'a> {
    pub entity: Entity,
    pub name: &'a str,
    pub timbers: f32,
    pub rigging: f32,
    /// What she'd fetch here.
    pub value: u32,
}

/// An officer looking for a post at the tavern.
pub(super) struct Candidate<'a> {
    pub entity: Entity,
    pub name: &'a str,
    pub officer: &'a Officer,
    pub berth: Berth,
}

/// The player's ship's battle damage.
pub(super) struct Condition {
    pub timbers: f32,
    pub rigging: f32,
    /// What making her sound would cost here.
    pub repair_cost: u32,
}

/// Everything the panel is built from.
pub(super) struct Context<'a> {
    pub goods: &'a Goods,
    pub port_entity: Entity,
    pub port_name: &'a str,
    pub port: &'a Port,
    /// How the town regards each nation.
    pub favour: &'a Favour,
    pub market: &'a Market,
    pub trader: Trader<'a>,
    pub jobs: Vec<Listing<'a>>,
    pub condition: Condition,
    /// The player's standing with the port's nation.
    pub standing: f32,
    pub prizes: Vec<PrizeListing<'a>>,
    /// The player's quarterdeck, for hiring officers.
    pub deck: Quarterdeck<'a>,
    pub candidates: Vec<Candidate<'a>>,
}

pub(super) fn location_name(location: Location) -> &'static str {
    match location {
        Location::Tavern => "Tavern",
        Location::Market => "Market",
        Location::Shipyard => "Shipyard",
        Location::GovernorsHouse => "Governor's House",
        Location::MerchantsExchange => "Merchants' Exchange",
    }
}

fn location_purpose(location: Location) -> &'static str {
    match location {
        Location::Tavern => "news, rum, and men looking for a berth",
        Location::Market => "cargo bought and sold",
        Location::Shipyard => "repairs, refits, and new hulls",
        Location::GovernorsHouse => "the crown's business",
        Location::MerchantsExchange => "freight contracts",
    }
}

pub(super) fn build(view: View, ctx: &Context) -> Panel {
    let mut rows = vec![ship_status(ctx)];
    match view {
        View::Town => town(ctx, &mut rows),
        View::At(location) => {
            rows.push(Row::text(Style::Heading, location_name(location)));
            match location {
                Location::Market => market(ctx, &mut rows),
                Location::Tavern => tavern(ctx, &mut rows),
                Location::Shipyard => shipyard(ctx, &mut rows),
                Location::GovernorsHouse | Location::MerchantsExchange => {}
            }
            job_board(ctx, location, &mut rows);
            rows.push(
                Row::text(Style::Body, "").with_button("Back to town", Action::Go(View::Town)),
            );
        }
    }
    Panel::new(
        ctx.port_name.to_string(),
        format!(
            "Flag: {}. Your standing here: {:+.0}",
            ctx.port.flag, ctx.standing
        ),
        rows,
    )
}

fn ship_status(ctx: &Context) -> Row {
    Row::text(Style::Faded, ship_status_line(ctx.goods, &ctx.trader))
}

fn town(ctx: &Context, rows: &mut Vec<Row>) {
    for &location in &ctx.port.locations {
        rows.push(
            Row::text(
                Style::Body,
                format!(
                    "{} — {}",
                    location_name(location),
                    location_purpose(location)
                ),
            )
            .with_button("Visit", Action::Go(View::At(location))),
        );
    }

    let short: Vec<&str> = ctx
        .market
        .entries
        .iter()
        .filter(|entry| shortfall(entry).is_some())
        .map(|entry| ctx.goods.get(entry.good).name.as_str())
        .collect();
    if !short.is_empty() {
        rows.push(Row::text(
            Style::Faded,
            format!("Short of: {}", short.join(", ")),
        ));
    }
    rows.push(Row::text(
        Style::Faded,
        format!("The town is {}", mood_line(ctx.port.flag, ctx.favour)),
    ));

    let carried: Vec<&Listing> = ctx.jobs.iter().filter(|l| l.carried).collect();
    if !carried.is_empty() {
        rows.push(Row::text(Style::Heading, "Your contracts"));
        rows.extend(carried.into_iter().map(|listing| job_row(ctx, listing)));
    }
    rows.push(Row::text(Style::Body, "").with_button("Cast Off", Action::CastOff));
}

fn market(ctx: &Context, rows: &mut Vec<Row>) {
    rows.push(Row {
        cells: ["Good", "Stock", "Buy at", "Sell at", "Aboard"]
            .map(String::from)
            .to_vec(),
        buttons: Vec::new(),
        style: Style::Faded,
    });
    for entry in &ctx.market.entries {
        if Dealing::of(ctx.goods, entry.good) != Some(Dealing::Freight) {
            continue;
        }
        let good = ctx.goods.get(entry.good);
        let mut row = Row {
            cells: vec![
                good.name.clone(),
                format!("{:.0}", entry.stock.floor()),
                asking_price(entry, good.base_price, 1)
                    .map_or("—".to_string(), |price| price.to_string()),
                bid_price(entry, good.base_price, 1).to_string(),
                format!("{:.0}", ctx.trader.hold.owned.get(entry.good).floor()),
            ],
            buttons: Vec::new(),
            style: Style::Body,
        };
        for side in [Side::Buy, Side::Sell] {
            for quantity in LOTS {
                if quote(
                    ctx.goods,
                    ctx.port,
                    ctx.market,
                    &ctx.trader,
                    entry.good,
                    quantity,
                    side,
                )
                .is_ok()
                {
                    let verb = if side == Side::Buy { "Buy" } else { "Sell" };
                    row = row.with_button(
                        format!("{verb} {quantity}"),
                        Action::Trade {
                            good: entry.good,
                            quantity,
                            side,
                        },
                    );
                }
            }
        }
        rows.push(row);
    }
}

fn shipyard(ctx: &Context, rows: &mut Vec<Row>) {
    let condition = &ctx.condition;
    if condition.repair_cost == 0 {
        rows.push(Row::text(
            Style::Body,
            "Your ship is sound. There is nothing to repair.",
        ));
    } else {
        let row = Row::text(
            Style::Body,
            format!(
                "Timbers {:.0}%, rigging {:.0}%: repairs cost {} pieces of eight",
                condition.timbers * 100.0,
                condition.rigging * 100.0,
                condition.repair_cost
            ),
        );
        rows.push(if condition.repair_cost <= ctx.trader.purse.0 {
            row.with_button("Repair", Action::Repair)
        } else {
            row
        });
    }
    if ctx.prizes.is_empty() {
        return;
    }
    rows.push(Row::text(Style::Heading, "Your prizes"));
    for prize in &ctx.prizes {
        rows.push(
            Row::text(
                Style::Body,
                format!(
                    "{}: timbers {:.0}%, rigging {:.0}%, fetches {} pieces of eight",
                    prize.name,
                    prize.timbers * 100.0,
                    prize.rigging * 100.0,
                    prize.value
                ),
            )
            .with_button("Sell", Action::SellPrize(prize.entity))
            .with_button("Take command", Action::TakeCommand(prize.entity)),
        );
    }
}

fn tavern(ctx: &Context, rows: &mut Vec<Row>) {
    let sailors = ctx.goods.sailors;
    shore_leave(ctx, rows);
    officers_for_hire(ctx, rows);
    let Some(entry) = ctx.market.entry(sailors) else {
        rows.push(Row::text(
            Style::Body,
            "No one here is looking for a berth.",
        ));
        return;
    };
    let bounty = asking_price(entry, ctx.goods.get(sailors).base_price, 1)
        .map_or("—".to_string(), |price| price.to_string());
    let mut row = Row::text(
        Style::Body,
        format!(
            "{:.0} men looking for a berth, {bounty} pieces of eight bounty a head",
            entry.stock.floor()
        ),
    );
    for quantity in HIRES {
        if quote(
            ctx.goods,
            ctx.port,
            ctx.market,
            &ctx.trader,
            sailors,
            quantity,
            Side::Buy,
        )
        .is_ok()
        {
            row = row.with_button(
                format!("Hire {quantity}"),
                Action::Trade {
                    good: sailors,
                    quantity,
                    side: Side::Buy,
                },
            );
        }
    }
    rows.push(row);
}

fn shore_leave(ctx: &Context, rows: &mut Vec<Row>) {
    let crew = ctx.trader.crew;
    let row = Row::text(
        Style::Body,
        format!(
            "Your men are {} (morale {:.0}). A run ashore would cheer them",
            crew.spirits().name(),
            crew.morale
        ),
    );
    rows.push(match shore_leave_cost(ctx.port, crew, ctx.trader.purse) {
        Ok(cost) => row.with_button(format!("Stand them one: {cost}"), Action::ShoreLeave),
        Err(refusal) => {
            let mut row = row;
            row.cells.push(format!("({refusal})"));
            row
        }
    });
}

fn officers_for_hire(ctx: &Context, rows: &mut Vec<Row>) {
    if ctx.candidates.is_empty() {
        return;
    }
    rows.push(Row::text(Style::Heading, "Officers looking for a post"));
    for candidate in &ctx.candidates {
        let Berth::ForHire { bounty, .. } = candidate.berth else {
            continue;
        };
        let role = candidate.officer.role;
        let row = Row::text(
            Style::Body,
            format!(
                "{}, {}: {}. Asks {bounty} pieces of eight",
                candidate.name,
                role.name(),
                role.duty()
            ),
        );
        rows.push(
            match check_order(
                OfficerAct::Hire,
                candidate.officer,
                candidate.berth,
                &ctx.deck,
            ) {
                Ok(_) => row.with_button("Hire", Action::Hire(candidate.entity)),
                Err(refusal) => {
                    let mut row = row;
                    row.cells.push(format!("({refusal})"));
                    row
                }
            },
        );
    }
}

/// Jobs offered at `location`, with an Accept button on those the ship can take.
fn job_board(ctx: &Context, location: Location, rows: &mut Vec<Row>) {
    let offered: Vec<&Listing> = ctx
        .jobs
        .iter()
        .filter(|listing| {
            !listing.carried
                && listing.job.origin == ctx.port_entity
                && job_office(&ctx.port.locations, ctx.goods.carriage(listing.job.good))
                    == Some(location)
        })
        .collect();
    let is_office = matches!(
        location,
        Location::GovernorsHouse | Location::MerchantsExchange
    );
    if offered.is_empty() {
        if is_office {
            rows.push(Row::text(Style::Body, "No work on offer today."));
        }
        return;
    }
    rows.push(Row::text(Style::Heading, "Work on offer"));
    for listing in offered {
        let row = job_row(ctx, listing);
        let Trader {
            hold,
            crew,
            complement,
            ..
        } = ctx.trader;
        rows.push(
            match check_acceptance(
                ctx.goods,
                listing.job,
                ctx.port_entity,
                ctx.market,
                hold,
                crew,
                complement,
            ) {
                Ok(()) => row.with_button("Accept", Action::Accept(listing.entity)),
                Err(refusal) => {
                    let mut row = row;
                    row.cells.push(format!("({refusal})"));
                    row
                }
            },
        );
    }
}

fn job_row(ctx: &Context, listing: &Listing) -> Row {
    let job = listing.job;
    let good = ctx.goods.get(job.good);
    let unit = match ctx.goods.carriage(job.good) {
        Carriage::Hold => " t",
        Carriage::Berths => "",
    };
    Row {
        cells: vec![
            format!("{}{unit} {}", job.quantity, good.name),
            format!("to {}", listing.destination),
            format!("{} pieces of eight", job.payment),
            format!("by {}", GameDate::at(job.deadline)),
        ],
        buttons: Vec::new(),
        style: Style::Body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::{Hold, Purse};
    use crate::crew::{Complement, Crew};
    use crate::economy::market::MarketEntry;
    use crate::officers::{Loyalty, Role};

    struct Fixture {
        goods: Goods,
        port: Port,
        market: Market,
        hold: Hold,
        crew: Crew,
        complement: Complement,
        job: Job,
        /// The player's timbers, for the shipyard.
        timbers: f32,
        /// A prize lying here, if any.
        prize: Option<Entity>,
        gunner: Officer,
    }

    const PORT: Entity = Entity::PLACEHOLDER;
    const GUNNER: Entity = Entity::from_raw_u32(11).unwrap();

    fn fixture() -> Fixture {
        let goods = Goods::standard();
        let entry = |key: &str, stock, per_day| MarketEntry {
            good: goods.id(key).unwrap(),
            stock,
            target: 100.0,
            per_day,
        };
        let market = Market {
            entries: vec![
                entry("rum", 100.0, 1.0),
                entry("sailors", 20.0, 1.0),
                entry("soldiers", 100.0, 1.0),
                entry("timber", 10.0, -1.0),
            ],
        };
        let job = Job {
            good: goods.id("rum").unwrap(),
            quantity: 20,
            origin: PORT,
            destination: Entity::from_raw_u32(7).unwrap(),
            payment: 150,
            deadline: crate::clock::DAY,
        };
        Fixture {
            port: Port {
                flag: crate::factions::Nation::England,
                town: Vec3::ZERO,
                locations: vec![
                    Location::Tavern,
                    Location::Market,
                    Location::MerchantsExchange,
                    Location::Shipyard,
                ],
            },
            goods,
            market,
            hold: Hold::new(40.0),
            crew: Crew::new(12),
            complement: Complement {
                min: 8,
                full: 40,
                berths: 75,
            },
            job,
            timbers: 1.0,
            prize: None,
            gunner: Officer {
                role: Role::Gunner,
                loyalty: Loyalty::Loyal,
                confined: false,
            },
        }
    }

    impl Fixture {
        fn panel(&self, view: View, purse: u32) -> Panel {
            let ctx = Context {
                goods: &self.goods,
                port_entity: PORT,
                port_name: "Kingston",
                port: &self.port,
                favour: &Favour::held_by(self.port.flag),
                market: &self.market,
                trader: Trader {
                    hold: &self.hold,
                    purse: Purse(purse),
                    crew: &self.crew,
                    complement: &self.complement,
                },
                jobs: vec![Listing {
                    entity: Entity::from_raw_u32(9).unwrap(),
                    job: &self.job,
                    destination: "Nassau",
                    carried: false,
                }],
                standing: 10.0,
                condition: Condition {
                    timbers: self.timbers,
                    rigging: 1.0,
                    repair_cost: ((1.0 - self.timbers) * 1000.0) as u32,
                },
                prizes: self
                    .prize
                    .iter()
                    .map(|&entity| PrizeListing {
                        entity,
                        name: "Swallow",
                        timbers: 0.5,
                        rigging: 0.8,
                        value: 900,
                    })
                    .collect(),
                deck: Quarterdeck {
                    ship: Entity::from_raw_u32(3).unwrap(),
                    docked: Some(PORT),
                    purse: Purse(purse),
                    posts: &[],
                },
                candidates: vec![Candidate {
                    entity: GUNNER,
                    name: "John Hale",
                    officer: &self.gunner,
                    berth: Berth::ForHire {
                        port: PORT,
                        bounty: 150,
                        until: 0.0,
                    },
                }],
            };
            build(view, &ctx)
        }
    }

    fn actions(panel: &Panel) -> Vec<Action> {
        panel
            .rows
            .iter()
            .flat_map(|row| row.buttons.iter().map(|(_, action)| *action))
            .collect()
    }

    fn text(panel: &Panel) -> String {
        panel
            .rows
            .iter()
            .flat_map(|row| row.cells.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn town_lists_its_locations_and_what_it_is_short_of() {
        let f = fixture();
        let panel = f.panel(View::Town, 500);
        let actions = actions(&panel);
        for location in &f.port.locations {
            assert!(actions.contains(&Action::Go(View::At(*location))));
        }
        assert!(!actions.contains(&Action::Go(View::At(Location::GovernorsHouse))));
        assert!(actions.contains(&Action::CastOff));
        assert!(text(&panel).contains("Short of: Timber"));
    }

    #[test]
    fn market_offers_only_freight_and_only_trades_the_ship_can_make() {
        let f = fixture();
        let rum = f.goods.id("rum").unwrap();
        let rich = f.panel(View::At(Location::Market), 100_000);
        let listed = text(&rich);
        assert!(listed.contains("Rum") && listed.contains("Timber"));
        assert!(!listed.contains("Sailors") && !listed.contains("Soldiers"));
        let buy = |quantity| Action::Trade {
            good: rum,
            quantity,
            side: Side::Buy,
        };
        assert!(actions(&rich).contains(&buy(10)));
        assert!(
            !actions(&rich).iter().any(|a| matches!(
                a,
                Action::Trade {
                    side: Side::Sell,
                    ..
                }
            )),
            "nothing aboard to sell"
        );

        let poor = f.panel(View::At(Location::Market), 0);
        assert!(!actions(&poor).contains(&buy(1)), "can't afford it");
    }

    #[test]
    fn tavern_hires_sailors_and_the_exchange_offers_freight_work() {
        let f = fixture();
        let tavern = f.panel(View::At(Location::Tavern), 500);
        assert!(actions(&tavern).contains(&Action::Trade {
            good: f.goods.sailors,
            quantity: 5,
            side: Side::Buy,
        }));
        assert!(
            !actions(&tavern)
                .iter()
                .any(|a| matches!(a, Action::Accept(_))),
            "freight work is at the exchange here"
        );

        let exchange = f.panel(View::At(Location::MerchantsExchange), 500);
        assert!(actions(&exchange).contains(&Action::Accept(Entity::from_raw_u32(9).unwrap())));
        assert!(text(&exchange).contains("to Nassau"));
        assert!(actions(&exchange).contains(&Action::Go(View::Town)));
    }

    #[test]
    fn the_tavern_offers_a_run_ashore_and_officers_the_captain_can_afford() {
        let f = fixture();
        let flush = actions(&f.panel(View::At(Location::Tavern), 500));
        assert!(flush.contains(&Action::ShoreLeave));
        assert!(flush.contains(&Action::Hire(GUNNER)));
        let broke = f.panel(View::At(Location::Tavern), 20);
        assert!(!actions(&broke).contains(&Action::Hire(GUNNER)));
        assert!(!actions(&broke).contains(&Action::ShoreLeave));
        assert!(text(&broke).contains("can't afford"));
    }

    #[test]
    fn work_the_ship_has_no_room_for_shows_why_instead_of_accept() {
        let mut f = fixture();
        f.hold = Hold::new(10.0);
        let exchange = f.panel(View::At(Location::MerchantsExchange), 500);
        assert!(
            !actions(&exchange)
                .iter()
                .any(|a| matches!(a, Action::Accept(_)))
        );
        assert!(text(&exchange).contains("no room aboard"));
    }

    #[test]
    fn a_sound_ship_has_nothing_to_repair() {
        let panel = fixture().panel(View::At(Location::Shipyard), 500);
        assert!(text(&panel).contains("nothing to repair"));
        assert!(!actions(&panel).contains(&Action::Repair));
    }

    #[test]
    fn repairs_are_offered_when_affordable() {
        let damaged = Fixture {
            timbers: 0.6,
            ..fixture()
        };
        let view = View::At(Location::Shipyard);
        assert!(actions(&damaged.panel(view, 500)).contains(&Action::Repair));
        assert!(
            !actions(&damaged.panel(view, 100)).contains(&Action::Repair),
            "costs 400"
        );
    }

    #[test]
    fn prizes_in_port_can_be_sold_or_commanded() {
        let prize = Entity::from_raw_u32(11).unwrap();
        let fixture = Fixture {
            prize: Some(prize),
            ..fixture()
        };
        let panel = fixture.panel(View::At(Location::Shipyard), 0);
        assert!(text(&panel).contains("Swallow"));
        let actions = actions(&panel);
        assert!(actions.contains(&Action::SellPrize(prize)));
        assert!(actions.contains(&Action::TakeCommand(prize)));
    }
}
