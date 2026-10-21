//! Allegiance: how each port regards each nation, and ports changing hands.
//!
//! A port's [`Favour`] for its owner settles toward a resting level, and
//! sinks while the things it needs run short. Whoever brings what it
//! needs, by contract or at market, wins favour for the colours she flies,
//! and the other nations lose a little. A port whose favour for its owner
//! sinks low enough is restless: its garrison deserts, and it will go over
//! to the nation it favours more, if a ship of that nation lies in its
//! harbour and none of its owner's warships stand guard. The weaker the
//! garrison, the sooner.

use bevy::prelude::*;

use crate::alert::{Alert, AlertKind};
use crate::clock::DAY;
use crate::combat::rounds::Dice;
use crate::economy::jobs::shortfall;
use crate::economy::market::{Market, Supplied};
use crate::factions::{Colours, Deed, DeedKind, Group, GroupKind, Nation, nation_count, nations};
use crate::goods::{GoodId, Goods};
use crate::loyalty::chance_in;
use crate::patrols::Patrol;
use crate::player::Player;
use crate::port::{Docked, Port};
use crate::schedule::{FrameSet, SimSet};

/// Favour runs between these.
const LEAST: f32 = 0.0;
const MOST: f32 = 100.0;
/// Where a port's favour settles: for the nation that holds it...
pub const OWNER_REST: f32 = 55.0;
/// ...and for every other.
pub const OTHER_REST: f32 = 25.0;
/// The fraction of the way to its resting level favour settles each day.
const SETTLING_PER_DAY: f32 = 0.05;
/// What its owner loses a day while everything a port needs runs short;
/// less while only some of it does.
const NEGLECT_PER_DAY: f32 = 2.5;
/// Favour won by filling a port's whole shortage of one good. The other
/// nations lose half as much.
const SUPPLY_FAVOUR: f32 = 25.0;
/// Below this favour for its owner, a port is restless.
pub const RESTLESS: f32 = 25.0;
/// A port deals with a nation it favours this much, whatever its owner
/// thinks of her.
pub const DEALS_WITH: f32 = 50.0;
/// A restless port's garrison deserts at this many men a day.
const DESERTION_PER_DAY: f64 = 2.0;
/// A restless port with no garrison at all rises with this chance a day,
/// with a ship of the nation it favours in harbour. A full garrison holds
/// it down.
const RISING_PER_DAY: f32 = 0.4;
/// How a port without soldiers in its market counts its garrison.
const NO_GARRISON: f32 = 0.5;
/// A warship of its owner this close to a port's anchorage, meters, keeps
/// it from rising.
const GUARD_RANGE: f32 = 1_500.0;
/// When a port goes over, its new owner starts with at least this much
/// favour, and its old one with at most this much.
const NEW_OWNER: f32 = 60.0;
const OLD_OWNER: f32 = 10.0;

/// How a port regards each nation, from 0 to 100.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Favour(Vec<f32>);

impl Default for Favour {
    /// No nation's port in particular.
    fn default() -> Self {
        Self(vec![OTHER_REST; nation_count()])
    }
}

impl Favour {
    /// A port held by `owner`, as it would settle.
    pub fn held_by(owner: Nation) -> Self {
        let mut favour = Self::default();
        favour.set(owner, OWNER_REST);
        favour
    }

    pub fn of(&self, nation: Nation) -> f32 {
        self.0[nation.index()]
    }

    pub fn set(&mut self, nation: Nation, value: f32) {
        self.0[nation.index()] = value.clamp(LEAST, MOST);
    }

    pub fn change(&mut self, nation: Nation, by: f32) {
        self.set(nation, self.of(nation) + by);
    }

    /// Over `days`, settles toward resting levels, while `neglect` (the
    /// fraction of what it needs that runs short) costs its `owner`.
    pub fn settle(&mut self, owner: Nation, neglect: f32, days: f32) {
        let pull = (SETTLING_PER_DAY * days).min(1.0);
        for nation in nations() {
            let rest = if nation == owner {
                OWNER_REST
            } else {
                OTHER_REST
            };
            self.change(nation, (rest - self.of(nation)) * pull);
        }
        self.change(owner, -NEGLECT_PER_DAY * neglect * days);
    }

    /// `by` filled `share` of a shortage.
    pub fn supplied(&mut self, by: Nation, share: f32) {
        let gain = supply_favour(share);
        for nation in nations() {
            self.change(nation, if nation == by { gain } else { -gain / 2.0 });
        }
    }

    /// Whether a port held by `owner` has turned against it.
    pub fn restless(&self, owner: Nation) -> bool {
        self.of(owner) < RESTLESS
    }

