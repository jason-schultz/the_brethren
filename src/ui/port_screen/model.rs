//! What the port screen shows, as plain data. Built fresh from the world
//! each frame and compared with what's on screen, so the screen is only
//! rebuilt when something visible changes.

use bevy::prelude::*;

use crate::allegiance::Favour;
use crate::clock::GameDate;
use crate::crew::shore_leave_cost;
use crate::economy::Refusal;
use crate::economy::jobs::{Job, check_acceptance, shortfall};
use crate::economy::market::{
    Dealing, Market, MarketEntry, Side, Trader, asking_price, bid_price, quote,
};
use crate::escorts::{Escort, check_escort};
use crate::factions::{Group, GroupKind, Nation, Standing, nations};
use crate::goods::{GoodId, Goods};
use crate::math::MPS_TO_KNOTS;
use crate::officers::{Berth, Officer, OfficerAct, Quarterdeck, check_order};
use crate::pardons::{NoPetition, check_petition};
use crate::port::{Location, Port, office};
use crate::service::{Service, check_join};
use crate::ship_class::ShipClass;
use crate::ui::panel::{self, Style};
use crate::ui::{amount, location_name, mood_line, ship_status_line, unit};
use crate::word::Fate;

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
    /// Take on this escort.
    AcceptEscort(Entity),
    CastOff,
    Repair,
    SellPrize(Entity),
    TakeCommand(Entity),
    /// Exchange her ship for a new one of this class.
    BuyShip(ShipClass),
    ShoreLeave,
    /// Hire this officer.
    Hire(Entity),
    /// Join this group.
    Join(Group),
    /// Pay a lawyer to petition this nation for a pardon.
    Petition(Nation),
    /// Say this is how the ship `lost` was lost.
    GiveAccount {
        lost: Entity,
        says: Fate,
    },
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

/// A merchant's request for an escort, as the screen needs it: one asked
/// for here, or one the player has taken on.
pub(super) struct EscortListing<'a> {
    pub entity: Entity,
    pub escort: &'a Escort,
    pub ward: &'a str,
    pub destination: &'a str,
    /// What she carries under contract, if anything.
    pub cargo: Option<(GoodId, u32)>,
    /// Whether the player has taken it on.
    pub taken: bool,
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

/// A new ship the yard offers: what she'd cost with the player's ship in
/// part exchange, or why it won't sell her.
pub(super) struct Offer {
    pub class: ShipClass,
    pub cost: Result<i64, Refusal>,
}

/// An officer looking for a post at the tavern.
pub(super) struct Candidate<'a> {
    pub entity: Entity,
    pub name: &'a str,
    pub officer: &'a Officer,
    pub berth: Berth,
}

