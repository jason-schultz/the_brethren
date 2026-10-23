//! Pirate raiders: NPC sloops of the lawless nations' navies (the Brethren's,
//! in the factions data) that lie in wait on the trade
//! lanes, take the shipping of nations hostile to them, and sell their
//! plunder at the nearest port that will have them.
//!
//! A port a raider supplies this way warms to the Brethren, and the cargo
//! she took never reaches the port it was bound for. The Brethren have no
//! crown to send them what they need: their raiders are out for what their
//! own ports are short of, lying in wait more often on the lanes from the
//! ports that make it, and bringing it home where it's wanted. That's the reason
//! for piracy, and how the Brethren win ports without the player. Word
//! gets about when she puts to sea ([`Sailed`]). A wreck talked of in her
//! port draws her before any lane: she lies over it, diving, and preys on
//! whoever else comes. Her [`crate::seamanship`] keeps her fit and out of
//! the way of warships; a warship that finds her lane ends her cruise.

use bevy::ecs::query::QueryData;
use bevy::math::FloatOrd;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::allegiance::Favour;
use crate::cargo::{Hold, Purse};
use crate::clock::{DAY, HOUR, WorldClock};
use crate::combat::rounds::Dice;
use crate::combat::tactics::unfit;
use crate::combat::{EngageOrder, Timbers};
use crate::crew::{Complement, Crew};
use crate::economy::jobs::shortfall;
use crate::economy::market::{Market, Side, Trade, affordable, sell_cargo};
use crate::factions::{Colours, GroupKind, Nation, Relations, Serves};
use crate::goods::{GoodId, Goods};
use crate::hunting::{Chase, Hunter, Pursuit, Sea, pursue};
use crate::map::{Charts, Land};
use crate::merchants::{harbours, provisions_needed};
use crate::navigation::route::{NavGraph, point_along, route_length};
use crate::navigation::{Arrival, Route, SailOrder};
use crate::port::{Docked, Port, PutToSea, welcomes};
use crate::roster::{Listed, Roster};
use crate::sailing::Rigging;
use crate::schedule::SimSet;
use crate::seamanship::{AtHerBusiness, ReadyInPort, Running, Upkeep};
use crate::ship_class::ShipClass;
use crate::word::Purpose;
use crate::wrecks::WreckTalk;

/// A raider ships this share of a full fighting crew: enough to fight,
/// with men to spare for boarding.
const MANNED: f64 = 0.75;
const PURSE: u32 = 50;
/// How long a raider carouses in port between cruises.
const IN_PORT: f64 = DAY;
/// How long she lies in wait on a lane before going home empty-handed.
const CRUISE: f64 = 4.0 * DAY;
/// She goes home to sell once plunder fills this much of her hold.
const LADEN_HOME: f64 = 0.5;
/// The opening raiders put to sea at this interval.
const OPENING_STAGGER: f64 = 6.0 * HOUR;
/// A lost raider is replaced this long after.
const REPLACEMENT: f64 = 3.0 * DAY;
/// Raiders lie in wait only on lanes at least this long, meters: not in a
/// harbour between two ports in sight of each other.
const SHORTEST_LANE: f32 = 30_000.0;
/// A lane from a port that makes what her nation's ports are short of is
/// this many times as likely to be where she lies in wait.
const WANTED_LANE: f32 = 3.0;

/// A pirate captain, and what she means to do next. The Brethren's
/// raiders are their navy.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(Serves(GroupKind::Navy), Chase, Upkeep = Upkeep::Purse)]
pub struct Raider {
    pub plan: Raid,
}

/// Every plan is safe to save and resume.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Raid {
    /// Lying in port until this [`WorldClock`] time.
    InPort { until: f64 },
    /// Ordered to sea: she'll cruise until this time once she's out.
    Weighing { until: f64 },
    /// Lying in wait on a lane until this time.
    Cruising { until: f64 },
    /// Bound for a port to sell her plunder.
    Homeward,
}

#[derive(Resource, Debug)]
struct Brethren(Roster);

impl Default for Brethren {
    /// Every lawless nation's navy, from the factions data.
    fn default() -> Self {
        Self(Roster::of_fleets(
            |nation| {
                if nation.is_crown() {
                    &[]
                } else {
                    &nation.fleet().navy
                }
            },
            REPLACEMENT,
        ))
    }
}

