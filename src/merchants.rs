//! Merchant captains: NPC ships that carry the ports' contracts.
//!
//! A merchant lies in port a while, takes the best-paying job on offer,
//! tops up her provisions, and sails to deliver it. With no work here, she
//! sails to the port with the most work waiting. A contract already aboard
//! comes first. She gives exactly the
//! orders the player gives ([`AcceptJob`], [`Trade`], [`CastOff`],
//! [`SailOrder`]), so she docks, delivers, and is paid by the same rules.

use bevy::math::FloatOrd;
use bevy::prelude::*;

use crate::cargo::{Hold, Purse};
use crate::clock::{DAY, HOUR, WorldClock};
use crate::combat::{Engaged, Struck};
use crate::crew::{Complement, Crew, RATION_TONS};
use crate::economy::jobs::{AcceptJob, Carrier, Job, check_acceptance};
use crate::economy::market::{Market, Side, Trade};
use crate::factions::{Colours, Nation};
use crate::goods::Goods;
use crate::navigation::{Arrival, Route, SailOrder};
use crate::port::{CastOff, Docked, Port};
use crate::roster::Roster;
use crate::schedule::SimSet;
use crate::ship_class::ShipClass;

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
/// Each merchant's crew and opening purse.
const CREW: u32 = 10;
const PURSE: u32 = 300;
/// Merchants carry a few guns for show, and more cargo.
const GUNS: u32 = 4;
/// The merchant fleet: one sloop per name, spread across her nation's ports.
const NAMES: [&str; 7] = [
    "Swallow",
    "Mary Ann",
    "Endeavour",
    "Two Brothers",
    "Dove",
    "Santa Rosa",
    "San José",
];
/// The nation each of [`NAMES`] sails for.
const FLAGS: [Nation; NAMES.len()] = [
    Nation::England,
    Nation::England,
    Nation::England,
    Nation::England,
    Nation::England,
    Nation::Spain,
    Nation::Spain,
];
/// A lost merchant is replaced this long after she's gone.
pub const REPLACEMENT: f64 = 2.0 * DAY;
/// Merchants sharing an anchorage lie this far apart, in meters.
const BERTH_SPACING: f32 = 30.0;

/// An NPC merchant captain, and what she means to do next.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
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
}

/// The merchant fleet's roster.
#[derive(Resource, Debug)]
struct Shipwrights(Roster);

impl Default for Shipwrights {
    fn default() -> Self {
        Self(Roster::new(&NAMES, REPLACEMENT))
    }
}

pub struct MerchantsPlugin;

impl Plugin for MerchantsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shipwrights>().add_systems(
            FixedUpdate,
            (captain_merchants, replace_lost_merchants).in_set(SimSet::Economy),
        );
    }
}

