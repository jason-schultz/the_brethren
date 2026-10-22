//! Merchant captains: NPC ships that carry the ports' contracts.
//!
//! A merchant lies in port a while, takes the best-paying job on offer,
//! tops up her provisions, and sails to deliver it. With no work here, she
//! sails to the port with the most work waiting. A contract already aboard
//! comes first. She gives exactly the
//! orders the player gives ([`AcceptJob`], [`Trade`], [`CastOff`],
//! [`SailOrder`]), so she docks, delivers, and is paid by the same rules.
//! With the player in port and raiders about, she asks for an [`Escort`]
//! before she sails, and word of where she's bound gets about ([`Sailed`]).
//! With no work, a wreck talked of in port sends her out diving; she
//! sells what she brings up at the next port she makes. Her
//! [`crate::seamanship`] keeps her fit and out of the way of warships.

use bevy::ecs::query::QueryData;
use bevy::ecs::system::SystemParam;
use bevy::math::FloatOrd;
use bevy::prelude::*;

use crate::cargo::{Hold, Purse};
use crate::clock::{DAY, HOUR, WorldClock};
use crate::crew::{Complement, Crew, RATION_TONS};
use crate::economy::jobs::{AcceptJob, Carrier, Job, check_acceptance};
use crate::economy::market::{Market, Side, Trade, sell_cargo};
use crate::escorts::{ESCORT_WAIT, Escort, escort_fee, waits_for_escort, wants_escort};
use crate::factions::{Colours, Nation, Relations};
use crate::goods::Carriage;
use crate::goods::Goods;
use crate::navigation::{Arrival, Route, SailOrder};
use crate::player::Player;
use crate::port::{Docked, Port, PutToSea};
use crate::roster::{Listed, Roster};
use crate::schedule::SimSet;
use crate::seamanship::{AtHerBusiness, Running, Upkeep};
use crate::ship_class::ShipClass;
use crate::word::Purpose;
use crate::wrecks::{SALVAGE_RANGE, WreckTalk};

/// How long a merchant lies in port before looking for work.
pub const IN_PORT: f64 = 6.0 * HOUR;
/// With no work anywhere, she looks again this much later.
const NO_WORK_RETRY: f64 = HOUR;
/// How long loading takes once she has chosen her voyage.
pub const LOADING: f64 = 0.5 * HOUR;
/// The opening fleet sails at this interval from the start of the game, so
/// there are ships at sea from the first hour.
const OPENING_STAGGER: f64 = 10.0 * 60.0;
/// A merchant sails with provisions for this many days.
const PROVISION_DAYS: f64 = 20.0;
/// Merchants sail lean, with this many hands beyond the least that can
/// work her.
const SPARE_HANDS: u32 = 2;
/// Each merchant's opening purse.
const PURSE: u32 = 300;
/// A lost merchant is replaced this long after she's gone.
pub const REPLACEMENT: f64 = 2.0 * DAY;
/// Merchants sharing an anchorage lie this far apart, in meters.
const BERTH_SPACING: f32 = 30.0;

/// An NPC merchant captain, and what she means to do next.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(Upkeep = Upkeep::Purse)]
pub struct Merchant {
    pub plan: Plan,
}

/// Every plan is safe to save and resume: a merchant found in port with
/// any plan simply settles in and looks for work again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Plan {
    /// Under way, or just arrived or departed and not yet settled in port.
    AtSea,
    /// Lying in port until this [`WorldClock`] time.
    InPort { until: f64 },
    /// Loading for a voyage to `bound_for`, sailing at `sails_at`.
    Loading { bound_for: Entity, sails_at: f64 },
    /// Out to dive on a wreck, until she's full or the wreck is bare.
    Salvaging { wreck: Entity },
}

/// The merchant fleet's roster.
#[derive(Resource, Debug)]
struct Shipwrights(Roster);

impl Default for Shipwrights {
    fn default() -> Self {
        Self(Roster::of_fleets(
            |nation| &nation.fleet().merchants,
            REPLACEMENT,
        ))
    }
}

pub struct MerchantsPlugin;

impl Plugin for MerchantsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shipwrights>().add_systems(
            FixedUpdate,
            (merchant_at_sea, merchant_in_port, replace_lost_merchants).in_set(SimSet::Economy),
        );
    }
}

