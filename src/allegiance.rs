//! Allegiance: how each port regards each nation, and ports changing hands.
//!
//! A port's [`Favour`] for its owner settles toward a resting level, and
//! sinks while the things it needs run short. Whoever brings what it
//! needs, by contract or at market, wins favour for the colours she flies,
//! and the other nations lose a little. A port whose favour for its owner
//! sinks low enough is restless: its garrison deserts, and it will go over
//! to the nation it favours more, if an armed ship of that nation lies in
//! its roads and none of its owner's stand guard. The weaker the garrison,
//! the sooner. Trade wins a town over, but only guns raise it.
//!
//! How low is low enough, and how soon it rises, is the port's [`Tier`]: a
//! capital hardly ever changes hands, and a haven often. A crown's
//! capitals are kept by convoys from home, which bring some of what they
//! need every few weeks.

use bevy::prelude::*;
use serde::Deserialize;

use crate::alert::{Alert, AlertKind};
use crate::clock::{DAY, WorldClock};
use crate::combat::rounds::Dice;
use crate::combat::{Struck, Sunk};
use crate::economy::jobs::shortfall;
use crate::economy::market::{Market, MarketEntry, Supplied};
use crate::factions::{Colours, Deed, DeedKind, Group, GroupKind, Nation, nation_count, nations};
use crate::goods::{GoodId, Goods};
use crate::loyalty::chance_in;
use crate::patrols::Patrol;
use crate::player::Player;
use crate::port::Port;
use crate::raiders::Raider;
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
/// Favour won by filling a port's whole shortage of one good, as a matter
/// of course: small beside the bands below, so that winning a port over
/// takes a run of deliveries, not one.
const SUPPLY_FAVOUR: f32 = 4.0;
/// Relief is worth more: up to this many times as much, for a good the
/// port has gone short of for [`RELIEF_DAYS`]...
const RELIEF: f32 = 4.0;
const RELIEF_DAYS: f64 = 30.0;
/// A port deals with a nation it favours this much, whatever its owner
/// thinks of her.
pub const DEALS_WITH: f32 = 50.0;
/// A restless port's garrison deserts at this many men a day.
const DESERTION_PER_DAY: f64 = 2.0;
/// How a port without soldiers in its market counts its garrison.
const NO_GARRISON: f32 = 0.5;
/// An armed ship this close to a port's anchorage, meters, lies in its
/// roads: her owner's keep it from rising, and her rival's can raise it.
const ROADS: f32 = 1_500.0;
/// A convoy from home reaches each of a crown's capitals this often...
const CONVOY_DAYS: f64 = 20.0;
/// ...with this share of the stock it wants of each thing it needs, as far
/// as it wants.
const CONVOY_SHARE: f64 = 1.0 / 3.0;
/// When a port goes over, its new owner starts with at least this much
/// favour, and its old one with at most this much.
const NEW_OWNER: f32 = 60.0;
const OLD_OWNER: f32 = 10.0;

/// How firmly a port is held, from the map.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tier {
    /// A nation's seat: loyal through all but the worst neglect, and hard
    /// to raise even then.
    Capital,
    /// A town like most.
    #[default]
    Colony,
    /// A lawless anchorage that goes to whoever looks after it.
    Haven,
}

impl Tier {
    pub fn name(self) -> &'static str {
        match self {
            Self::Capital => "capital",
            Self::Colony => "colony",
            Self::Haven => "haven",
        }
    }

    /// Below this favour for its owner, a port is restless.
    pub fn restless_below(self) -> f32 {
        match self {
            Self::Capital => 10.0,
            Self::Colony => 25.0,
            Self::Haven => 35.0,
        }
    }

    /// The chance a day a restless port with no garrison at all rises,
    /// with an armed ship of the nation it favours in its roads. A full
    /// garrison holds it down.
    fn rising_per_day(self) -> f32 {
        match self {
            Self::Capital => 0.05,
            Self::Colony => 0.4,
            Self::Haven => 0.6,
        }
    }
}

/// How a port regards each nation, from 0 to 100, and how firmly it's
/// held.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Favour {
    values: Vec<f32>,
    pub tier: Tier,
}

impl Default for Favour {
    /// No nation's port in particular.
    fn default() -> Self {
        Self {
            values: vec![OTHER_REST; nation_count()],
            tier: Tier::default(),
        }
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
        self.values[nation.index()]
    }