/// A new merchant called `name` under `nation`'s colours, lying at `port`
/// until `until`.
fn merchant(
    goods: &Goods,
    name: &str,
    nation: Nation,
    port: Entity,
    at: Vec3,
    until: f64,
) -> impl Bundle {
    let class = ShipClass::SLOOP;
    let mut hold = class.empty_hold();
    hold.owned
        .add(goods.provisions, provisions_needed(CREW as f64, 0.0));
    (
        Name::new(name.to_string()),
        Merchant {
            plan: Plan::InPort { until },
        },
        class.build(CREW, GUNS, hold),
        Purse(PURSE),
        Colours(nation),
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
    NAMES
        .iter()
        .zip(FLAGS)
        .enumerate()
        .filter_map(|(i, (name, nation))| {
            // Her place among her own nation's merchants.
            let index = FLAGS[..i].iter().filter(|&&flag| flag == nation).count();
            let (port, anchorage, before) = home(ports, nation, index)?;
            let berth = (before + 1) as f32 * BERTH_SPACING;
            let until = OPENING_STAGGER * (i + 1) as f64;
            Some(merchant(
                goods,
                name,
                nation,
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
    let Some((index, name)) = shipwrights.0.due(ships.iter(), now) else {
        return;
    };
    let nation = FLAGS[index];
    if let Some((port, anchorage, _)) = home(&harbours(&ports), nation, index) {
        let berth = anchorage + Vec3::X * BERTH_SPACING;
        commands.spawn(merchant(&goods, name, nation, port, berth, now + IN_PORT));
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

fn captain_merchants(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    mut merchants: Query<
        (
            Entity,
            &mut Merchant,
            Option<&Docked>,
            Has<Route>,
            &Transform,
            &Hold,
            &Crew,
            &Complement,
        ),
        // In battle, her tactics have the helm.
        (Without<Engaged>, Without<Struck>),
    >,
    open_jobs: Query<(Entity, &Job), Without<Carrier>>,
    carried_jobs: Query<(&Job, &Carrier)>,
    markets: Query<&Market>,
    ports: Query<(Entity, &Transform), With<Port>>,
    mut accepts: MessageWriter<AcceptJob>,
    mut trades: MessageWriter<Trade>,
    mut cast_offs: MessageWriter<CastOff>,
    mut orders: MessageWriter<SailOrder>,
) {
    let now = clock.elapsed;
    for (ship, mut merchant, docked, under_way, transform, hold, crew, complement) in &mut merchants
    {
        let Some(&Docked { port: here }) = docked else {
            merchant.plan = Plan::AtSea;
            // Adrift with no voyage, as after a fight: make for the nearest port.
            if !under_way
                && let Some((port, anchorage)) = ports.iter().min_by_key(|(_, anchorage)| {
                    FloatOrd(anchorage.translation.distance(transform.translation))
                })
            {
                orders.write(SailOrder {
                    ship,
                    destination: anchorage.translation,
                    on_arrival: Arrival::Dock(port),
                });
            }
            continue;
        };
        merchant.plan = match merchant.plan {
            Plan::AtSea => Plan::InPort {
                until: now + IN_PORT,
            },
            Plan::InPort { until } if now >= until => {
                let carrying = carried_jobs
                    .iter()
                    .find(|(job, carrier)| carrier.0 == ship && job.destination != here)
                    .map(|(job, _)| job.destination);
                // One contract at a time: take on work only with none aboard.
                let job = if carrying.is_none() {
                    best_job(
                        &goods,
                        here,
                        open_jobs.iter().filter(|(_, job)| job.origin == here),
                        &markets,
                        hold,
                        crew,
                        complement,
                    )
                } else {
                    None
                };
                let bound_for = carrying
                    .or(job.map(|(_, destination)| destination))
                    .or_else(|| busiest_port(here, open_jobs.iter().map(|(_, job)| job)));
                match bound_for {
                    Some(bound_for) => {
                        if let Some((job, _)) = job {
                            accepts.write(AcceptJob { ship, job });
                        }
                        let people = crew.count as f64 + hold.passengers(&goods);
                        let tons = provisions_needed(people, hold.owned.get(goods.provisions));
                        if tons > 0.0 {
                            trades.write(Trade {
                                ship,
                                good: goods.provisions,
                                quantity: tons as u32,
                                side: Side::Buy,
                            });
                        }
                        Plan::Loading {
                            bound_for,
                            sails_at: now + LOADING,
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
                if let Ok((_, anchorage)) = ports.get(bound_for) {
                    cast_offs.write(CastOff { ship });
                    orders.write(SailOrder {
                        ship,
                        destination: anchorage.translation,
                        on_arrival: Arrival::Dock(bound_for),
                    });
                }
                // Still docked until the cast-off order is carried out;
                // `AtSea` waits that out.
                Plan::AtSea
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
                        flag: Nation::England,
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
                            },
                            MarketEntry {
                                good: provisions,
                                stock: 100.0,
                                target: 100.0,
                                per_day: 1.0,
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
        let fleet = merchant_fleet(&goods, &[(port, Nation::England, Vec3::ZERO)]);
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
    fn a_lost_merchant_is_replaced_after_a_while() {
        let mut app = sim_app();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let (supplier, _) = two_ports(&mut app);
        let goods = app.world().resource::<Goods>().clone();
        let fleet = merchant_fleet(&goods, &[(supplier, Nation::England, Vec3::ZERO)]);
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
        assert_eq!(count(&mut app), NAMES.len() - 1, "not yet");
        run_until(&mut app, (REPLACEMENT * 0.2) as f32, |_| false);
        assert_eq!(count(&mut app), NAMES.len());
        let name = app.world().get::<Name>(ships[0]).unwrap().clone();
        let names: Vec<String> = app
            .world_mut()
            .query::<&Name>()
            .iter(app.world())
            .map(|n| n.to_string())
            .collect();
        assert_eq!(names.iter().filter(|n| **n == name.as_str()).count(), 1);
        assert!(
            names.contains(&NAMES[1].to_string()),
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