/// A new merchant from the roster, lying at `port` until `until`. She sails
/// lean, with half the guns she's pierced for: more room for cargo.
fn merchant(goods: &Goods, listed: Listed, port: Entity, at: Vec3, until: f64) -> impl Bundle {
    let class = listed.class;
    let crew = class.complement.min + SPARE_HANDS;
    let mut hold = class.empty_hold();
    hold.owned
        .add(goods.provisions, provisions_needed(crew as f64, 0.0));
    (
        Name::new(listed.name),
        Merchant {
            plan: Plan::InPort { until },
        },
        class.build(crew, class.guns / 2, hold),
        Purse(PURSE),
        Colours(listed.nation),
        Transform::from_translation(at),
        Docked { port },
    )
}

/// A port a merchant can sail from: the port, its flag, and its anchorage.
pub type Harbour = (Entity, Nation, Vec3);

/// The `index`th of `nation`'s merchants' home port, and how many of her
/// fleet lie there before her: one of her nation's ports, or any port if
/// it has none.
fn home(ports: &[Harbour], nation: Nation, index: usize) -> Option<(Entity, Vec3, usize)> {
    let own: Vec<&Harbour> = ports
        .iter()
        .filter(|(_, flag, _)| *flag == nation)
        .collect();
    let choices = if own.is_empty() {
        ports.iter().collect()
    } else {
        own
    };
    let &(port, _, anchorage) = *choices.get(index % choices.len().max(1))?;
    Some((port, anchorage, index / choices.len()))
}

/// The merchant fleet, spread round-robin across her nations' `ports`,
/// looking for work at [`OPENING_STAGGER`] intervals.
pub fn merchant_fleet(goods: &Goods, ports: &[Harbour]) -> Vec<impl Bundle> {
    let fleet = Shipwrights::default().0.ships;
    fleet
        .iter()
        .enumerate()
        .filter_map(|(i, &listed)| {
            // Her place among her own nation's merchants.
            let index = fleet[..i]
                .iter()
                .filter(|other| other.nation == listed.nation)
                .count();
            let (port, anchorage, before) = home(ports, listed.nation, index)?;
            let berth = (before + 1) as f32 * BERTH_SPACING;
            let until = OPENING_STAGGER * (i + 1) as f64;
            Some(merchant(
                goods,
                listed,
                port,
                anchorage + Vec3::X * berth,
                until,
            ))
        })
        .collect()
}

/// The ports, in a stable order.
pub fn harbours(ports: &Query<(Entity, &Port, &Transform)>) -> Vec<Harbour> {
    let mut harbours: Vec<Harbour> = ports
        .iter()
        .map(|(entity, port, anchorage)| (entity, port.flag, anchorage.translation))
        .collect();
    // Query order isn't stable; sort so ships launch the same way every game.
    harbours.sort_by_key(|&(port, ..)| port);
    harbours
}

/// A merchant sunk or taken is replaced [`REPLACEMENT`] later, launched at
/// one of her nation's ports.
fn replace_lost_merchants(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    ships: Query<&Name, With<ShipClass>>,
    ports: Query<(Entity, &Port, &Transform)>,
    mut shipwrights: ResMut<Shipwrights>,
    mut commands: Commands,
) {
    let now = clock.elapsed;
    let Some((index, listed)) = shipwrights.0.due(ships.iter(), now) else {
        return;
    };
    if let Some((port, anchorage, _)) = home(&harbours(&ports), listed.nation, index) {
        let berth = anchorage + Vec3::X * BERTH_SPACING;
        commands.spawn(merchant(&goods, listed, port, berth, now + IN_PORT));
    }
}

/// Whole tons of provisions to buy so `people` have [`PROVISION_DAYS`] aboard.
pub fn provisions_needed(people: f64, aboard: f64) -> f64 {
    (people * RATION_TONS * PROVISION_DAYS - aboard)
        .max(0.0)
        .ceil()
}

/// The best-paying job a ship docked at `port` can take, if any.
fn best_job<'a>(
    goods: &Goods,
    port: Entity,
    jobs: impl Iterator<Item = (Entity, &'a Job)>,
    markets: &Query<&Market>,
    hold: &Hold,
    crew: &Crew,
    complement: &Complement,
) -> Option<(Entity, Entity)> {
    jobs.filter(|(_, job)| {
        markets.get(job.origin).is_ok_and(|market| {
            check_acceptance(goods, job, port, market, hold, crew, complement).is_ok()
        })
    })
    .max_by_key(|(_, job)| job.payment)
    .map(|(entity, job)| (entity, job.destination))
}

