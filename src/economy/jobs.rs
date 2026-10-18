//! Contracts: deliveries that ports post when they run short.
//!
//! A port whose stock of a needed good falls below [`JOB_THRESHOLD`] of its
//! target posts a job at the nearest port producing that good. The shipper
//! hands over the goods (or soldiers) when a captain accepts. Delivering
//! them to the port in need pays the fee and restocks it.

use bevy::math::FloatOrd;
use bevy::prelude::*;

use super::Refusal;
use super::market::{Market, MarketEntry, Supplied};
use crate::alert::{Alert, AlertKind, fell_below};
use crate::cargo::{Hold, Purse};
use crate::clock::{DAY, WorldClock};
use crate::crew::{Complement, Crew};
use crate::factions::{Deed, DeedKind, Relations};
use crate::goods::{Carriage, GoodId, Goods};
use crate::map::Land;
use crate::navigation::route::{NavGraph, route_length};
use crate::port::{Docked, Location, Port};

/// A needed good whose stock falls below this fraction of target gets a job posted.
pub const JOB_THRESHOLD: f64 = 0.5;
/// Largest single consignment, in tons or heads, so a small ship can carry it.
pub const MAX_JOB: u32 = 30;
/// Speed (m/s) assumed when setting deadlines: a slow merchantman's average.
const PLANNING_SPEED: f32 = 2.5;
/// Fraction of the fee paid for delivering after the deadline.
const LATE_PAY: f64 = 0.5;

/// A delivery contract.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Job {
    pub good: GoodId,
    pub quantity: u32,
    /// Where the goods are picked up, and where the job is offered.
    pub origin: Entity,
    /// The port that needs them.
    pub destination: Entity,
    /// Fee in pieces of eight for delivery on time.
    pub payment: u32,
    /// [`WorldClock`] time by which the job should be delivered.
    pub deadline: f64,
}

/// The ship that accepted a job.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Carrier(pub Entity);

/// Order a docked ship to take on a job at its port.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptJob {
    pub ship: Entity,
    pub job: Entity,
}

/// How much a port in need should ask for, if its stock is low enough to post a job.
pub fn shortfall(entry: &MarketEntry) -> Option<u32> {
    (entry.is_needed() && entry.stock < entry.target * JOB_THRESHOLD)
        .then(|| ((entry.target - entry.stock).round() as u32).clamp(1, MAX_JOB))
}

/// The fee for carrying `quantity` tons or heads `distance` meters.
pub fn payment(carriage: Carriage, quantity: u32, distance: f32) -> u32 {
    // A flat fee for loading and unloading, plus a rate per kilometer.
    let (flat, per_km) = match carriage {
        Carriage::Hold => (5.0, 0.05),
        Carriage::Berths => (3.0, 0.03),
    };
    (quantity as f32 * (flat + per_km * distance / 1000.0)).round() as u32
}

/// When a job posted at `now` for a passage of `distance` meters falls due:
/// twice the time a slow merchantman needs, plus a day to load.
pub fn deadline(now: f64, distance: f32) -> f64 {
    now + 2.0 * (distance / PLANNING_SPEED) as f64 + DAY
}

/// What a job pays if delivered at `now`.
pub fn pay_on_delivery(job: &Job, now: f64) -> u32 {
    if now <= job.deadline {
        job.payment
    } else {
        (job.payment as f64 * LATE_PAY).round() as u32
    }
}

/// Where in a town jobs for goods of `carriage` are offered. Soldiers are
/// the governor's business and freight the merchants'. Ports without the
/// right office do business in the tavern.
pub fn job_office(locations: &[Location], carriage: Carriage) -> Option<Location> {
    let office = match carriage {
        Carriage::Hold => Location::MerchantsExchange,
        Carriage::Berths => Location::GovernorsHouse,
    };
    [office, Location::Tavern]
        .into_iter()
        .find(|location| locations.contains(location))
}

/// Whether a ship docked at `port` can take on `job`. Both the port screen
/// and the acceptance itself use this.
pub fn check_acceptance(
    goods: &Goods,
    job: &Job,
    port: Entity,
    origin_market: &Market,
    hold: &Hold,
    crew: &Crew,
    complement: &Complement,
) -> Result<(), Refusal> {
    let quantity = job.quantity as f64;
    if port != job.origin {
        Err(Refusal::NotHere)
    } else if origin_market
        .entry(job.good)
        .is_none_or(|entry| entry.stock < quantity)
    {
        Err(Refusal::OutOfStock)
    } else if hold.room(
        goods,
        goods.carriage(job.good),
        crew.count,
        complement.berths,
    ) < quantity
    {
        Err(Refusal::NoRoom)
    } else {
        Ok(())
    }
}

