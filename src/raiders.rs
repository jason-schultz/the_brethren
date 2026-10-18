//! Pirate raiders: NPC sloops of the Brethren that lie in wait on the trade
//! lanes, take the shipping of nations hostile to them, and sell their
//! plunder at the nearest port that will have them.
//!
//! A port a raider supplies this way warms to the Brethren, and the cargo
//! she took never reaches the port it was bound for. That's the reason
//! for piracy, and how the Brethren win ports without the player.

use bevy::math::FloatOrd;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::allegiance::Favour;
use crate::cargo::{Hold, Purse};
use crate::clock::{DAY, HOUR, WorldClock};
use crate::combat::rounds::Dice;
use crate::combat::{EngageOrder, Engaged, Struck};
use crate::crew::Crew;
use crate::economy::market::{Market, Side, Trade};
use crate::factions::{Colours, Nation, Relations};
use crate::goods::Goods;
use crate::hunting::{Hunter, Pursuit, Sea, pursue};
use crate::map::Land;
use crate::merchants::{harbours, provisions_needed};
use crate::navigation::route::{NavGraph, point_along, route_length};
use crate::navigation::{Arrival, Route, SailOrder};
use crate::port::{CastOff, Docked, Port, welcomes};
use crate::roster::Roster;
use crate::schedule::SimSet;
use crate::ship_class::ShipClass;

/// Hornigold's, Teach's, and Bonnet's sloops.
const NAMES: [&str; 3] = ["Ranger", "Adventure", "Revenge"];
const NATION: Nation = Nation::Pirates;
/// A crew to fight with and spare men for boarding.
const CREW: u32 = 30;
const GUNS: u32 = ShipClass::SLOOP.guns;
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

/// A pirate captain, and what she means to do next.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
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
    fn default() -> Self {
        Self(Roster::new(&NAMES, REPLACEMENT))
    }
}

pub struct RaidersPlugin;

impl Plugin for RaidersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Brethren>()
            .add_systems(FixedUpdate, (raid, recruit_raiders).in_set(SimSet::Economy));
    }
}

/// A raider called `name`, lying at `port` until `until`.
fn raider(goods: &Goods, name: &str, port: Entity, at: Vec3, until: f64) -> impl Bundle {
    let class = ShipClass::SLOOP;
    let mut hold = class.empty_hold();
    hold.owned
        .add(goods.provisions, provisions_needed(CREW as f64, 0.0));
    (
        Name::new(name.to_string()),
        Raider {
            plan: Raid::InPort { until },
        },
        class.build(CREW, GUNS, hold),
        Purse(PURSE),
        Colours(NATION),
        Transform::from_translation(at),
        Docked { port },
    )
}

/// The Brethren's ports, in a stable order: where raiders are fitted out.
fn havens(ports: &Query<(Entity, &Port, &Transform)>) -> Vec<(Entity, Vec3)> {
    harbours(ports)
        .into_iter()
        .filter(|&(_, flag, _)| flag == NATION)
        .map(|(port, _, anchorage)| (port, anchorage))
        .collect()
}