/// The other port with the most open jobs waiting to be picked up there.
fn busiest_port<'a>(here: Entity, jobs: impl Iterator<Item = &'a Job>) -> Option<Entity> {
    let mut counts: Vec<(Entity, usize)> = Vec::new();
    for job in jobs.filter(|job| job.origin != here) {
        match counts.iter_mut().find(|(port, _)| *port == job.origin) {
            Some((_, count)) => *count += 1,
            None => counts.push((job.origin, 1)),
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(port, _)| port)
}

/// A merchant, as her captain sees her.
#[derive(QueryData)]
#[query_data(mutable)]
struct MerchantShip {
    entity: Entity,
    merchant: &'static mut Merchant,
    at: &'static Transform,
    hold: &'static Hold,
    crew: &'static Crew,
    complement: &'static Complement,
    colours: Option<&'static Colours>,
    under_way: Has<Route>,
}

/// The exchange, as a merchant captain deals with it: the open contracts,
/// who carries which, and taking one on.
#[derive(SystemParam)]
struct Exchange<'w, 's> {
    open: Query<'w, 's, (Entity, &'static Job), Without<Carrier>>,
    carried: Query<'w, 's, (&'static Job, &'static Carrier)>,
    accepts: MessageWriter<'w, AcceptJob>,
}

impl Exchange<'_, '_> {
    /// The contracts `ship` carries.
    fn carried_by(&self, ship: Entity) -> impl Iterator<Item = &Job> {
        self.carried
            .iter()
            .filter(move |(_, carrier)| carrier.0 == ship)
            .map(|(job, _)| job)
    }

    /// The open contracts anyone may take.
    fn for_anyone(&self) -> impl Iterator<Item = (Entity, &Job)> {
        self.open.iter().filter(|(_, job)| job.for_anyone())
    }
}

/// Escorts asked for, and the captains in port who might take them on.
#[derive(SystemParam)]
struct EscortDesk<'w, 's> {
    escorts: Query<'w, 's, &'static Escort>,
    players: Query<'w, 's, &'static Docked, With<Player>>,
    relations: Res<'w, Relations>,
    commands: Commands<'w, 's>,
}

impl EscortDesk<'_, '_> {
    /// With a captain in port at `here` and raiders about, a merchant
    /// sailing for `bound_for` with `job` asks for an escort. Whether she
    /// has.
    fn ask(
        &mut self,
        ship: Entity,
        colours: Option<&Colours>,
        job: Option<&Job>,
        here: Entity,
        bound_for: Entity,
    ) -> bool {
        let wants = colours.zip(job).filter(|(colours, _)| {
            wants_escort(colours.0, &self.relations)
                && self.players.iter().any(|docked| docked.port == here)
                && !self.escorts.iter().any(|escort| escort.ward == ship)
        });
        let Some((colours, job)) = wants else {
            return false;
        };
        self.commands.spawn(Escort {
            ward: ship,
            nation: colours.0,
            origin: here,
            destination: bound_for,
            fee: escort_fee(job.payment),
            escort: None,
            lost_at: None,
        });
        true
    }

    /// Where the captain escorting `ship` lies, if she has one in port.
    fn escort_lies(&self, ship: Entity) -> Option<Entity> {
        self.escorts
            .iter()
            .find(|escort| escort.ward == ship)
            .and_then(|escort| self.players.get(escort.escort?).ok())
            .map(|docked| docked.port)
    }
}

/// A merchant at sea dives on her wreck while there's room and something
/// to bring up; otherwise, with no voyage, as after a fight, she makes for
/// the nearest port.
fn merchant_at_sea(
    goods: Res<Goods>,
    talk: WreckTalk,
    ports: Query<(Entity, &Transform), With<Port>>,
    mut merchants: Query<MerchantShip, (AtHerBusiness, Without<Running>, Without<Docked>)>,
    mut orders: MessageWriter<SailOrder>,
) {
    for mut ship in &mut merchants {
        let at = ship.at.translation;
        if let Plan::Salvaging { wreck } = ship.merchant.plan {
            let room = ship.hold.room(
                &goods,
                Carriage::Hold,
                ship.crew.count,
                ship.complement.berths,
            );
            if let Ok((_, lies, remains)) = talk.wrecks.get(wreck)
                && remains.has_anything()
                && room >= 1.0
            {
                if !ship.under_way && at.distance(lies.translation) > SALVAGE_RANGE {
                    orders.write(SailOrder {
                        ship: ship.entity,
                        destination: lies.translation,
                        on_arrival: Arrival::HeaveTo,
                    });
                }
                continue;
            }
        }
        ship.merchant.plan = Plan::AtSea;
        if !ship.under_way
            && let Some((port, anchorage)) = ports
                .iter()
                .min_by_key(|(_, anchorage)| FloatOrd(anchorage.translation.distance(at)))
        {
            orders.write(SailOrder {
                ship: ship.entity,
                destination: anchorage.translation,
                on_arrival: Arrival::Dock(port),
            });
        }
    }
}

/// A merchant in port settles in, sells what she salvaged, and looks for
/// work: a contract here, a wreck talked of, or the port with the most work
/// waiting. She victuals, asks for an escort if she'd want one, loads, and
/// sails.
fn merchant_in_port(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    mut merchants: Query<(MerchantShip, &Docked), (AtHerBusiness, Without<Running>)>,
    mut exchange: Exchange,
    mut desk: EscortDesk,
    markets: Query<&Market>,
    ports: Query<(Entity, &Transform), With<Port>>,
    talk: WreckTalk,
    mut trades: MessageWriter<Trade>,
    mut put_to_sea: PutToSea,
) {
    let now = clock.elapsed;
    for (mut ship, &Docked { port: here }) in &mut merchants {
        let entity = ship.entity;
        ship.merchant.plan = match ship.merchant.plan {
            Plan::AtSea | Plan::Salvaging { .. } => {
                // Sell what she salvaged.
                if let Ok(market) = markets.get(here) {
                    trades.write_batch(sell_cargo(&goods, entity, ship.hold, market));
                }
                Plan::InPort {
                    until: now + IN_PORT,
                }
            }
            Plan::InPort { until } if now >= until => {
                let carrying = exchange
                    .carried_by(entity)
                    .find(|job| job.destination != here)
                    .cloned();
                // One contract at a time: take on work only with none aboard.
                let job = carrying
                    .is_none()
                    .then(|| {
                        best_job(
                            &goods,
                            here,
                            exchange.for_anyone().filter(|(_, job)| job.origin == here),
                            &markets,
                            ship.hold,
                            ship.crew,
                            ship.complement,
                        )
                    })
                    .flatten();
                let contract = carrying.or_else(|| {
                    let (entity, _) = job?;
                    exchange.open.get(entity).ok().map(|(_, job)| job.clone())
                });
                // With no contract, a wreck talked of here beats looking
                // for work elsewhere.
                let wreck = contract
                    .is_none()
                    .then(|| talk.known_at(here, ship.at.translation))
                    .and_then(|known| known.first().copied());
                if let Some((wreck, lies)) = wreck {
                    put_to_sea.sail(entity, lies, Arrival::HeaveTo, None);
                    ship.merchant.plan = Plan::Salvaging { wreck };
                    continue;
                }
                let bound_for = contract
                    .as_ref()
                    .map(|job| job.destination)
                    .or_else(|| busiest_port(here, exchange.for_anyone().map(|(_, job)| job)));
                match bound_for {
                    Some(bound_for) => {
                        if let Some((job, _)) = job {
                            exchange.accepts.write(AcceptJob { ship: entity, job });
                        }
                        let people = ship.crew.count as f64 + ship.hold.passengers(&goods);
                        let tons = provisions_needed(people, ship.hold.owned.get(goods.provisions));
                        if tons > 0.0 {
                            trades.write(Trade {
                                ship: entity,
                                good: goods.provisions,
                                quantity: tons as u32,
                                side: Side::Buy,
                            });
                        }
                        // Asking for an escort, she gives her time to answer.
                        let asked =
                            desk.ask(entity, ship.colours, contract.as_ref(), here, bound_for);
                        let loading = if asked { ESCORT_WAIT } else { LOADING };
                        Plan::Loading {
                            bound_for,
                            sails_at: now + loading,
                        }
                    }
                    None => Plan::InPort {
                        until: now + NO_WORK_RETRY,
                    },
                }
            }
            Plan::Loading {
                bound_for,
                sails_at,
            } if now >= sails_at => {
                if waits_for_escort(desk.escort_lies(entity), here, now, sails_at) {
                    ship.merchant.plan
                } else {
                    if let Ok((_, anchorage)) = ports.get(bound_for) {
                        let told = exchange
                            .carried_by(entity)
                            .find(|job| job.destination == bound_for)
                            .map(|job| {
                                let purpose = Purpose::Carrying {
                                    good: job.good,
                                    quantity: job.quantity,
                                    bound_for,
                                };
                                (here, purpose)
                            });
                        put_to_sea.sail(
                            entity,
                            anchorage.translation,
                            Arrival::Dock(bound_for),
                            told,
                        );
                    }
                    // Still docked until the cast-off order is carried out;
                    // `AtSea` waits that out.
                    Plan::AtSea
                }
            }
            plan => plan,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::market::MarketEntry;
    use crate::port::Location;
    use crate::test_support::{run_until, sim_app};
    use crate::time_scale::MAX_STEP;

    /// Two ports 1 km apart in open water. The first has naval stores to
    /// spare; the second is short of them.
    fn two_ports(app: &mut App) -> (Entity, Entity) {
        // Merchants work on a scale of hours, so step as at full time compression.
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let goods = app.world().resource::<Goods>().clone();
        let stores = goods.id("naval_stores").unwrap();
        let provisions = goods.provisions;
        let mut spawn = |x: f32, stock: f64, per_day: f64| {
            app.world_mut()
                .spawn((
                    Port {
                        flag: Nation::ENGLAND,
                        town: Vec3::new(x, 0.0, 200.0),
                        locations: vec![Location::Market, Location::MerchantsExchange],
                    },
                    Market {
                        entries: vec![
                            MarketEntry {
                                good: stores,
                                stock,
                                target: 50.0,
                                per_day,
                                short_for: 0.0,
                            },
                            MarketEntry {
                                good: provisions,
                                stock: 100.0,
                                target: 100.0,
                                per_day: 1.0,
                                short_for: 0.0,
                            },
                        ],
                    },
                    Transform::from_xyz(x, 0.0, 0.0),
                ))
                .id()
        };
        (spawn(0.0, 100.0, 1.0), spawn(1000.0, 0.0, -1.0))
    }

    fn spawn_merchant(app: &mut App, port: Entity) -> Entity {
        let goods = app.world().resource::<Goods>().clone();
        let fleet = merchant_fleet(&goods, &[(port, Nation::ENGLAND, Vec3::ZERO)]);
        let ship = app
            .world_mut()
            .spawn(fleet.into_iter().next().unwrap())
            .id();
        // Hold her in port only briefly.
        app.world_mut().get_mut::<Merchant>(ship).unwrap().plan = Plan::InPort { until: 0.0 };
        ship
    }

    #[test]
    fn provisions_top_up_to_the_voyage_allowance() {
        let full = 10.0 * RATION_TONS * PROVISION_DAYS;
        assert_eq!(provisions_needed(10.0, 0.0), full.ceil());
        assert_eq!(provisions_needed(10.0, full), 0.0);
        assert_eq!(provisions_needed(10.0, 50.0), 0.0, "never sells back");
    }

    #[test]
    fn busiest_port_is_where_most_work_waits_elsewhere() {
        let mut world = World::new();
        let [here, a, b] = [(); 3].map(|_| world.spawn_empty().id());
        let good = Goods::standard().provisions;
        let job = |origin| Job {
            good,
            quantity: 1,
            origin,
            destination: here,
            payment: 1,
            deadline: 0.0,
            introduction: None,
        };
        let jobs = [job(a), job(b), job(b), job(here), job(here), job(here)];
        assert_eq!(busiest_port(here, jobs.iter()), Some(b));
        assert_eq!(busiest_port(here, [job(here)].iter()), None);
    }

    #[test]
    fn a_merchant_carries_a_job_to_the_port_in_need_and_is_paid() {
        let mut app = sim_app();
        let (supplier, needy) = two_ports(&mut app);
        let ship = spawn_merchant(&mut app, supplier);
        let delivered = run_until(&mut app, 2.0 * 24.0 * HOUR as f32, |world| {
            world.get::<Docked>(ship) == Some(&Docked { port: needy })
                && world.get::<Purse>(ship).unwrap().0 > PURSE
        });
        assert!(delivered, "never delivered");
        let world = app.world_mut();
        assert_eq!(world.query::<&Job>().iter(world).count(), 0, "job done");
    }

    #[test]
    fn a_merchant_asks_the_captain_in_port_to_escort_her_and_sails_with_her() {
        use crate::escorts::AcceptEscort;
        use crate::word::{Happening, Ledger};
        let mut app = sim_app();
        let (supplier, needy) = two_ports(&mut app);
        let ship = spawn_merchant(&mut app, supplier);
        let player = app
            .world_mut()
            .spawn((Player, Docked { port: supplier }, Transform::default()))
            .id();
        let request = |world: &World| {
            world
                .iter_entities()
                .find(|entity| entity.get::<Escort>().is_some_and(|e| e.ward == ship))
                .map(|entity| entity.id())
        };
        // Contracts are posted in the first step, maybe after she's looked.
        let looked_twice = (NO_WORK_RETRY + HOUR) as f32;
        assert!(run_until(&mut app, looked_twice, |world| request(world).is_some()));
        let escort = request(app.world()).unwrap();
        app.world_mut().write_message(AcceptEscort {
            ship: player,
            escort,
        });
        let sailed = |world: &World| world.get::<Docked>(ship).is_none();
        let wait = ESCORT_WAIT + 2.0 * HOUR;
        assert!(
            !run_until(&mut app, wait as f32, sailed),
            "she waits for her escort"
        );
        app.world_mut().entity_mut(player).remove::<Docked>();
        assert!(
            run_until(&mut app, HOUR as f32, sailed),
            "she sails with her"
        );
        let ledger = app.world().resource::<Ledger>();
        let word = ledger.news.iter().find_map(|news| match news.event {
            Happening::Sailed {
                ship: sailed,
                purpose,
                ..
            } if sailed == ship => Some(purpose),
            _ => None,
        });
        assert!(
            matches!(word, Some(Purpose::Carrying { bound_for, .. }) if bound_for == needy),
            "word gets about: {word:?}"
        );
    }

    #[test]
    fn a_lost_merchant_is_replaced_after_a_while() {
        let mut app = sim_app();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let (supplier, _) = two_ports(&mut app);
        let goods = app.world().resource::<Goods>().clone();
        let fleet = merchant_fleet(&goods, &[(supplier, Nation::ENGLAND, Vec3::ZERO)]);
        let ships: Vec<Entity> = fleet
            .into_iter()
            .map(|merchant| app.world_mut().spawn(merchant).id())
            .collect();
        app.world_mut().despawn(ships[1]);
        let count = |app: &mut App| {
            app.world_mut()
                .query_filtered::<(), With<Merchant>>()
                .iter(app.world())
                .count()
        };
        run_until(&mut app, (REPLACEMENT * 0.9) as f32, |_| false);
        let fleet = Shipwrights::default().0.ships;
        assert_eq!(count(&mut app), fleet.len() - 1, "not yet");
        run_until(&mut app, (REPLACEMENT * 0.2) as f32, |_| false);
        assert_eq!(count(&mut app), fleet.len());
        let name = app.world().get::<Name>(ships[0]).unwrap().clone();
        let names: Vec<String> = app
            .world_mut()
            .query::<&Name>()
            .iter(app.world())
            .map(|n| n.to_string())
            .collect();
        assert_eq!(names.iter().filter(|n| **n == name.as_str()).count(), 1);
        assert!(
            names.contains(&fleet[1].name.to_string()),
            "the lost name is reused"
        );
    }

    #[test]
    fn a_merchant_without_work_waits_in_port() {
        let mut app = sim_app();
        let (supplier, _) = two_ports(&mut app);
        // Nobody is short of anything.
        for mut market in app
            .world_mut()
            .query::<&mut Market>()
            .iter_mut(app.world_mut())
        {
            for entry in &mut market.entries {
                entry.stock = entry.target;
            }
        }
        let ship = spawn_merchant(&mut app, supplier);
        let left = run_until(&mut app, 3.0 * HOUR as f32, |world| {
            world.get::<Docked>(ship).is_none()
        });
        assert!(!left);
    }
}