/// Posts a job for every shortfall that doesn't already have one, at the
/// nearest port that produces the good and has stock to spare. No port
/// ships goods to an enemy's.
pub(super) fn post_jobs(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    land: Res<Land>,
    graph: Res<NavGraph>,
    relations: Res<Relations>,
    ports: Query<(Entity, &Port, &Market, &Transform)>,
    jobs: Query<&Job>,
    mut commands: Commands,
) {
    for (destination, in_need, market, to) in &ports {
        for entry in &market.entries {
            let Some(quantity) = shortfall(entry) else {
                continue;
            };
            if jobs
                .iter()
                .any(|job| job.destination == destination && job.good == entry.good)
            {
                continue;
            }
            let carriage = goods.carriage(entry.good);
            let nearest = ports
                .iter()
                .filter(|&(origin, port, market, _)| {
                    origin != destination
                        && !relations.at_war(port.flag, in_need.flag)
                        && job_office(&port.locations, carriage).is_some()
                        && market.entry(entry.good).is_some_and(|supply| {
                            supply.is_produced() && supply.stock >= quantity as f64
                        })
                })
                .map(|(origin, _, _, from)| {
                    let route = graph.plan(&land, from.translation, to.translation);
                    (origin, route_length(from.translation, &route))
                })
                .min_by_key(|&(_, distance)| FloatOrd(distance));
            if let Some((origin, distance)) = nearest {
                commands.spawn(Job {
                    good: entry.good,
                    quantity,
                    origin,
                    destination,
                    payment: payment(carriage, quantity, distance),
                    deadline: deadline(clock.elapsed, distance),
                });
            }
        }
    }
}

/// Jobs nobody took in time are withdrawn.
pub(super) fn expire_jobs(
    clock: Res<WorldClock>,
    jobs: Query<(Entity, &Job), Without<Carrier>>,
    mut commands: Commands,
) {
    for (entity, job) in &jobs {
        if clock.elapsed > job.deadline {
            commands.entity(entity).despawn();
        }
    }
}

pub(super) fn accept_jobs(
    mut orders: MessageReader<AcceptJob>,
    goods: Res<Goods>,
    jobs: Query<&Job, Without<Carrier>>,
    mut ships: Query<(&Docked, &mut Hold, &Crew, &Complement)>,
    mut markets: Query<&mut Market>,
    mut commands: Commands,
) {
    let mut accepted = Vec::new();
    for order in orders.read() {
        let Ok(job) = jobs.get(order.job) else {
            continue;
        };
        let Ok((docked, mut hold, crew, complement)) = ships.get_mut(order.ship) else {
            continue;
        };
        let Ok(mut market) = markets.get_mut(job.origin) else {
            continue;
        };
        // `Carrier` is inserted by command, so guard against two orders for one job this frame.
        if accepted.contains(&order.job)
            || check_acceptance(&goods, job, docked.port, &market, &hold, crew, complement).is_err()
        {
            continue;
        }
        let quantity = job.quantity as f64;
        if let Some(entry) = market.entry_mut(job.good) {
            entry.stock -= quantity;
        }
        hold.consigned.add(job.good, quantity);
        commands.entity(order.job).insert(Carrier(order.ship));
        accepted.push(order.job);
    }
}

/// Warns a carrier once when its contract falls due within a day.
pub(super) fn warn_of_deadlines(
    time: Res<Time>,
    clock: Res<WorldClock>,
    jobs: Query<(Entity, &Job, &Carrier)>,
    mut alerts: MessageWriter<Alert>,
) {
    let before = clock.elapsed - time.delta_secs_f64();
    for (job, details, carrier) in &jobs {
        if fell_below(
            details.deadline - before,
            details.deadline - clock.elapsed,
            DAY,
        ) {
            alerts.write(Alert {
                ship: carrier.0,
                kind: AlertKind::DeadlineNear { job },
            });
        }
    }
}