    pub fn set(&mut self, nation: Nation, value: f32) {
        self.values[nation.index()] = value.clamp(LEAST, MOST);
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

    /// `by` filled `share` of a shortage the port had `waited` days with.
    /// Doing it as a matter of course costs no one; relief costs the other
    /// nations half what it's worth beyond that.
    pub fn supplied(&mut self, by: Nation, share: f32, waited: f64) {
        let gain = supply_favour(share, waited);
        let relief = gain - supply_favour(share, 0.0);
        for nation in nations() {
            self.change(nation, if nation == by { gain } else { -relief / 2.0 });
        }
    }

    /// Whether a port held by `owner` has turned against it.
    pub fn restless(&self, owner: Nation) -> bool {
        self.of(owner) < self.tier.restless_below()
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

/// The favour won by filling `share` of a port's shortage of one good,
/// after it had `waited` days short of it.
fn supply_favour(share: f32, waited: f64) -> f32 {
    let long = (waited / RELIEF_DAYS).min(1.0) as f32;
    SUPPLY_FAVOUR * share * (1.0 + (RELIEF - 1.0) * long)
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

/// The daily chance a restless port of `tier` rises, given its garrison.
pub fn rising_per_day(tier: Tier, garrison: f32) -> f32 {
    tier.rising_per_day() * (1.0 - garrison).max(0.0)
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
        (
            rising_per_day(favour.tier, now),
            rising_per_day(favour.tier, landed),
        )
    });
    Some(Stakes {
        fills,
        favour: supply_favour(fills as f32, entry.short_for),
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
                (convoys_from_home, keep_allegiance, rise)
                    .chain()
                    .in_set(SimSet::Economy),
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
            favour.supplied(nation, supply.share as f32, supply.waited);
        }
    }
}

/// Whether a convoy from home has come in during the last `step` seconds,
/// at `now`: one every [`CONVOY_DAYS`].
fn convoy_due(now: f64, step: f64) -> bool {
    let period = CONVOY_DAYS * DAY;
    (now / period).floor() > ((now - step) / period).floor()
}

/// What a convoy from home lands of `entry` at a port of `tier` flying
/// `flag`, as far as it wants: a share of what it needs at a crown's
/// capital, and nothing anywhere else.
fn convoy_landing(tier: Tier, flag: Nation, entry: &MarketEntry) -> f64 {
    if tier == Tier::Capital && flag.is_crown() && entry.is_needed() {
        entry.target * CONVOY_SHARE
    } else {
        0.0
    }
}

/// What convoys from home land of `entry` a day, at most, at a port of
/// `tier` flying `flag`.
pub fn convoys_per_day(tier: Tier, flag: Nation, entry: &MarketEntry) -> f64 {
    convoy_landing(tier, flag, entry) / CONVOY_DAYS
}

/// A convoy brings each of a crown's capitals a share of what it needs.
fn convoys_from_home(
    time: Res<Time>,
    clock: Res<WorldClock>,
    mut ports: Query<(&Port, &Favour, &mut Market)>,
) {
    if !convoy_due(clock.elapsed, time.delta_secs_f64()) {
        return;
    }
    for (port, favour, mut market) in &mut ports {
        for entry in &mut market.entries {
            let landed = convoy_landing(favour.tier, port.flag, entry);
            entry.stock = entry.stock.max((entry.stock + landed).min(entry.target));
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

/// Ships with guns to raise a town or keep it: warships, raiders, and the
/// player, unless they've struck or sunk. Merchantmen win a town over, but
/// can't raise it.
type Armed = (
    Or<(With<Patrol>, With<Raider>, With<Player>)>,
    Without<Struck>,
    Without<Sunk>,
);

/// A restless port rises for the nation it favours, if an armed ship of
/// hers lies in its roads, the player's first, and none of its owner's
/// does. The ship that was there gets the credit.
fn rise(
    time: Res<Time>,
    goods: Res<Goods>,
    mut dice: ResMut<Dice>,
    mut ports: Query<(Entity, &mut Port, &mut Favour, &Market, &Transform)>,
    armed: Query<(Entity, &Colours, &Transform, Has<Player>), Armed>,
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
        let in_roads = armed
            .iter()
            .filter(|(_, _, at, _)| at.translation.distance(anchorage.translation) <= ROADS);
        if in_roads.clone().any(|(_, colours, ..)| colours.0 == owner) {
            continue;
        }
        let Some((doer, ..)) = in_roads
            .filter(|(_, colours, ..)| colours.0 == pretender)
            .max_by_key(|&(.., is_player)| is_player)
        else {
            continue;
        };
        let rising = rising_per_day(favour.tier, garrison(&goods, market));
        if dice.roll() >= chance_in(rising, days) {
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
    use crate::port::{Docked, Location};
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
    fn supplying_a_port_as_a_matter_of_course_wins_a_little_and_costs_no_one() {
        let mut favour = Favour::held_by(Nation::ENGLAND);
        favour.supplied(Nation::PIRATES, 0.5, 0.0);
        assert_eq!(favour.of(Nation::PIRATES), OTHER_REST + 2.0);
        assert_eq!(favour.of(Nation::ENGLAND), OWNER_REST);
        assert_eq!(favour.of(Nation::SPAIN), OTHER_REST);
    }

    #[test]
    fn relief_after_a_long_wait_wins_more_at_the_others_expense() {
        let mut favour = Favour::held_by(Nation::ENGLAND);
        favour.supplied(Nation::PIRATES, 0.5, RELIEF_DAYS / 2.0);
        assert_eq!(favour.of(Nation::PIRATES), OTHER_REST + 5.0);
        assert_eq!(favour.of(Nation::ENGLAND), OWNER_REST - 1.5);
        let mut longer = Favour::held_by(Nation::ENGLAND);
        longer.supplied(Nation::PIRATES, 0.5, RELIEF_DAYS * 3.0);
        assert_eq!(
            longer.of(Nation::PIRATES),
            OTHER_REST + 8.0,
            "relief is worth at most {RELIEF} times as much"
        );
    }

    #[test]
    fn one_delivery_never_wins_a_port_over_or_its_trade() {
        // Filling a whole shortage the port has gone a season without.
        let mut favour = Favour::held_by(Nation::ENGLAND);
        favour.supplied(Nation::PIRATES, 1.0, RELIEF_DAYS * 3.0);
        assert!(favour.of(Nation::PIRATES) < DEALS_WITH);
        assert!(!favour.restless(Nation::ENGLAND));
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
            short_for: 10.0,
        };
        let market = Market {
            entries: vec![entry(rum, 10.0), entry(goods.soldiers, 0.0)],
        };
        let mut favour = Favour::held_by(Nation::SPAIN);
        let rum_stakes = stakes(&goods, rum, 20, Nation::SPAIN, &market, &favour).unwrap();
        assert_eq!(rum_stakes.fills, 0.5);
        let mut supplied = favour.clone();
        supplied.supplied(Nation::ENGLAND, rum_stakes.fills as f32, 10.0);
        assert_eq!(
            supplied.of(Nation::ENGLAND),
            favour.of(Nation::ENGLAND) + rum_stakes.favour,
            "the favour at stake is the favour delivery wins"
        );
        assert!(rum_stakes.short && rum_stakes.rising.is_none());

        favour.set(Nation::SPAIN, 10.0);
        let soldiers = stakes(&goods, goods.soldiers, 40, Nation::SPAIN, &market, &favour);
        let (now, landed) = soldiers.unwrap().rising.unwrap();
        assert_eq!((now, landed), (rising_per_day(Tier::Colony, 0.0), 0.0));

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
        for tier in [Tier::Capital, Tier::Colony, Tier::Haven] {
            assert_eq!(rising_per_day(tier, 1.0), 0.0);
            assert!(rising_per_day(tier, 0.0) > rising_per_day(tier, 0.5));
        }
    }

    #[test]
    fn a_capital_bears_neglect_a_colony_wont_and_a_haven_turns_soonest() {
        let settled = |tier, neglect| {
            let mut favour = Favour::held_by(Nation::SPAIN);
            favour.tier = tier;
            for _ in 0..200 {
                favour.settle(Nation::SPAIN, neglect, 1.0);
            }
            favour.restless(Nation::SPAIN)
        };
        assert!(settled(Tier::Colony, 0.8));
        assert!(!settled(Tier::Capital, 0.8));
        assert!(
            settled(Tier::Capital, 1.0),
            "utterly forgotten, even a capital turns"
        );
        assert!(settled(Tier::Haven, 0.5));
        assert!(!settled(Tier::Colony, 0.5));
        assert!(
            rising_per_day(Tier::Capital, 0.0) < rising_per_day(Tier::Colony, 0.0)
                && rising_per_day(Tier::Colony, 0.0) < rising_per_day(Tier::Haven, 0.0)
        );
    }

    #[test]
    fn a_convoy_comes_in_once_every_few_weeks_whatever_the_step() {
        let period = CONVOY_DAYS * DAY;
        assert!(convoy_due(period + 1.0, 2.0));
        assert!(!convoy_due(period + 3.0, 2.0));
        assert!(!convoy_due(period - 1.0, 2.0));
        let steps = (0..(3.0 * period / MAX_STEP) as u32)
            .filter(|step| convoy_due(f64::from(step + 1) * MAX_STEP, MAX_STEP))
            .count();
        assert_eq!(steps, 3);
    }

    #[test]
    fn convoys_keep_a_crowns_capitals_and_nowhere_else() {
        let mut app = sim_app();
        let goods = app.world().resource::<Goods>().clone();
        let rum = goods.id("rum").unwrap();
        let port = |app: &mut App, flag, tier| {
            let mut favour = Favour::held_by(flag);
            favour.tier = tier;
            let market = Market {
                entries: vec![MarketEntry {
                    good: rum,
                    stock: 0.0,
                    target: 30.0,
                    per_day: -1.0,
                    short_for: 0.0,
                }],
            };
            let port = Port {
                flag,
                town: Vec3::ZERO,
                locations: vec![Location::Market],
            };
            app.world_mut().spawn((port, favour, market)).id()
        };
        let capital = port(&mut app, Nation::SPAIN, Tier::Capital);
        let colony = port(&mut app, Nation::SPAIN, Tier::Colony);
        let haven = port(&mut app, Nation::PIRATES, Tier::Capital);
        app.world_mut().resource_mut::<WorldClock>().elapsed = CONVOY_DAYS * DAY - 1.0;
        run_until(&mut app, 10.0, |_| false);
        let rum_at = |port| {
            app.world()
                .get::<Market>(port)
                .unwrap()
                .entry(rum)
                .unwrap()
                .stock
        };
        assert!((rum_at(capital) - 30.0 * CONVOY_SHARE).abs() < 0.01);
        assert_eq!(rum_at(colony), 0.0);
        assert_eq!(rum_at(haven), 0.0, "no crown sends the Brethren convoys");
        // What the logbook is told convoys land is what they land.
        let entry = app.world().get::<Market>(capital).unwrap().entries[0].clone();
        assert_eq!(
            convoys_per_day(Tier::Capital, Nation::SPAIN, &entry) * CONVOY_DAYS,
            30.0 * CONVOY_SHARE
        );
        assert_eq!(convoys_per_day(Tier::Capital, Nation::PIRATES, &entry), 0.0);
    }

    /// A restless English colony with a weak garrison, short of rum.
    fn restless_port(app: &mut App) -> Entity {
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let goods = app.world().resource::<Goods>().clone();
        let mut favour = Favour::held_by(Nation::ENGLAND);
        favour.set(Nation::ENGLAND, 10.0);
        favour.set(Nation::PIRATES, 40.0);
        app.world_mut()
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
                            short_for: 0.0,
                        },
                        MarketEntry {
                            good: goods.id("rum").unwrap(),
                            stock: 0.0,
                            target: 30.0,
                            per_day: -1.0,
                            short_for: 0.0,
                        },
                    ],
                },
            ))
            .id()
    }