/// A loss the player was about for and can give an account of: what's
/// asked, and what she could say, with a label for each.
pub(super) struct AccountListing {
    pub lost: Entity,
    pub asked: String,
    pub choices: Vec<(Fate, String)>,
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
    pub escorts: Vec<EscortListing<'a>>,
    /// What the tavern talks of, as it was told to the player, a sentence
    /// each.
    pub word: Vec<String>,
    pub accounts: Vec<AccountListing>,
    pub condition: Condition,
    pub standing: &'a Standing,
    /// The groups the player serves.
    pub service: &'a Service,
    /// The colours she flies.
    pub colours: Option<Nation>,
    pub prizes: Vec<PrizeListing<'a>>,
    /// The new ships the shipyard sells.
    pub offers: Vec<Offer>,
    /// The player's quarterdeck, for hiring officers.
    pub deck: Quarterdeck<'a>,
    pub candidates: Vec<Candidate<'a>>,
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
            service_desk(ctx, location, &mut rows);
            if location == Location::Tavern {
                petitions(ctx, &mut rows);
            }
            job_board(ctx, location, &mut rows);
            escorts_wanted(ctx, location, &mut rows);
            rows.push(
                Row::text(Style::Body, "").with_button("Back to town", Action::Go(View::Town)),
            );
        }
    }
    Panel::new(
        ctx.port_name.to_string(),
        format!(
            "Flag: {}. Your standing here: {:+.0}",
            ctx.port.flag,
            ctx.standing.with(ctx.port.flag)
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

    let needs: Vec<String> = ctx
        .market
        .entries
        .iter()
        .filter(|entry| entry.is_needed())
        .map(|entry| {
            let name = &ctx.goods.get(entry.good).name;
            match shortfall(entry) {
                Some(_) => format!("{name} (SHORT)"),
                None => name.clone(),
            }
        })
        .collect();
    if !needs.is_empty() {
        rows.push(Row::text(
            Style::Faded,
            format!("The town needs: {}", needs.join(", ")),
        ));
    }
    rows.push(Row::text(
        Style::Faded,
        format!("The town is {}", mood_line(ctx.port.flag, ctx.favour)),
    ));

    let carried: Vec<&Listing> = ctx.jobs.iter().filter(|l| l.carried).collect();
    let escorting: Vec<&EscortListing> = ctx.escorts.iter().filter(|e| e.taken).collect();
    if !carried.is_empty() || !escorting.is_empty() {
        rows.push(Row::text(Style::Heading, "Your contracts"));
        rows.extend(carried.into_iter().map(|listing| job_row(ctx, listing)));
        rows.extend(
            escorting
                .into_iter()
                .map(|listing| escort_row(ctx, listing)),
        );
    }
    rows.push(Row::text(Style::Body, "").with_button("Cast Off", Action::CastOff));
}

fn market(ctx: &Context, rows: &mut Vec<Row>) {
    rows.push(Row {
        cells: ["Good", "Stock", "Wanted", "Buy at", "Sell at", "Aboard"]
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
                wanted(entry),
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

/// How much more of a good the town wants, if it needs it at all: the gap
/// below its target, marked when it's short enough to post work.
fn wanted(entry: &MarketEntry) -> String {
    let gap = entry.target - entry.stock;
    match shortfall(entry) {
        Some(_) => format!("{gap:.0} SHORT"),
        None if entry.is_needed() && gap >= 1.0 => format!("{gap:.0}"),
        None => "—".to_string(),
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
    offers(ctx, rows);
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

/// The new ships for sale, each with her particulars and what she'd cost
/// with the player's ship in part exchange.
fn offers(ctx: &Context, rows: &mut Vec<Row>) {
    if ctx.offers.is_empty() {
        return;
    }
    rows.push(Row::text(
        Style::Heading,
        "New ships, yours in part exchange",
    ));
    for offer in &ctx.offers {
        let class = &offer.class;
        let particulars = format!(
            "{}: {} guns, {:.0} tons, {} hands to sail and {} to fight, {} berths, {:.1} knots",
            class.name,
            class.guns,
            class.hold_tons,
            class.complement.min,
            class.complement.full,
            class.complement.berths,
            class.hull.speed_mps * MPS_TO_KNOTS,
        );
        rows.push(match offer.cost {
            Ok(cost) => {
                let terms = if cost >= 0 {
                    format!("costs {cost} pieces of eight")
                } else {
                    format!("the yard pays you {}", -cost)
                };
                Row::text(Style::Body, format!("{particulars}: {terms}"))
                    .with_button("Buy", Action::BuyShip(*class))
            }
            Err(refusal) => Row::text(Style::Body, format!("{particulars}: {refusal}")),
        });
    }
}

fn tavern(ctx: &Context, rows: &mut Vec<Row>) {
    let sailors = ctx.goods.sailors;
    shore_leave(ctx, rows);
    officers_for_hire(ctx, rows);
    accounts(ctx, rows);
    hearsay(ctx, rows);
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

/// The losses the player was about for, and what she could say of each.
/// Whatever she says is taken at her word, until the truth comes out.
fn accounts(ctx: &Context, rows: &mut Vec<Row>) {
    if ctx.accounts.is_empty() {
        return;
    }
    rows.push(Row::text(Style::Heading, "They'll want your account"));
    for listing in &ctx.accounts {
        let mut row = Row::text(Style::Body, listing.asked.clone());
        for (says, label) in &listing.choices {
            row = row.with_button(
                label.clone(),
                Action::GiveAccount {
                    lost: listing.lost,
                    says: *says,
                },
            );
        }
        rows.push(row);
    }
}

/// What the tavern talks of: ships sailing, deeds done, wrecks. It may
/// not all be true.
fn hearsay(ctx: &Context, rows: &mut Vec<Row>) {
    rows.push(Row::text(Style::Heading, "Talk in the tavern"));
    if ctx.word.is_empty() {
        rows.push(Row::text(Style::Faded, "No news worth the telling."));
    }
    rows.extend(
        ctx.word
            .iter()
            .map(|line| Row::text(Style::Body, line.clone())),
    );
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

/// The groups of the port's nation that keep their office at `location`,
/// with the player's standing and post with each, and a Join button where
/// she can.
fn service_desk(ctx: &Context, location: Location, rows: &mut Vec<Row>) {
    let here = ctx
        .port
        .flag
        .groups()
        .into_iter()
        .filter(|group| office(&ctx.port.locations, group.kind) == Some(location));
    for group in here {
        let mut row = Row::text(
            Style::Body,
            format!("{group}: you stand {:+.0}", ctx.standing.of(group)),
        );
        if let Some(post) = ctx.service.post(group) {
            row.cells
                .push(format!("You serve as {}: {}", post.title(), post.perk()));
        } else {
            row = match check_join(group, ctx.port, ctx.standing, ctx.service, ctx.colours) {
                Ok(()) => row.with_button("Join", Action::Join(group)),
                Err(refusal) => {
                    row.cells.push(format!("({refusal})"));
                    row
                }
            };
        }
        rows.push(row);
    }
}

/// A lawyer's offer to petition each crown that wants the player.
fn petitions(ctx: &Context, rows: &mut Vec<Row>) {
    for nation in nations() {
        let row = |fee: u32| {
            Row::text(
                Style::Body,
                format!(
                    "A lawyer will carry your petition for a pardon to {nation}, for {fee} pieces of eight"
                ),
            )
        };
        match check_petition(ctx.standing, nation, ctx.trader.purse, ctx.port) {
            Ok(fee) => rows.push(row(fee).with_button("Petition", Action::Petition(nation))),
            Err(NoPetition::TooDear(fee)) => {
                let mut row = row(fee);
                row.cells.push("(you can't pay him)".to_string());
                rows.push(row);
            }
            Err(NoPetition::NotWanted | NoPetition::NoLawyer) => {}
        }
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
                && listing.job.office(ctx.goods, ctx.port) == Some(location)
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

/// Merchants sailing from here who want an escort, at the merchants'
/// office, with an Accept button on those the player can take on.
fn escorts_wanted(ctx: &Context, location: Location, rows: &mut Vec<Row>) {
    if office(&ctx.port.locations, GroupKind::Merchants) != Some(location) {
        return;
    }
    let wanted: Vec<&EscortListing> = ctx
        .escorts
        .iter()
        .filter(|listing| !listing.taken && listing.escort.origin == ctx.port_entity)
        .collect();
    if wanted.is_empty() {
        return;
    }
    rows.push(Row::text(Style::Heading, "Escorts wanted"));
    rows.push(Row::text(
        Style::Faded,
        "She sails when you do, and pays if she comes in with you in sight.",
    ));
    for listing in wanted {
        let row = escort_row(ctx, listing);
        rows.push(match check_escort(listing.escort, ctx.port_entity) {
            Ok(()) => row.with_button("Accept", Action::AcceptEscort(listing.entity)),
            Err(refusal) => {
                let mut row = row;
                row.cells.push(format!("({refusal})"));
                row
            }
        });
    }
}

fn escort_row(ctx: &Context, listing: &EscortListing) -> Row {
    let cargo = listing.cargo.map_or(String::new(), |(good, quantity)| {
        format!(", with {}", amount(ctx.goods, good, quantity))
    });
    let verb = if listing.taken { "Escorting" } else { "Escort" };
    Row::text(
        Style::Body,
        format!(
            "{verb} the {} to {}{cargo}: {} pieces of eight",
            listing.ward, listing.destination, listing.escort.fee
        ),
    )
}

fn job_row(ctx: &Context, listing: &Listing) -> Row {
    let job = listing.job;
    let good = ctx.goods.get(job.good);
    let unit = unit(ctx.goods, job.good);
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
    use crate::shipyard::{PartExchange, check_purchase};

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
        standing: Standing,
        service: Service,
        /// A merchant's request for an escort from here, and whether the
        /// player has taken it.
        escort: Option<(Escort, bool)>,
        /// What the tavern talks of.
        word: Vec<String>,
        /// Whether the player was about when a ship was lost.
        lost_nearby: bool,
    }

    const PORT: Entity = Entity::PLACEHOLDER;
    const ESCORT: Entity = Entity::from_raw_u32(13).unwrap();
    const GUNNER: Entity = Entity::from_raw_u32(11).unwrap();
    const DOVE: Entity = Entity::from_raw_u32(15).unwrap();

    fn fixture() -> Fixture {
        let goods = Goods::standard();
        let entry = |key: &str, stock, per_day| MarketEntry {
            good: goods.id(key).unwrap(),
            stock,
            target: 100.0,
            per_day,
            short_for: 0.0,
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
            introduction: None,
        };
        Fixture {
            port: Port {
                flag: crate::factions::Nation::ENGLAND,
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
            standing: Standing::default(),
            service: Service::default(),
            escort: None,
            word: Vec::new(),
            lost_nearby: false,
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
                escorts: self
                    .escort
                    .iter()
                    .map(|(escort, taken)| EscortListing {
                        entity: ESCORT,
                        escort,
                        ward: "Swallow",
                        destination: "Santiago",
                        cargo: Some((self.job.good, 20)),
                        taken: *taken,
                    })
                    .collect(),
                word: self.word.clone(),
                accounts: self
                    .lost_nearby
                    .then(|| AccountListing {
                        lost: DOVE,
                        asked: "Dove was lost 2 days ago. How was she lost?".to_string(),
                        choices: vec![
                            (Fate::Wrecked, "Wrecked".to_string()),
                            (Fate::Taken(ESCORT), "Taken by Ranger".to_string()),
                        ],
                    })
                    .into_iter()
                    .collect(),
                standing: &self.standing,
                service: &self.service,
                colours: Some(self.port.flag),
                condition: Condition {
                    timbers: self.timbers,
                    rigging: 1.0,
                    repair_cost: ((1.0 - self.timbers) * 1000.0) as u32,
                },
                offers: ShipClass::for_sale()
                    .filter(|class| *class != ShipClass::sloop())
                    .map(|class| Offer {
                        class,
                        cost: check_purchase(
                            &self.goods,
                            &self.port,
                            &class,
                            PartExchange {
                                class: &ShipClass::sloop(),
                                timbers: self.timbers,
                                rigging: 1.0,
                                hold: &self.hold,
                                crew: &self.crew,
                            },
                            Purse(purse),
                        ),
                    })
                    .collect(),
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
        assert!(text(&panel).contains("The town needs: Timber (SHORT)"));
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
    fn a_group_takes_proven_captains_at_its_office() {
        let mut f = fixture();
        let merchants = Group {
            nation: f.port.flag,
            kind: crate::factions::GroupKind::Merchants,
        };
        let exchange = || View::At(Location::MerchantsExchange);
        let unproven = f.panel(exchange(), 500);
        assert!(text(&unproven).contains("the Jamaica merchants: you stand +10"));
        assert!(!actions(&unproven).contains(&Action::Join(merchants)));
        f.standing.set_group(merchants, 25.0);
        assert!(actions(&f.panel(exchange(), 500)).contains(&Action::Join(merchants)));
        f.service = Service(vec![crate::service::Post {
            group: merchants,
            rank: 0,
        }]);
        assert!(text(&f.panel(exchange(), 500)).contains("You serve as Associate"));
        let navy_here = text(&f.panel(View::At(Location::Tavern), 500));
        assert!(
            navy_here.contains("the Royal Navy"),
            "no governor here, so the navy meets in the tavern"
        );
    }

    #[test]
    fn the_exchange_finds_merchants_an_escort_and_the_town_lists_it_once_taken() {
        let mut f = fixture();
        let escort = Escort {
            ward: Entity::from_raw_u32(14).unwrap(),
            nation: Nation::ENGLAND,
            origin: PORT,
            destination: Entity::from_raw_u32(7).unwrap(),
            fee: 60,
            escort: None,
            lost_at: None,
        };
        f.escort = Some((escort, false));
        let exchange = f.panel(View::At(Location::MerchantsExchange), 500);
        assert!(actions(&exchange).contains(&Action::AcceptEscort(ESCORT)));
        assert!(
            text(&exchange).contains("Escort the Swallow to Santiago, with 20 t rum: 60 pieces")
        );
        assert!(!text(&f.panel(View::At(Location::Tavern), 500)).contains("Swallow"));

        f.escort = Some((escort, true));
        assert!(
            !actions(&f.panel(View::At(Location::MerchantsExchange), 500))
                .contains(&Action::AcceptEscort(ESCORT))
        );
        assert!(text(&f.panel(View::Town, 500)).contains("Escorting the Swallow to Santiago"));
    }

    #[test]
    fn the_tavern_asks_for_an_account_of_a_loss_she_was_about_for() {
        let mut f = fixture();
        assert!(!text(&f.panel(View::At(Location::Tavern), 500)).contains("account"));
        f.lost_nearby = true;
        let tavern = f.panel(View::At(Location::Tavern), 500);
        assert!(text(&tavern).contains("Dove was lost"));
        let told: Vec<Action> = actions(&tavern)
            .into_iter()
            .filter(|action| matches!(action, Action::GiveAccount { .. }))
            .collect();
        assert_eq!(
            told,
            [Fate::Wrecked, Fate::Taken(ESCORT)]
                .map(|says| Action::GiveAccount { lost: DOVE, says })
        );
    }

    #[test]
    fn the_tavern_talks_of_the_news() {
        let mut f = fixture();
        let quiet = text(&f.panel(View::At(Location::Tavern), 500));
        assert!(quiet.contains("No news worth the telling."));
        f.word = vec!["Ranger took Dove off Kingston 2 days ago.".to_string()];
        let talk = text(&f.panel(View::At(Location::Tavern), 500));
        assert!(talk.contains("Ranger took Dove off Kingston"), "{talk}");
    }

    #[test]
    fn a_lawyer_in_the_tavern_petitions_a_crown_that_wants_her() {
        let mut f = fixture();
        f.port.flag = Nation::PIRATES;
        let petition = Action::Petition(Nation::ENGLAND);
        assert!(!actions(&f.panel(View::At(Location::Tavern), 10_000)).contains(&petition));
        f.standing.set(Nation::ENGLAND, -40.0);
        assert!(actions(&f.panel(View::At(Location::Tavern), 10_000)).contains(&petition));
        let poor = f.panel(View::At(Location::Tavern), 10);
        assert!(!actions(&poor).contains(&petition));
        assert!(text(&poor).contains("(you can't pay him)"));
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
    fn the_shipyard_sells_the_ships_she_can_pay_for() {
        let buys = |purse| {
            let panel = fixture().panel(View::At(Location::Shipyard), purse);
            actions(&panel)
                .into_iter()
                .filter_map(|action| match action {
                    Action::BuyShip(class) => Some(class.name),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let panel = fixture().panel(View::At(Location::Shipyard), 0);
        assert!(text(&panel).contains("Frigate"), "every class is shown");
        assert!(text(&panel).contains("can't afford it"));
        assert!(!buys(0).contains(&"Frigate"));
        assert!(!buys(0).contains(&"Sloop"), "not the class she sails");
        assert!(buys(100_000).contains(&"Frigate"));
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