/// A carrier docked at a job's destination hands over the consignment and is paid.
pub(super) fn deliver_jobs(
    clock: Res<WorldClock>,
    jobs: Query<(Entity, &Job, &Carrier)>,
    mut ships: Query<(&Docked, &mut Hold, &mut Purse)>,
    mut markets: Query<(&mut Market, &Port)>,
    mut deeds: MessageWriter<Deed>,
    mut supplies: MessageWriter<Supplied>,
    mut commands: Commands,
) {
    for (entity, job, carrier) in &jobs {
        let Ok((docked, mut hold, mut purse)) = ships.get_mut(carrier.0) else {
            continue;
        };
        let quantity = job.quantity as f64;
        if docked.port != job.destination || hold.consigned.get(job.good) < quantity {
            continue;
        }
        let Ok((mut market, port)) = markets.get_mut(job.destination) else {
            continue;
        };
        deeds.write(Deed {
            doer: carrier.0,
            kind: DeedKind::Delivered,
            nation: port.flag,
        });
        hold.consigned.remove(job.good, quantity);
        if let Some(entry) = market.entry_mut(job.good) {
            supplies.write(Supplied {
                port: job.destination,
                ship: carrier.0,
                share: entry.gap_filled(quantity),
            });
            entry.stock += quantity;
        }
        purse.0 += pay_on_delivery(job, clock.elapsed);
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{run_checked, run_seconds, sim_app};

    fn needed(good: GoodId, stock: f64) -> MarketEntry {
        MarketEntry {
            good,
            stock,
            target: 100.0,
            per_day: -1.0,
        }
    }

    fn produced(good: GoodId, stock: f64) -> MarketEntry {
        MarketEntry {
            per_day: 1.0,
            ..needed(good, stock)
        }
    }

    #[test]
    fn only_needed_goods_running_low_have_a_shortfall() {
        let goods = Goods::standard();
        let rum = goods.id("rum").unwrap();
        assert_eq!(shortfall(&needed(rum, 60.0)), None, "not low enough yet");
        assert_eq!(shortfall(&needed(rum, 40.0)), Some(MAX_JOB), "capped");
        assert_eq!(
            shortfall(&produced(rum, 0.0)),
            None,
            "the port makes its own"
        );
        let small = MarketEntry {
            target: 10.0,
            ..needed(rum, 2.0)
        };
        assert_eq!(shortfall(&small), Some(8));
    }

    #[test]
    fn longer_passages_pay_more() {
        let near = payment(Carriage::Hold, 20, 5_000.0);
        let far = payment(Carriage::Hold, 20, 500_000.0);
        assert!(near > 0 && far > near * 5, "near {near}, far {far}");
        assert!(payment(Carriage::Hold, 40, 5_000.0) > near);
    }

    #[test]
    fn late_delivery_pays_half() {
        let job = Job {
            good: Goods::standard().provisions,
            quantity: 10,
            origin: Entity::PLACEHOLDER,
            destination: Entity::PLACEHOLDER,
            payment: 100,
            deadline: 1_000.0,
        };
        assert_eq!(pay_on_delivery(&job, 1_000.0), 100);
        assert_eq!(pay_on_delivery(&job, 1_001.0), 50);
    }

    #[test]
    fn jobs_are_offered_at_the_right_office_or_else_the_tavern() {
        use Location::*;
        assert_eq!(
            job_office(&[Tavern, MerchantsExchange], Carriage::Hold),
            Some(MerchantsExchange)
        );
        assert_eq!(job_office(&[Tavern, Market], Carriage::Hold), Some(Tavern));
        assert_eq!(
            job_office(&[GovernorsHouse, Tavern], Carriage::Berths),
            Some(GovernorsHouse)
        );
        assert_eq!(job_office(&[Market], Carriage::Berths), None);
    }

    struct Harbour {
        app: App,
        goods: Goods,
        rum: GoodId,
        producer: Entity,
        consumer: Entity,
    }

    /// Two ports 2 km apart: one makes rum, one is running out of it.
    fn harbour() -> Harbour {
        let mut app = sim_app();
        let goods = app.world().resource::<Goods>().clone();
        let rum = goods.id("rum").unwrap();
        let port = |locations| Port {
            flag: crate::factions::Nation::England,
            town: Vec3::ZERO,
            locations,
        };
        let producer = app
            .world_mut()
            .spawn((
                port(vec![Location::MerchantsExchange]),
                Market {
                    entries: vec![produced(rum, 100.0)],
                },
                Transform::default(),
            ))
            .id();
        let consumer = app
            .world_mut()
            .spawn((
                port(vec![Location::Market]),
                Market {
                    entries: vec![needed(rum, 10.0)],
                },
                Transform::from_xyz(2_000.0, 0.0, 0.0),
            ))
            .id();
        Harbour {
            app,
            goods,
            rum,
            producer,
            consumer,
        }
    }

    fn jobs(app: &mut App) -> Vec<(Entity, Job)> {
        app.world_mut()
            .query::<(Entity, &Job)>()
            .iter(app.world())
            .map(|(entity, job)| (entity, job.clone()))
            .collect()
    }

    #[test]
    fn a_port_running_short_posts_one_job_at_the_producer() {
        let mut h = harbour();
        run_seconds(&mut h.app, 5.0);
        let jobs = jobs(&mut h.app);
        assert_eq!(jobs.len(), 1, "posted once, not every step");
        let job = &jobs[0].1;
        assert_eq!((job.origin, job.destination), (h.producer, h.consumer));
        assert_eq!((job.good, job.quantity), (h.rum, MAX_JOB));
        assert!(job.payment > 0);
    }

    #[test]
    fn unaccepted_jobs_expire_at_their_deadline() {
        let mut h = harbour();
        run_seconds(&mut h.app, 1.0);
        let (entity, job) = jobs(&mut h.app)[0].clone();
        h.app.world_mut().resource_mut::<WorldClock>().elapsed = job.deadline + 1.0;
        h.app.update();
        assert!(h.app.world().get_entity(entity).is_err());
    }

    #[test]
    fn accepting_loads_the_consignment_and_delivery_pays_and_restocks() {
        let mut h = harbour();
        run_seconds(&mut h.app, 1.0);
        let (job, details) = jobs(&mut h.app)[0].clone();
        let ship = h
            .app
            .world_mut()
            .spawn((
                Docked { port: h.producer },
                Hold::new(40.0),
                Purse(0),
                Crew::new(10),
                Complement {
                    min: 8,
                    full: 40,
                    berths: 75,
                },
            ))
            .id();
        h.app.world_mut().write_message(AcceptJob { ship, job });
        h.app.update();

        let quantity = details.quantity as f64;
        assert_eq!(h.app.world().get::<Carrier>(job), Some(&Carrier(ship)));
        assert_eq!(
            h.app
                .world()
                .get::<Hold>(ship)
                .unwrap()
                .consigned
                .get(h.rum),
            quantity
        );
        let producer_stock = h.app.world().get::<Market>(h.producer).unwrap().entries[0].stock;
        assert!(producer_stock < 100.0 - quantity + 0.01);

        h.app
            .world_mut()
            .entity_mut(ship)
            .insert(Docked { port: h.consumer });
        let consumer_stock = h.app.world().get::<Market>(h.consumer).unwrap().entries[0].stock;
        h.app.update();

        assert!(h.app.world().get_entity(job).is_err(), "job complete");
        assert_eq!(h.app.world().get::<Purse>(ship).unwrap().0, details.payment);
        assert!(
            h.app
                .world()
                .get::<Hold>(ship)
                .unwrap()
                .consigned
                .get(h.rum)
                == 0.0
        );
        let restocked = h.app.world().get::<Market>(h.consumer).unwrap().entries[0].stock;
        assert!(restocked > consumer_stock + quantity - 0.01);
    }

    #[test]
    fn a_carrier_is_warned_once_as_its_deadline_nears() {
        let mut h = harbour();
        let ship = h.app.world_mut().spawn_empty().id();
        let job = h
            .app
            .world_mut()
            .spawn((
                Job {
                    good: h.rum,
                    quantity: 1,
                    origin: h.producer,
                    destination: h.consumer,
                    payment: 10,
                    deadline: DAY + 30.0,
                },
                Carrier(ship),
            ))
            .id();
        let mut alerts = Vec::new();
        run_checked(&mut h.app, 60.0, |world| {
            let messages = world.resource::<Messages<Alert>>();
            alerts.extend(messages.iter_current_update_messages().copied());
        });
        assert_eq!(
            alerts,
            vec![Alert {
                ship,
                kind: AlertKind::DeadlineNear { job }
            }]
        );
    }

    #[test]
    fn a_job_cannot_be_accepted_away_from_its_origin_or_without_room() {
        let h = harbour();
        let job = Job {
            good: h.rum,
            quantity: 30,
            origin: h.producer,
            destination: h.consumer,
            payment: 10,
            deadline: 0.0,
        };
        let market = Market {
            entries: vec![produced(h.rum, 100.0)],
        };
        let crew = Crew::new(10);
        let complement = Complement {
            min: 8,
            full: 40,
            berths: 75,
        };
        let check = |port, hold: &Hold| {
            check_acceptance(&h.goods, &job, port, &market, hold, &crew, &complement)
        };
        assert_eq!(check(h.producer, &Hold::new(40.0)), Ok(()));
        assert_eq!(check(h.consumer, &Hold::new(40.0)), Err(Refusal::NotHere));
        assert_eq!(check(h.producer, &Hold::new(20.0)), Err(Refusal::NoRoom));
    }
}