/// The raiders at the start of a game, in the Brethren's ports.
pub fn raiding_party(
    goods: &Goods,
    ports: &Query<(Entity, &Port, &Transform)>,
) -> Vec<impl Bundle> {
    let havens = havens(ports);
    NAMES
        .iter()
        .enumerate()
        .filter_map(|(i, name)| {
            let (port, anchorage) = *havens.get(i % havens.len().max(1))?;
            let until = OPENING_STAGGER * (i + 1) as f64;
            Some(raider(goods, name, port, anchorage, until))
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
    let Some((index, name)) = brethren.0.due(ships.iter(), now) else {
        return;
    };
    let havens = havens(&ports);
    if let Some(&(port, anchorage)) = havens.get(index % havens.len().max(1)) {
        commands.spawn(raider(&goods, name, port, anchorage, now + IN_PORT));
    }
}

/// Tons of plunder aboard: everything but her own provisions.
pub fn plunder_tons(goods: &Goods, hold: &Hold) -> f64 {
    hold.owned
        .iter()
        .filter(|&(good, _)| good != goods.provisions)
        .map(|(_, tons)| tons)
        .sum()
}

/// Where a raider lies in wait: halfway along one of the lanes between
/// `anchorages` long enough to cruise, picked by `roll` in `[0, 1)`.
fn lying_in_wait(land: &Land, graph: &NavGraph, anchorages: &[Vec3], roll: f32) -> Option<Vec3> {
    let lanes: Vec<(Vec3, Vec<Vec3>)> = anchorages
        .iter()
        .enumerate()
        .flat_map(|(i, &from)| anchorages[i + 1..].iter().map(move |&to| (from, to)))
        .map(|(from, to)| (from, graph.plan(land, from, to)))
        .filter(|(from, route)| route_length(*from, route) >= SHORTEST_LANE)
        .collect();
    let (from, route) = lanes.get((roll * lanes.len() as f32) as usize % lanes.len().max(1))?;
    Some(point_along(*from, route, route_length(*from, route) / 2.0))
}

fn raid(
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    land: Res<Land>,
    graph: Res<NavGraph>,
    relations: Res<Relations>,
    mut dice: ResMut<Dice>,
    ports: Query<(Entity, &Port, &Transform)>,
    towns: Query<(Entity, &Port, &Favour, &Transform)>,
    markets: Query<&Market>,
    mut raiders: Query<
        (
            Entity,
            &mut Raider,
            &Colours,
            Option<&Docked>,
            Option<&Route>,
            &Transform,
            &Hold,
            &Crew,
        ),
        (Without<Engaged>, Without<Struck>),
    >,
    sea: Sea,
    mut engagements: MessageWriter<EngageOrder>,
    mut cast_offs: MessageWriter<CastOff>,
    mut orders: MessageWriter<SailOrder>,
    mut trades: MessageWriter<Trade>,
) {
    let now = clock.elapsed;
    let anchorages: Vec<Vec3> = harbours(&ports)
        .into_iter()
        .map(|(.., anchorage)| anchorage)
        .collect();
    for (ship, mut raider, &Colours(nation), docked, route, at, hold, crew) in &mut raiders {
        if let Some(&Docked { port }) = docked {
            raider.plan = match raider.plan {
                Raid::InPort { until } if now >= until => {
                    match lying_in_wait(&land, &graph, &anchorages, dice.roll()) {
                        Some(ground) => {
                            cast_offs.write(CastOff { ship });
                            orders.write(SailOrder {
                                ship,
                                destination: ground,
                                on_arrival: Arrival::HeaveTo,
                            });
                            Raid::Weighing {
                                until: now + CRUISE,
                            }
                        }
                        None => raider.plan,
                    }
                }
                Raid::InPort { .. } | Raid::Weighing { .. } => raider.plan,
                Raid::Cruising { .. } | Raid::Homeward => {
                    // In from a cruise: sell what the market deals in, and
                    // victual for the next.
                    if let Ok(market) = markets.get(port) {
                        for (good, tons) in hold.owned.iter() {
                            if good != goods.provisions && market.entry(good).is_some() {
                                trades.write(Trade {
                                    ship,
                                    good,
                                    quantity: tons.floor() as u32,
                                    side: Side::Sell,
                                });
                            }
                        }
                    }
                    let tons =
                        provisions_needed(crew.count as f64, hold.owned.get(goods.provisions));
                    if tons > 0.0 {
                        trades.write(Trade {
                            ship,
                            good: goods.provisions,
                            quantity: tons as u32,
                            side: Side::Buy,
                        });
                    }
                    Raid::InPort {
                        until: now + IN_PORT,
                    }
                }
            };
            continue;
        }
        if let Raid::Weighing { until } = raider.plan {
            raider.plan = Raid::Cruising { until };
        }
        let Raid::Cruising { until } = raider.plan else {
            // Homeward, or adrift after a fight: make for port.
            if route.is_none()
                && let Some((port, anchorage)) = towns
                    .iter()
                    .filter(|(_, port, favour, _)| welcomes(port, favour, nation, &relations))
                    .map(|(entity, .., anchorage)| (entity, anchorage.translation))
                    .min_by_key(|(_, anchorage)| FloatOrd(anchorage.distance(at.translation)))
            {
                orders.write(SailOrder {
                    ship,
                    destination: anchorage,
                    on_arrival: Arrival::Dock(port),
                });
            }
            continue;
        };
        let laden = plunder_tons(&goods, hold) >= hold.capacity_tons * LADEN_HOME;
        if now >= until || laden {
            raider.plan = Raid::Homeward;
            continue;
        }
        if let Some((target, quarry)) = sea.nearest_quarry(Hunter::Raider(nation), at.translation) {
            let heading_for = route.and_then(|route| route.waypoints.back().copied());
            match pursue(at.translation, heading_for, quarry) {
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
        let near = [Vec3::ZERO, Vec3::new(5_000.0, 0.0, 0.0)];
        assert_eq!(lying_in_wait(&land, &graph, &near, 0.5), None);
        let far = [
            Vec3::ZERO,
            Vec3::new(5_000.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 80_000.0),
        ];
        for roll in [0.0, 0.5, 0.99] {
            let ground = lying_in_wait(&land, &graph, &far, roll).unwrap();
            assert!(ground.z > 30_000.0, "{ground}");
        }
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
        assert_eq!(raiders(&mut app), NAMES.len());
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