    /// A pirate lying in `port`, a raider if `armed`, else a merchantman.
    fn pirate_in(app: &mut App, port: Entity, armed: bool) {
        let mut ship = app.world_mut().spawn((
            Colours(Nation::PIRATES),
            Docked { port },
            Transform::default(),
        ));
        if armed {
            ship.insert(Raider {
                plan: crate::raiders::Raid::InPort { until: f64::MAX },
            });
        }
    }

    /// Whether `port` goes over within ten days, when an armed pirate in
    /// its roads would all but surely raise it.
    fn rises_within_ten_days(app: &mut App, port: Entity) -> bool {
        run_until(app, 10.0 * DAY as f32, |world| {
            world.get::<Port>(port).unwrap().flag != Nation::ENGLAND
        })
    }

    #[test]
    fn a_restless_port_rises_for_the_pirate_raider_in_its_harbour() {
        let mut app = sim_app();
        let port = restless_port(&mut app);
        pirate_in(&mut app, port, true);
        let rose = run_until(&mut app, 30.0 * DAY as f32, |world| {
            world.get::<Port>(port).unwrap().flag == Nation::PIRATES
        });
        assert!(rose, "never rose");
        let favour = app.world().get::<Favour>(port).unwrap();
        assert!(!favour.restless(Nation::PIRATES), "{favour:?}");
    }

    #[test]
    fn a_merchantman_wins_a_town_over_but_cannot_raise_it() {
        let mut app = sim_app();
        let port = restless_port(&mut app);
        pirate_in(&mut app, port, false);
        assert!(!rises_within_ten_days(&mut app, port));
    }

    #[test]
    fn an_owners_warship_on_guard_keeps_a_restless_port() {
        let mut app = sim_app();
        let port = restless_port(&mut app);
        pirate_in(&mut app, port, true);
        app.world_mut().spawn((
            Patrol::bound_for(0),
            Colours(Nation::ENGLAND),
            Transform::from_xyz(100.0, 0.0, 0.0),
        ));
        assert!(!rises_within_ten_days(&mut app, port));
    }
}