pub struct RaidersPlugin;

impl Plugin for RaidersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Brethren>().add_systems(
            FixedUpdate,
            (raid_from_port, raid_at_sea, recruit_raiders).in_set(SimSet::Economy),
        );
    }
}

/// A raider from the roster, lying at `port` until `until`, with every gun
/// mounted.
fn raider(goods: &Goods, listed: Listed, port: Entity, at: Vec3, until: f64) -> impl Bundle {
    let class = listed.class;
    let crew = manned(&class.complement);
    let mut hold = class.empty_hold();
    hold.owned
        .add(goods.provisions, provisions_needed(crew as f64, 0.0));
    (
        Name::new(listed.name),
        Raider {
            plan: Raid::InPort { until },
        },
        class.build(crew, class.guns, hold),
        Purse(PURSE),
        Colours(listed.nation),
        Transform::from_translation(at),
        Docked { port },
    )
}

/// `nation`'s ports, in a stable order: where her raiders are fitted out.
fn havens(ports: &Query<(Entity, &Port, &Transform)>, nation: Nation) -> Vec<(Entity, Vec3)> {
    harbours(ports)
        .into_iter()
        .filter(|&(_, flag, _)| flag == nation)
        .map(|(port, _, anchorage)| (port, anchorage))
        .collect()
}

/// The raiders at the start of a game, in their nations' ports.
pub fn raiding_party(
    goods: &Goods,
    ports: &Query<(Entity, &Port, &Transform)>,
) -> Vec<impl Bundle> {
    Brethren::default()
        .0
        .ships
        .into_iter()
        .enumerate()
        .filter_map(|(i, listed)| {
            let havens = havens(ports, listed.nation);
            let (port, anchorage) = *havens.get(i % havens.len().max(1))?;
            let until = OPENING_STAGGER * (i + 1) as f64;
            Some(raider(goods, listed, port, anchorage, until))
        })
        .collect()
}

fn recruit_raiders(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    ships: Query<&Name, With<ShipClass>>,
    ports: Query<(Entity, &Port, &Transform)>,
    mut brethren: ResMut<Brethren>,
    mut commands: Commands,
) {
    let now = clock.elapsed;
    let Some((index, listed)) = brethren.0.due(ships.iter(), now) else {
        return;
    };
    let havens = havens(&ports, listed.nation);
    if let Some(&(port, anchorage)) = havens.get(index % havens.len().max(1)) {
        let until = now + IN_PORT;
        commands.spawn(raider(&goods, listed, port, anchorage, until));
    }
}

/// The plunder aboard: everything but her own provisions.
fn plunder<'a>(goods: &Goods, hold: &'a Hold) -> impl Iterator<Item = (GoodId, f64)> + 'a {
    let provisions = goods.provisions;
    hold.owned
        .iter()
        .filter(move |&(good, _)| good != provisions)
}

/// Tons of plunder aboard.
pub fn plunder_tons(goods: &Goods, hold: &Hold) -> f64 {
    plunder(goods, hold).map(|(_, tons)| tons).sum()
}

/// What the ports flying `nation`'s colours are short of, of `markets`
/// with the flags they fly: what her raiders are out for.
fn wants<'a>(
    nation: Nation,
    markets: impl IntoIterator<Item = (Nation, &'a Market)>,
) -> Vec<GoodId> {
    let mut wanted: Vec<GoodId> = markets
        .into_iter()
        .filter(|&(flag, _)| flag == nation)
        .flat_map(|(_, market)| &market.entries)
        .filter(|entry| shortfall(entry).is_some())
        .map(|entry| entry.good)
        .collect();
    wanted.sort();
    wanted.dedup();
    wanted
}

/// Whether `market` makes any of `wanted`.
fn makes_any(market: &Market, wanted: &[GoodId]) -> bool {
    market
        .entries
        .iter()
        .any(|entry| entry.is_produced() && wanted.contains(&entry.good))
}