    /// The nation a restless port held by `owner` would go over to: the
    /// one it favours most, if more than its owner.
    pub fn pretender(&self, owner: Nation) -> Option<Nation> {
        nations()
            .filter(|&nation| nation != owner && self.of(nation) > self.of(owner))
            .max_by(|&a, &b| self.of(a).total_cmp(&self.of(b)))
            .filter(|_| self.restless(owner))
    }

    /// Gone over from `from` to `to`.
    fn go_over(&mut self, from: Nation, to: Nation) {
        self.set(to, self.of(to).max(NEW_OWNER));
        self.set(from, self.of(from).min(OLD_OWNER));
    }
}

/// The favour won by filling `share` of a port's shortage of one good.
fn supply_favour(share: f32) -> f32 {
    SUPPLY_FAVOUR * share
}

/// The fraction of a port's needs running short.
pub fn neglect(market: &Market) -> f32 {
    let needs = market.entries.iter().filter(|entry| entry.is_needed());
    let (short, all) = needs.fold((0, 0), |(short, all), entry| {
        (short + shortfall(entry).is_some() as u32, all + 1)
    });
    if all == 0 {
        0.0
    } else {
        short as f32 / all as f32
    }
}

/// A port's garrison, from 0 (none) to 1 (at full strength or more).
pub fn garrison(goods: &Goods, market: &Market) -> f32 {
    market
        .entry(goods.soldiers)
        .map_or(NO_GARRISON, |entry| manned(entry.stock, entry.target))
}

/// A garrison of `soldiers` against a `target`, from 0 to 1.
fn manned(soldiers: f64, target: f64) -> f32 {
    (soldiers / target).min(1.0) as f32
}

/// The daily chance a restless port rises, given its garrison.
pub fn rising_per_day(garrison: f32) -> f32 {
    RISING_PER_DAY * (1.0 - garrison).max(0.0)
}

/// What hangs on a cargo reaching a port that needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stakes {
    /// The share of the port's shortage it fills, as a fraction of target.
    pub fills: f64,
    /// The favour that wins there for the colours she flies.
    pub favour: f32,
    /// Whether the port is short of it now, so that while it goes without,
    /// its owner goes on losing favour there.
    pub short: bool,
    /// Whether the port is restless now.
    pub restless: bool,
    /// For soldiers bound for a restless port: its chance a day of rising
    /// now, and once they're landed.
    pub rising: Option<(f32, f32)>,
}

/// What hangs on `quantity` of `good` reaching a port held by `owner`:
/// nothing, if it isn't short of them.
pub fn stakes(
    goods: &Goods,
    good: GoodId,
    quantity: u32,
    owner: Nation,
    market: &Market,
    favour: &Favour,
) -> Option<Stakes> {
    let entry = market.entry(good)?;
    let fills = entry.gap_filled(quantity as f64);
    if fills <= 0.0 {
        return None;
    }
    let restless = favour.restless(owner);
    let rising = (restless && good == goods.soldiers).then(|| {
        let now = manned(entry.stock, entry.target);
        let landed = manned(entry.stock + quantity as f64, entry.target);
        (rising_per_day(now), rising_per_day(landed))
    });
    Some(Stakes {
        fills,
        favour: supply_favour(fills as f32),
        short: shortfall(entry).is_some(),
        restless,
        rising,
    })
}

pub struct AllegiancePlugin;

impl Plugin for AllegiancePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, take_supplies.after(FrameSet::Orders))
            .add_systems(
                FixedUpdate,
                (keep_allegiance, rise).chain().in_set(SimSet::Economy),
            );
    }
}

/// Whoever supplies a port wins its favour for the colours she flies.
fn take_supplies(
    mut supplies: MessageReader<Supplied>,
    ships: Query<&Colours>,
    mut ports: Query<&mut Favour>,
) {
    for supply in supplies.read() {
        if supply.share > 0.0
            && let Ok(&Colours(nation)) = ships.get(supply.ship)
            && let Ok(mut favour) = ports.get_mut(supply.port)
        {
            favour.supplied(nation, supply.share as f32);
        }
    }
}