/// Where a raider lies in wait: halfway along one of the lanes between
/// `anchorages` long enough to cruise, picked by `roll` in `[0, 1)`. Each
/// anchorage comes with whether its port makes what she's out for: lanes
/// from those are [`WANTED_LANE`] times as likely.
fn lying_in_wait(
    land: &Land,
    graph: &NavGraph,
    anchorages: &[(Vec3, bool)],
    roll: f32,
) -> Option<Vec3> {
    let lanes: Vec<(Vec3, Vec<Vec3>, f32)> = anchorages
        .iter()
        .enumerate()
        .flat_map(|(i, &from)| anchorages[i + 1..].iter().map(move |&to| (from, to)))
        .map(|((from, wanted), (to, wanted_too))| {
            let weight = if wanted || wanted_too {
                WANTED_LANE
            } else {
                1.0
            };
            (from, graph.plan(land, from, to), weight)
        })
        .filter(|(from, route, _)| route_length(*from, route) >= SHORTEST_LANE)
        .collect();
    let mut left = roll * lanes.iter().map(|&(.., weight)| weight).sum::<f32>();
    let (from, route, _) = lanes
        .iter()
        .find(|&&(.., weight)| {
            left -= weight;
            left < 0.0
        })
        .or(lanes.last())?;
    Some(point_along(*from, route, route_length(*from, route) / 2.0))
}

/// The port a raider home from a cruise at `at` with `plunder` aboard makes
/// for, of `harbours` that would have her: the nearest short of something
/// she has, or else the nearest.
fn homeward<'a>(
    harbours: impl IntoIterator<Item = (Entity, Vec3, &'a Market)>,
    at: Vec3,
    plunder: &[GoodId],
) -> Option<(Entity, Vec3)> {
    let relieves = |market: &Market| {
        plunder.iter().any(|&good| {
            market
                .entry(good)
                .is_some_and(|entry| shortfall(entry).is_some())
        })
    };
    harbours
        .into_iter()
        .min_by_key(|&(_, anchorage, market)| (!relieves(market), FloatOrd(anchorage.distance(at))))
        .map(|(port, anchorage, _)| (port, anchorage))
}

/// A raider, as her captain sees her.
#[derive(QueryData)]
#[query_data(mutable)]
struct RaiderShip {
    entity: Entity,
    raider: &'static mut Raider,
    colours: &'static Colours,
    at: &'static Transform,
    hold: &'static Hold,
    crew: &'static Crew,
    complement: &'static Complement,
    purse: &'static Purse,
    timbers: &'static Timbers,
    rigging: &'static Rigging,
    route: Option<&'static Route>,
    chase: &'static mut Chase,
    running: Has<Running>,
}

impl RaiderShipItem<'_, '_> {
    /// In no state to go looking for a fight.
    fn unfit(&self) -> bool {
        unfit(
            self.timbers.0,
            self.rigging.0,
            self.crew.count,
            self.complement.min,
        )
    }
}

/// A raider's crew at full strength, by her standards.
fn manned(complement: &Complement) -> u32 {
    (complement.full as f64 * MANNED).round() as u32
}

/// The order signing on as many men as `raider` wants to make up her crew,
/// can pay for, and the tavern has, if any.
fn sign_on(goods: &Goods, market: Option<&Market>, raider: &RaiderShipItem) -> Option<Trade> {
    let wanted = manned(raider.complement).saturating_sub(raider.crew.count);
    let entry = market?.entry(goods.sailors)?;
    let base_price = goods.get(goods.sailors).base_price;
    let hired = affordable(entry, base_price, wanted, raider.purse.0);
    (hired > 0).then_some(Trade {
        ship: raider.entity,
        good: goods.sailors,
        quantity: hired,
        side: Side::Buy,
    })
}

/// Where a raider of `nation` lies in wait, on the charts: on a lane
/// between ports, picked by `roll` in `[0, 1)`, likelier from the ports
/// that make what hers are short of.
fn ground(charts: &Charts, markets: &Query<&Market>, nation: Nation, roll: f32) -> Option<Vec3> {
    let harbours: Vec<_> = harbours(&charts.ports)
        .into_iter()
        .filter_map(|(port, flag, anchorage)| Some((flag, anchorage, markets.get(port).ok()?)))
        .collect();
    let wanted = wants(
        nation,
        harbours.iter().map(|&(flag, _, market)| (flag, market)),
    );
    let anchorages: Vec<(Vec3, bool)> = harbours
        .iter()
        .map(|&(_, anchorage, market)| (anchorage, makes_any(market, &wanted)))
        .collect();
    lying_in_wait(&charts.land, &charts.graph, &anchorages, roll)
}

/// A raider in port: sells her plunder, victuals, and signs on men when
/// she comes in, and puts to sea again when she's had her time ashore.
fn raid_from_port(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    mut dice: ResMut<Dice>,
    charts: Charts,
    markets: Query<&Market>,
    talk: WreckTalk,
    mut raiders: Query<(RaiderShip, &Docked), ReadyInPort>,
    mut trades: MessageWriter<Trade>,
    mut put_to_sea: PutToSea,
) {
    let now = clock.elapsed;
    for (mut raider, &Docked { port }) in &mut raiders {
        let ship = raider.entity;
        let market = markets.get(port).ok();
        raider.raider.plan = match raider.raider.plan {
            Raid::InPort { until } if now >= until => {
                // A wreck talked of in port is easier pickings than a
                // lane.
                let wreck = talk
                    .known_at(port, raider.at.translation)
                    .first()
                    .map(|&(_, lies)| lies);
                let nation = raider.colours.0;
                match wreck.or_else(|| ground(&charts, &markets, nation, dice.roll())) {
                    Some(ground) => {
                        put_to_sea.sail(
                            ship,
                            ground,
                            Arrival::HeaveTo,
                            Some((port, Purpose::Cruising)),
                        );
                        Raid::Weighing {
                            until: now + CRUISE,
                        }
                    }
                    None => raider.raider.plan,
                }
            }
            Raid::InPort { .. } | Raid::Weighing { .. } => raider.raider.plan,
            Raid::Cruising { .. } | Raid::Homeward => {
                // In from a cruise: sell what the market deals in, victual
                // for the next, and make up her crew.
                if let Some(market) = market {
                    trades.write_batch(sell_cargo(&goods, ship, raider.hold, market));
                }
                let tons = provisions_needed(
                    raider.crew.count as f64,
                    raider.hold.owned.get(goods.provisions),
                );
                if tons > 0.0 {
                    trades.write(Trade {
                        ship,
                        good: goods.provisions,
                        quantity: tons as u32,
                        side: Side::Buy,
                    });
                }
                trades.write_batch(sign_on(&goods, market, &raider));
                Raid::InPort {
                    until: now + IN_PORT,
                }
            }
        };
    }
}