/// Favour settles and neglect tells, and a restless port's garrison
/// deserts. The player hears when a port turns restless.
fn keep_allegiance(
    time: Res<Time>,
    goods: Res<Goods>,
    mut ports: Query<(Entity, &Port, &mut Favour, &mut Market)>,
    player: Query<Entity, With<Player>>,
    mut alerts: MessageWriter<Alert>,
) {
    let days = time.delta_secs_f64() / DAY;
    for (port, held, mut favour, mut market) in &mut ports {
        let was_restless = favour.restless(held.flag);
        favour.settle(held.flag, neglect(&market), days as f32);
        if !favour.restless(held.flag) {
            continue;
        }
        if let Some(soldiers) = market.entry_mut(goods.soldiers) {
            soldiers.stock = (soldiers.stock - DESERTION_PER_DAY * days).max(0.0);
        }
        if !was_restless && let Ok(ship) = player.single() {
            alerts.write(Alert {
                ship,
                kind: AlertKind::Restless { port },
            });
        }
    }
}

/// A restless port rises for the nation it favours, if one of her ships
/// lies in its harbour, the player's first, and none of its owner's
/// warships stand guard. The ship that was there gets the credit.
fn rise(
    time: Res<Time>,
    goods: Res<Goods>,
    mut dice: ResMut<Dice>,
    mut ports: Query<(Entity, &mut Port, &mut Favour, &Market, &Transform)>,
    docked: Query<(Entity, &Docked, &Colours, Has<Player>)>,
    warships: Query<(&Colours, &Transform), With<Patrol>>,
    player: Query<Entity, With<Player>>,
    mut deeds: MessageWriter<Deed>,
    mut alerts: MessageWriter<Alert>,
) {
    let days = (time.delta_secs_f64() / DAY) as f32;
    for (port, mut held, mut favour, market, anchorage) in &mut ports {
        let owner = held.flag;
        let Some(pretender) = favour.pretender(owner) else {
            continue;
        };
        let guarded = warships.iter().any(|(colours, at)| {
            colours.0 == owner && at.translation.distance(anchorage.translation) <= GUARD_RANGE
        });
        let present = docked
            .iter()
            .filter(|(_, docked, colours, _)| docked.port == port && colours.0 == pretender)
            .max_by_key(|&(.., is_player)| is_player)
            .map(|(ship, ..)| ship);
        let Some(doer) = present.filter(|_| !guarded) else {
            continue;
        };
        if dice.roll() >= chance_in(rising_per_day(garrison(&goods, market)), days) {
            continue;
        }
        held.flag = pretender;
        favour.go_over(owner, pretender);
        deeds.write(Deed {
            doer,
            kind: DeedKind::RaisedPort,
            group: Group {
                nation: owner,
                kind: GroupKind::Governor,
            },
            about: None,
        });
        if let Ok(ship) = player.single() {
            alerts.write(Alert {
                ship,
                kind: AlertKind::WentOver {
                    port,
                    from: owner,
                    to: pretender,
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::market::MarketEntry;
    use crate::port::Location;
    use crate::test_support::{run_until, sim_app};
    use crate::time_scale::MAX_STEP;

    #[test]
    fn a_neglected_port_turns_restless_and_a_cared_for_one_settles_back() {
        let mut favour = Favour::held_by(Nation::ENGLAND);
        for _ in 0..60 {
            favour.settle(Nation::ENGLAND, 1.0, 1.0);
        }
        assert!(favour.restless(Nation::ENGLAND), "{favour:?}");
        for _ in 0..60 {
            favour.settle(Nation::ENGLAND, 0.0, 1.0);
        }
        assert!(!favour.restless(Nation::ENGLAND), "{favour:?}");
        let mut half = Favour::held_by(Nation::ENGLAND);
        for _ in 0..200 {
            half.settle(Nation::ENGLAND, 0.4, 1.0);
        }
        assert!(
            !half.restless(Nation::ENGLAND),
            "a port short of a few things grumbles but holds: {half:?}"
        );
    }

    #[test]
    fn supplying_a_port_wins_its_favour_at_the_others_expense() {
        let mut favour = Favour::held_by(Nation::ENGLAND);
        favour.supplied(Nation::PIRATES, 0.4);
        assert_eq!(favour.of(Nation::PIRATES), OTHER_REST + 10.0);
        assert_eq!(favour.of(Nation::ENGLAND), OWNER_REST - 5.0);
        assert_eq!(favour.of(Nation::SPAIN), OTHER_REST - 5.0);
    }

    #[test]
    fn a_restless_port_would_go_over_to_the_nation_it_favours_more() {
        let mut favour = Favour::held_by(Nation::ENGLAND);
        favour.set(Nation::PIRATES, 40.0);
        assert_eq!(favour.pretender(Nation::ENGLAND), None, "not restless");
        favour.set(Nation::ENGLAND, 20.0);
        assert_eq!(favour.pretender(Nation::ENGLAND), Some(Nation::PIRATES));
        favour.set(Nation::SPAIN, 45.0);
        assert_eq!(favour.pretender(Nation::ENGLAND), Some(Nation::SPAIN));
        favour.go_over(Nation::ENGLAND, Nation::SPAIN);
        assert!(!favour.restless(Nation::SPAIN));
        assert!(favour.of(Nation::ENGLAND) <= OLD_OWNER);
    }

    #[test]
    fn whats_at_stake_is_what_delivery_would_really_do() {
        let goods = Goods::standard();
        let rum = goods.id("rum").unwrap();
        let entry = |good, stock| MarketEntry {
            good,
            stock,
            target: 40.0,
            per_day: -1.0,
        };
        let market = Market {
            entries: vec![entry(rum, 10.0), entry(goods.soldiers, 0.0)],
        };
        let mut favour = Favour::held_by(Nation::SPAIN);
        let rum_stakes = stakes(&goods, rum, 20, Nation::SPAIN, &market, &favour).unwrap();
        assert_eq!(rum_stakes.fills, 0.5);
        let mut supplied = favour.clone();
        supplied.supplied(Nation::ENGLAND, rum_stakes.fills as f32);
        assert_eq!(
            supplied.of(Nation::ENGLAND),
            favour.of(Nation::ENGLAND) + rum_stakes.favour,
            "the favour at stake is the favour delivery wins"
        );
        assert!(rum_stakes.short && rum_stakes.rising.is_none());

        favour.set(Nation::SPAIN, 10.0);
        let soldiers = stakes(&goods, goods.soldiers, 40, Nation::SPAIN, &market, &favour);
        let (now, landed) = soldiers.unwrap().rising.unwrap();
        assert_eq!((now, landed), (rising_per_day(0.0), 0.0));

        let full = Market {
            entries: vec![entry(rum, 40.0)],
        };
        assert_eq!(stakes(&goods, rum, 20, Nation::SPAIN, &full, &favour), None);
        assert_eq!(
            stakes(
                &goods,
                goods.provisions,
                20,
                Nation::SPAIN,
                &market,
                &favour
            ),
            None,
            "it doesn't deal in them"
        );
    }

    #[test]
    fn a_full_garrison_holds_a_port_down() {
        assert_eq!(rising_per_day(1.0), 0.0);
        assert!(rising_per_day(0.0) > rising_per_day(0.5));
    }

    /// A restless English port with a weak garrison, short of rum, and a
    /// pirate sloop in harbour.
    fn restless_port(app: &mut App) -> (Entity, Entity) {
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let goods = app.world().resource::<Goods>().clone();
        let mut favour = Favour::held_by(Nation::ENGLAND);
        favour.set(Nation::ENGLAND, 10.0);
        favour.set(Nation::PIRATES, 40.0);
        let port = app
            .world_mut()
            .spawn((
                Port {
                    flag: Nation::ENGLAND,
                    town: Vec3::new(0.0, 0.0, 200.0),
                    locations: vec![Location::Market],
                },
                favour,
                Market {
                    entries: vec![
                        MarketEntry {
                            good: goods.soldiers,
                            stock: 2.0,
                            target: 30.0,
                            per_day: -0.1,
                        },
                        MarketEntry {
                            good: goods.id("rum").unwrap(),
                            stock: 0.0,
                            target: 30.0,
                            per_day: -1.0,
                        },
                    ],
                },
            ))
            .id();
        let pirate = app
            .world_mut()
            .spawn((Colours(Nation::PIRATES), Docked { port }))
            .id();
        (port, pirate)
    }

    #[test]
    fn a_restless_port_rises_for_the_pirate_in_its_harbour() {
        let mut app = sim_app();
        let (port, _) = restless_port(&mut app);
        let rose = run_until(&mut app, 30.0 * DAY as f32, |world| {
            world.get::<Port>(port).unwrap().flag == Nation::PIRATES
        });
        assert!(rose, "never rose");
        let favour = app.world().get::<Favour>(port).unwrap();
        assert!(!favour.restless(Nation::PIRATES), "{favour:?}");
    }

    #[test]
    fn an_owners_warship_on_guard_keeps_a_restless_port() {
        let mut app = sim_app();
        let (port, _) = restless_port(&mut app);
        app.world_mut().spawn((
            Patrol::bound_for(0),
            Colours(Nation::ENGLAND),
            Transform::from_xyz(100.0, 0.0, 0.0),
        ));
        let rose = run_until(&mut app, 30.0 * DAY as f32, |world| {
            world.get::<Port>(port).unwrap().flag != Nation::ENGLAND
        });
        assert!(!rose);
    }
}