/// A raider at sea: lies in wait on her lane and takes what comes, until
/// her cruise is up, her hold is full, she's in no state to fight, or a
/// warship has found her. Then she makes for port, running from any
/// warship first.
fn raid_at_sea(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    relations: Res<Relations>,
    towns: Query<(Entity, &Port, &Favour, &Transform, &Market)>,
    mut raiders: Query<RaiderShip, (AtHerBusiness, Without<Docked>)>,
    sea: Sea,
    mut engagements: MessageWriter<EngageOrder>,
    mut orders: MessageWriter<SailOrder>,
) {
    let now = clock.elapsed;
    for mut raider in &mut raiders {
        let route = raider.route;
        let (ship, at, Colours(nation)) = (raider.entity, raider.at.translation, *raider.colours);
        if let Raid::Weighing { until } = raider.raider.plan {
            raider.raider.plan = Raid::Cruising { until };
        }
        // A warship that would hunt her has found her lane: she gives it
        // up, rather than run and come back to it, time after time. Her
        // seamanship has the helm until she's clear.
        if raider.running {
            raider.raider.plan = Raid::Homeward;
            continue;
        }
        let heading_for = route.and_then(|route| route.waypoints.back().copied());
        let Raid::Cruising { until } = raider.raider.plan else {
            // Homeward, or adrift after a fight: make for port.
            let plunder: Vec<GoodId> = plunder(&goods, raider.hold).map(|(good, _)| good).collect();
            if route.is_none()
                && let Some((port, anchorage)) = homeward(
                    towns
                        .iter()
                        .filter(|(_, port, favour, ..)| welcomes(port, favour, nation, &relations))
                        .map(|(entity, _, _, anchorage, market)| {
                            (entity, anchorage.translation, market)
                        }),
                    at,
                    &plunder,
                )
            {
                orders.write(SailOrder {
                    ship,
                    destination: anchorage,
                    on_arrival: Arrival::Dock(port),
                });
            }
            continue;
        };
        let laden = plunder_tons(&goods, raider.hold) >= raider.hold.capacity_tons * LADEN_HOME;
        // Unfit with no yard to have her, she still sells what she has.
        if now >= until || laden || raider.unfit() {
            raider.raider.plan = Raid::Homeward;
            continue;
        }
        let quarry = sea.nearest_quarry(Hunter::Raider(nation), at, raider.chase.letting_be(now));
        if raider
            .chase
            .keep_after(quarry.map(|(target, _)| target), now, raider.crew.morale)
            && let Some((target, quarry)) = quarry
        {
            match pursue(at, heading_for, quarry) {
                Pursuit::Engage => {
                    engagements.write(EngageOrder { ship, target });
                }
                Pursuit::SailTo(quarry) => {
                    orders.write(SailOrder {
                        ship,
                        destination: quarry,
                        on_arrival: Arrival::HeaveTo,
                    });
                }
                Pursuit::Hold => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::tactics::battered;
    use crate::economy::market::MarketEntry;
    use crate::merchants::Merchant;
    use crate::new_game::new_game;
    use crate::test_support::run_until;
    use crate::time_scale::MAX_STEP;

    #[test]
    fn plunder_is_everything_but_her_own_provisions() {
        let goods = Goods::standard();
        let mut hold = Hold::new(30.0);
        hold.owned.add(goods.provisions, 3.0);
        hold.owned.add(goods.id("rum").unwrap(), 10.0);
        assert_eq!(plunder_tons(&goods, &hold), 10.0);
    }

    #[test]
    fn raiders_lie_in_wait_out_at_sea_not_in_harbour() {
        let land = Land::default();
        let graph = NavGraph::default();
        let near = [(Vec3::ZERO, false), (Vec3::new(5_000.0, 0.0, 0.0), false)];
        assert_eq!(lying_in_wait(&land, &graph, &near, 0.5), None);
        let far = [
            (Vec3::ZERO, false),
            (Vec3::new(5_000.0, 0.0, 0.0), false),
            (Vec3::new(0.0, 0.0, 80_000.0), false),
        ];
        for roll in [0.0, 0.5, 0.99] {
            let ground = lying_in_wait(&land, &graph, &far, roll).unwrap();
            assert!(ground.z > 30_000.0, "{ground}");
        }
    }

    #[test]
    fn raiders_are_out_for_what_their_own_ports_are_short_of() {
        let goods = Goods::standard();
        let [munitions, rum, salt] = ["munitions", "rum", "salt"].map(|key| goods.id(key).unwrap());
        let entry = |good, stock, per_day| MarketEntry {
            good,
            stock,
            target: 40.0,
            per_day,
            short_for: 0.0,
        };
        let nassau = Market {
            entries: vec![
                entry(munitions, 0.0, -1.0),
                entry(rum, 40.0, -1.0),
                entry(salt, 0.0, 1.0),
            ],
        };
        let port_royal = Market {
            entries: vec![entry(rum, 0.0, -1.0)],
        };
        let markets = [(Nation::PIRATES, &nassau), (Nation::ENGLAND, &port_royal)];
        assert_eq!(wants(Nation::PIRATES, markets), [munitions]);
        let kingston = Market {
            entries: vec![entry(munitions, 20.0, 1.0)],
        };
        assert!(makes_any(&kingston, &[munitions]));
        assert!(
            !makes_any(&nassau, &[munitions]),
            "it needs them, not makes them"
        );
    }

    #[test]
    fn raiders_lie_in_wait_likelier_on_the_lanes_from_where_its_made() {
        let land = Land::default();
        let graph = NavGraph::default();
        // The first makes what she's out for: two of the three lanes run
        // from it.
        let ports = [
            (Vec3::ZERO, true),
            (Vec3::new(80_000.0, 0.0, 0.0), false),
            (Vec3::new(0.0, 0.0, 80_000.0), false),
        ];
        let rolls = 1_000;
        let from_it = (0..rolls)
            .filter_map(|n| lying_in_wait(&land, &graph, &ports, n as f32 / rolls as f32))
            .filter(|ground| ground.x < 1.0 || ground.z < 1.0)
            .count();
        // Of seven shares, six fall on its lanes: 3 + 3 against 1.
        let expected = rolls * 6 / 7;
        assert!(from_it.abs_diff(expected) <= 2, "{from_it} of {rolls}");
    }

    #[test]
    fn a_raider_brings_home_what_a_port_is_short_of_though_another_is_nearer() {
        let goods = Goods::standard();
        let [munitions, sugar] = ["munitions", "sugar"].map(|key| goods.id(key).unwrap());
        let short = Market {
            entries: vec![MarketEntry {
                good: munitions,
                stock: 0.0,
                target: 40.0,
                per_day: -1.0,
                short_for: 0.0,
            }],
        };
        let wanting_nothing = Market::default();
        let [near, far] = [Entity::from_raw_u32(1), Entity::from_raw_u32(2)].map(Option::unwrap);
        let (near_at, far_at) = (Vec3::X * 1_000.0, Vec3::X * 50_000.0);
        let harbours = [(near, near_at, &wanting_nothing), (far, far_at, &short)];
        assert_eq!(
            homeward(harbours, Vec3::ZERO, &[munitions]),
            Some((far, far_at))
        );
        assert_eq!(
            homeward(harbours, Vec3::ZERO, &[sugar]),
            Some((near, near_at)),
            "nobody's short of what she has: the nearest"
        );
    }

    /// A new world at full time compression, and its first raider.
    fn first_raider() -> (App, Entity) {
        let (mut app, _) = new_game();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        app.update();
        let raider = app
            .world_mut()
            .query_filtered::<Entity, With<Raider>>()
            .iter(app.world())
            .min()
            .unwrap();
        (app, raider)
    }

    #[test]
    fn a_battered_raider_lies_in_port_until_she_is_refitted() {
        let (mut app, raider) = first_raider();
        app.world_mut().entity_mut(raider).insert((
            Timbers(0.1),
            Raider {
                plan: Raid::InPort { until: 0.0 },
            },
        ));
        run_until(&mut app, (0.5 * DAY) as f32, |_| false);
        assert!(
            app.world().get::<Docked>(raider).is_some(),
            "she sailed battered"
        );
        let sailed = run_until(&mut app, (3.0 * DAY) as f32, |world| {
            world.get::<Docked>(raider).is_none()
        });
        assert!(sailed, "she never sailed");
        assert!(!battered(
            app.world().get::<Timbers>(raider).unwrap().0,
            1.0
        ));
    }

    #[test]
    fn a_short_handed_raider_signs_on_men_before_she_sails() {
        let (mut app, raider) = first_raider();
        let complement = *app.world().get::<Complement>(raider).unwrap();
        app.world_mut().entity_mut(raider).insert((
            Crew::new(2),
            Purse(10_000),
            Raider {
                plan: Raid::InPort { until: 0.0 },
            },
        ));
        let sailed = run_until(&mut app, (3.0 * DAY) as f32, |world| {
            world.get::<Docked>(raider).is_none()
        });
        assert!(sailed, "she never sailed");
        let crew = app.world().get::<Crew>(raider).unwrap().count;
        assert!(crew >= complement.min, "sailed with {crew} men");
    }

    #[test]
    fn raiders_put_out_from_nassau_and_take_merchants_on_the_lanes() {
        let (mut app, _) = new_game();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let raiders = |app: &mut App| {
            app.world_mut()
                .query_filtered::<Entity, With<Raider>>()
                .iter(app.world())
                .count()
        };
        assert_eq!(raiders(&mut app), Brethren::default().0.ships.len());
        let merchants = |world: &World| {
            world
                .iter_entities()
                .filter(|entity| entity.contains::<Merchant>())
                .count()
        };
        let fleet = merchants(app.world());
        // A merchant taken is plundered and burned, so the fleet shrinks
        // until her replacement is launched.
        let took_one = run_until(&mut app, 12.0 * DAY as f32, |world| {
            merchants(world) < fleet
        });
        assert!(took_one, "no merchant was taken in twelve days");
    }
}
