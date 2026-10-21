//! Ships' companies: how many hands a ship needs to sail, feeding them,
//! and their morale.
//!
//! Morale runs from 0 to 100. It settles toward a resting level: higher in
//! port than on a long cruise, and nowhere near it for a starving crew.
//! Victories and plunder lift it; defeats and losses sink it. A sullen crew
//! works slowly and deserts in port, and a mutinous one is ripe for a
//! ringleader (see `loyalty`).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::alert::{Alert, AlertKind, fell_below};
use crate::cargo::{Hold, Purse};
use crate::clock::DAY;
use crate::economy::Refusal;
use crate::goods::Goods;
use crate::officers::Afterguard;
use crate::port::{Docked, Location, Port};
use crate::prizes::Prize;
use crate::sailing::SailHandling;
use crate::schedule::{FrameSet, SimSet};

/// Below this fraction of her minimum crew, a ship can't make sail at all.
pub const SKELETON_FRACTION: f32 = 0.25;
/// Tons of provisions (food and water) one person eats in a day.
pub const RATION_TONS: f64 = 0.005;
/// Provisions for fewer than this many days raise an alert.
pub const LOW_PROVISIONS_DAYS: f64 = 3.0;
/// Fraction of a starving crew lost, to death or desertion, each day.
const STARVATION_LOSS: f64 = 0.1;
/// A new crew's morale: steady.
pub const NEW_CREW_MORALE: f32 = 50.0;
/// Morale settles toward these in port and at sea...
const RESTING_IN_PORT: f32 = 60.0;
const RESTING_AT_SEA: f32 = 45.0;
/// ...by this many points a day, and a starving crew's falls this fast.
const SETTLING_PER_DAY: f32 = 4.0;
const STARVING_PER_DAY: f32 = 15.0;
/// Losing the whole crew would cost this much morale; losing a tenth, a
/// tenth of it.
const LOSSES_MORALE: f32 = 40.0;
/// A sullen or mutinous crew in port loses this fraction of its men a day
/// to desertion.
const DESERTION_PER_DAY: f64 = 0.1;
/// What a run ashore costs a head at the tavern, and what it does for morale.
pub const SHORE_LEAVE_PER_MAN: u32 = 3;
const SHORE_LEAVE_MORALE: f32 = 25.0;
/// Morale gained from news of a fight.
const PRIZE_MORALE: f32 = 15.0;
const PLUNDER_MORALE: f32 = 10.0;
const VICTORY_MORALE: f32 = 5.0;
const REPULSED_MORALE: f32 = -10.0;

/// The men working a ship.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crew {
    pub count: u32,
    /// Seconds the crew has gone without provisions.
    hungry_for: f64,
    /// Men lost but not yet a whole man, so steady fire or desertion wears
    /// a crew down instead of rounding away.
    wounds: f64,
    /// From 0 (mutinous) to 100 (eager).
    pub morale: f32,
}

impl Crew {
    pub fn new(count: u32) -> Self {
        Self {
            count,
            hungry_for: 0.0,
            wounds: 0.0,
            morale: NEW_CREW_MORALE,
        }
    }

    /// Loses `men`, whole or in part. Fractions add up. Returns the whole
    /// men lost.
    fn lose(&mut self, men: f64) -> u32 {
        self.wounds += men;
        let lost = (self.wounds.floor() as u32).min(self.count);
        self.wounds -= self.wounds.floor();
        self.count -= lost;
        lost
    }

    /// Takes `men` casualties, killed or out of action. Fractions add up,
    /// and every man lost costs the survivors some heart.
    pub fn take_casualties(&mut self, men: f64) {
        let before = self.count;
        let lost = self.lose(men);
        if lost > 0 {
            self.cheer(-LOSSES_MORALE * lost as f32 / before as f32);
        }
    }

    /// Raises morale by `by` points, or lowers it if negative.
    pub fn cheer(&mut self, by: f32) {
        self.morale = (self.morale + by).clamp(0.0, 100.0);
    }

    /// How the men feel.
    pub fn spirits(&self) -> Spirits {
        Spirits::of(self.morale)
    }

    pub fn is_starving(&self) -> bool {
        self.hungry_for > 0.0
    }
}

/// Morale in words, and what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Spirits {
    /// Below 20: ripe for mutiny.
    Mutinous,
    /// Below 40: slow to work, and deserting in port.
    Sullen,
    Steady,
    Content,
    /// 80 and over: working with a will.
    Eager,
}

impl Spirits {
    pub fn of(morale: f32) -> Self {
        match morale {
            m if m < 20.0 => Self::Mutinous,
            m if m < 40.0 => Self::Sullen,
            m if m < 60.0 => Self::Steady,
            m if m < 80.0 => Self::Content,
            _ => Self::Eager,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Mutinous => "mutinous",
            Self::Sullen => "sullen",
            Self::Steady => "steady",
            Self::Content => "content",
            Self::Eager => "eager",
        }
    }

    /// How hard the men work, sailing and fighting, against a steady crew.
    pub fn work_rate(self) -> f32 {
        match self {
            Self::Mutinous => 0.8,
            Self::Sullen => 0.9,
            Self::Steady | Self::Content => 1.0,
            Self::Eager => 1.1,
        }
    }

    /// Whether men desert when the ship lies in port.
    pub fn deserting(self) -> bool {
        self <= Self::Sullen
    }
}

/// The morale a crew settles toward. `bonus` is what a quartermaster adds.
pub fn resting_morale(docked: bool, bonus: f32) -> f32 {
    let resting = if docked {
        RESTING_IN_PORT
    } else {
        RESTING_AT_SEA
    };
    resting + bonus
}

/// Morale after `days` settling toward `resting`, or falling for a
/// starving crew.
pub fn settle(morale: f32, resting: f32, starving: bool, days: f32) -> f32 {
    if starving {
        return (morale - STARVING_PER_DAY * days).max(0.0);
    }
    let step = SETTLING_PER_DAY * days;
    (morale + (resting - morale).clamp(-step, step)).clamp(0.0, 100.0)
}

/// How news of a fight moves the morale of the crew it's about.
pub fn morale_from(kind: AlertKind) -> f32 {
    match kind {
        AlertKind::PrizeTaken { .. } => PRIZE_MORALE,
        AlertKind::Plundered { .. } => PLUNDER_MORALE,
        AlertKind::EnemyStruck { .. } | AlertKind::EnemySank { .. } => VICTORY_MORALE,
        AlertKind::Repulsed { .. } => REPULSED_MORALE,
        _ => 0.0,
    }
}

/// What a run ashore costs a crew of `crew` men at `port`, or why there's
/// none to be had. The port screen and the order both use this.
pub fn shore_leave_cost(port: &Port, crew: &Crew, purse: Purse) -> Result<u32, Refusal> {
    if !port.locations.contains(&Location::Tavern) {
        return Err(Refusal::NotTraded);
    }
    let cost = crew.count * SHORE_LEAVE_PER_MAN;
    if cost > purse.0 {
        Err(Refusal::CantAfford)
    } else {
        Ok(cost)
    }
}

/// Order a docked ship's crew a run ashore, to raise their morale.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShoreLeave {
    pub ship: Entity,
}

/// How many people a ship class needs and holds.
#[derive(Component, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Complement {
    /// Hands needed to work the ship day to day.
    pub min: u32,
    /// Hands needed to also fight her guns.
    pub full: u32,
    /// Most people she can carry, crew and passengers together.
    pub berths: u32,
}

/// How much of her sail a crew of `crew` can work on a ship needing `min` hands.
pub fn sail_handling(crew: u32, min: u32) -> f32 {
    let manned = crew as f32 / min.max(1) as f32;
    if manned < SKELETON_FRACTION {
        0.0
    } else {
        manned.min(1.0)
    }
}

/// Days `tons` of provisions will last `people`.
pub fn days_of_provisions(tons: f64, people: f64) -> f64 {
    if people <= 0.0 {
        f64::INFINITY
    } else {
        tons / (people * RATION_TONS)
    }
}

/// Feeds the crew and `passengers` for `dt` seconds from `provisions` tons.
/// Returns the tons eaten. A crew without enough to eat loses men for every
/// full day it goes hungry.
pub fn feed(crew: &mut Crew, provisions: f64, passengers: f64, dt: f64) -> f64 {
    let people = crew.count as f64 + passengers;
    let needed = people * RATION_TONS * dt / DAY;
    if provisions >= needed {
        crew.hungry_for = 0.0;
        return needed;
    }
    crew.hungry_for += dt;
    if crew.hungry_for >= DAY {
        crew.hungry_for -= DAY;
        let lost = (crew.count as f64 * STARVATION_LOSS).ceil() as u32;
        crew.count -= lost.min(crew.count);
    }
    provisions
}

pub struct CrewPlugin;

impl Plugin for CrewPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShoreLeave>()
            .add_systems(FixedUpdate, man_the_sails.in_set(SimSet::Navigation))
            .add_systems(
                FixedUpdate,
                (eat, keep_spirits, hear_the_news).in_set(SimSet::Resolve),
            )
            // An order, so it works while paused.
            .add_systems(Update, shore_leave.in_set(FrameSet::Orders));
    }
}

/// A prize crew only works her home under easy sail, not fights her: this
/// many hands sail a prize as well as a full crew would.
pub const PRIZE_HANDS: u32 = 2;

/// Sets how much sail each crew can work: her numbers, how hard the men
/// work, and her sailing master's skill.
fn man_the_sails(
    mut ships: Query<(
        &Crew,
        &Complement,
        Has<Prize>,
        Option<&Afterguard>,
        &mut SailHandling,
    )>,
) {
    for (crew, complement, prize, afterguard, mut handling) in &mut ships {
        let needed = if prize { PRIZE_HANDS } else { complement.min };
        let skill = afterguard.map_or(1.0, Afterguard::sailing);
        handling.set_if_neq(SailHandling(
            sail_handling(crew.count, needed) * crew.spirits().work_rate() * skill,
        ));
    }
}

/// Morale settles, and a sullen crew in port drifts away.
fn keep_spirits(time: Res<Time>, mut ships: Query<(&mut Crew, Has<Docked>, Option<&Afterguard>)>) {
    let days = time.delta_secs_f64() / DAY;
    for (mut crew, docked, afterguard) in &mut ships {
        let resting = resting_morale(docked, afterguard.map_or(0.0, Afterguard::morale));
        let morale = settle(crew.morale, resting, crew.is_starving(), days as f32);
        if morale != crew.morale {
            crew.morale = morale;
        }
        if docked && crew.spirits().deserting() {
            let deserters = crew.count as f64 * DESERTION_PER_DAY * days;
            crew.lose(deserters);
        }
    }
}

/// Victories and defeats move the morale of the crews they're about.
fn hear_the_news(mut alerts: MessageReader<Alert>, mut crews: Query<&mut Crew>) {
    for alert in alerts.read() {
        let by = morale_from(alert.kind);
        if by != 0.0
            && let Ok(mut crew) = crews.get_mut(alert.ship)
        {
            crew.cheer(by);
        }
    }
}

fn shore_leave(
    mut orders: MessageReader<ShoreLeave>,
    ports: Query<&Port>,
    mut ships: Query<(&Docked, &mut Crew, &mut Purse)>,
) {
    for order in orders.read() {
        let Ok((docked, mut crew, mut purse)) = ships.get_mut(order.ship) else {
            continue;
        };
        let Ok(port) = ports.get(docked.port) else {
            continue;
        };
        if let Ok(cost) = shore_leave_cost(port, &crew, *purse) {
            purse.0 -= cost;
            crew.cheer(SHORE_LEAVE_MORALE);
        }
    }
}

fn eat(
    time: Res<Time>,
    goods: Res<Goods>,
    mut ships: Query<(Entity, &mut Crew, &mut Hold)>,
    mut alerts: MessageWriter<Alert>,
) {
    let dt = time.delta_secs_f64();
    for (ship, mut crew, mut hold) in &mut ships {
        let passengers = hold.passengers(&goods);
        let people = crew.count as f64 + passengers;
        let before = hold.owned.get(goods.provisions);
        let eaten = feed(&mut crew, before, passengers, dt);
        hold.owned.remove(goods.provisions, eaten);
        let after = hold.owned.get(goods.provisions);
        if fell_below(
            days_of_provisions(before, people),
            days_of_provisions(after, people),
            LOW_PROVISIONS_DAYS,
        ) {
            alerts.write(Alert {
                ship,
                kind: AlertKind::ProvisionsLow,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship_class::ShipClass;
    use crate::test_support::{run_seconds, sim_app};

    #[test]
    fn a_full_crew_handles_all_sail_and_a_skeleton_crew_none() {
        assert_eq!(sail_handling(8, 8), 1.0);
        assert_eq!(sail_handling(40, 8), 1.0, "extra hands don't add sail");
        assert_eq!(sail_handling(4, 8), 0.5);
        assert_eq!(sail_handling(1, 8), 0.0, "below a skeleton crew");
        assert_eq!(sail_handling(0, 0), 0.0);
    }

    #[test]
    fn casualties_add_up_to_whole_men() {
        let mut crew = Crew::new(10);
        crew.take_casualties(0.4);
        assert_eq!(crew.count, 10);
        crew.take_casualties(0.7);
        assert_eq!(crew.count, 9);
        crew.take_casualties(50.0);
        assert_eq!(crew.count, 0, "never below none");
    }

    #[test]
    fn happier_crews_work_harder() {
        let rates: Vec<f32> = [5.0, 30.0, 50.0, 70.0, 90.0]
            .iter()
            .map(|&morale| Spirits::of(morale).work_rate())
            .collect();
        assert!(rates.windows(2).all(|pair| pair[0] <= pair[1]), "{rates:?}");
        assert_eq!(
            Spirits::of(NEW_CREW_MORALE).work_rate(),
            1.0,
            "a new crew is steady"
        );
        assert!(Spirits::of(30.0).deserting() && !Spirits::of(50.0).deserting());
    }

    #[test]
    fn morale_settles_toward_its_resting_level_and_hunger_sinks_it() {
        assert_eq!(settle(20.0, 60.0, false, 1.0), 20.0 + SETTLING_PER_DAY);
        assert_eq!(settle(59.0, 60.0, false, 10.0), 60.0, "no overshoot");
        assert_eq!(settle(90.0, 45.0, false, 1.0), 90.0 - SETTLING_PER_DAY);
        assert_eq!(settle(50.0, 60.0, true, 1.0), 50.0 - STARVING_PER_DAY);
        assert_eq!(settle(5.0, 60.0, true, 1.0), 0.0);
        assert!(
            resting_morale(true, 0.0) > resting_morale(false, 0.0),
            "port is better"
        );
        assert!(resting_morale(false, 10.0) > resting_morale(false, 0.0));
    }

    #[test]
    fn losses_cost_the_survivors_heart_in_proportion() {
        let mut light = Crew::new(40);
        light.take_casualties(4.0);
        let mut heavy = Crew::new(40);
        heavy.take_casualties(20.0);
        assert!(light.morale < NEW_CREW_MORALE);
        assert!(heavy.morale < light.morale);
        let mut grazed = Crew::new(40);
        grazed.take_casualties(0.5);
        assert_eq!(grazed.morale, NEW_CREW_MORALE, "no one lost yet");
    }

    #[test]
    fn prizes_cheer_the_men_and_repulses_dishearten_them() {
        let other = Entity::PLACEHOLDER;
        assert!(morale_from(AlertKind::PrizeTaken { other }) > 0.0);
        assert!(morale_from(AlertKind::Repulsed { other }) < 0.0);
        assert_eq!(
            morale_from(AlertKind::SailSighted {
                other,
                hostile: false
            }),
            0.0
        );
    }

    /// A docked sloop with `crew` men at `morale`, at a port with a tavern.
    fn in_port(morale: f32, crew: u32) -> (App, Entity) {
        let mut app = sim_app();
        app.insert_resource(Time::<Fixed>::from_seconds(crate::time_scale::MAX_STEP));
        let port = app
            .world_mut()
            .spawn(Port {
                flag: crate::factions::Nation::ENGLAND,
                town: Vec3::ZERO,
                locations: vec![Location::Tavern],
            })
            .id();
        let goods = app.world().resource::<Goods>().clone();
        let mut hold = Hold::new(40.0);
        hold.owned.add(goods.provisions, 5.0);
        let mut men = Crew::new(crew);
        men.morale = morale;
        let ship = app
            .world_mut()
            .spawn((men, hold, Purse(500), Docked { port }))
            .id();
        (app, ship)
    }

    #[test]
    fn a_sullen_crew_deserts_in_port() {
        let (mut app, ship) = in_port(10.0, 20);
        run_seconds(&mut app, DAY as f32);
        let crew = app.world().get::<Crew>(ship).unwrap();
        assert!(crew.count < 20 && crew.count >= 17, "{}", crew.count);
        let (mut app, ship) = in_port(50.0, 20);
        run_seconds(&mut app, DAY as f32);
        assert_eq!(app.world().get::<Crew>(ship).unwrap().count, 20);
    }

    #[test]
    fn a_run_ashore_costs_a_few_pieces_a_man_and_cheers_them() {
        let (mut app, ship) = in_port(30.0, 20);
        app.world_mut().write_message(ShoreLeave { ship });
        app.update();
        let world = app.world();
        assert_eq!(
            world.get::<Purse>(ship).unwrap().0,
            500 - 20 * SHORE_LEAVE_PER_MAN
        );
        assert!(world.get::<Crew>(ship).unwrap().morale >= 30.0 + SHORE_LEAVE_MORALE - 1.0);
    }

    #[test]
    fn provisions_last_longer_with_fewer_mouths() {
        assert!((days_of_provisions(1.0, 10.0) - 20.0).abs() < 1e-9);
        assert!((days_of_provisions(1.0, 20.0) - 10.0).abs() < 1e-9);
        assert_eq!(days_of_provisions(1.0, 0.0), f64::INFINITY);
    }

    #[test]
    fn a_fed_crew_eats_its_ration_and_loses_no_one() {
        let mut crew = Crew::new(10);
        let eaten = feed(&mut crew, 1.0, 10.0, DAY);
        assert!((eaten - 20.0 * RATION_TONS).abs() < 1e-9);
        assert_eq!(crew.count, 10);
        assert!(!crew.is_starving());
    }

    #[test]
    fn a_starving_crew_loses_a_tenth_each_day() {
        let mut crew = Crew::new(20);
        feed(&mut crew, 0.0, 0.0, DAY / 2.0);
        assert_eq!(crew.count, 20, "half a day hungry costs no one yet");
        assert!(crew.is_starving());
        feed(&mut crew, 0.0, 0.0, DAY / 2.0);
        assert_eq!(crew.count, 18);
        feed(&mut crew, 1.0, 0.0, 60.0);
        assert!(!crew.is_starving(), "fed again");
    }

    #[test]
    fn crew_size_sets_sail_handling_and_stores_are_eaten() {
        let mut app = sim_app();
        let goods = app.world().resource::<Goods>().clone();
        let mut hold = Hold::new(40.0);
        hold.owned.add(goods.provisions, 1.0);
        let complement = Complement {
            min: 8,
            full: 40,
            berths: 75,
        };
        let ship = app
            .world_mut()
            .spawn((ShipClass::sloop().hull, Crew::new(4), complement, hold))
            .id();
        run_seconds(&mut app, 600.0);

        assert_eq!(app.world().get::<SailHandling>(ship).unwrap().0, 0.5);
        let left = app
            .world()
            .get::<Hold>(ship)
            .unwrap()
            .owned
            .get(goods.provisions);
        assert!(left < 1.0, "nothing eaten");
    }

    #[test]
    fn running_low_on_provisions_raises_one_alert() {
        let mut app = sim_app();
        app.insert_resource(Time::<Fixed>::from_seconds(crate::time_scale::MAX_STEP));
        let goods = app.world().resource::<Goods>().clone();
        let mut hold = Hold::new(40.0);
        // Ten men eat 0.05 t a day: this is a little over the alert threshold.
        hold.owned
            .add(goods.provisions, 0.05 * (LOW_PROVISIONS_DAYS + 0.1));
        let ship = app.world_mut().spawn((Crew::new(10), hold)).id();
        let mut alerts = Vec::new();
        crate::test_support::run_checked(&mut app, 86_400.0, |world| {
            let messages = world.resource::<Messages<Alert>>();
            alerts.extend(messages.iter_current_update_messages().copied());
        });
        assert_eq!(
            alerts,
            vec![Alert {
                ship,
                kind: AlertKind::ProvisionsLow
            }]
        );
    }
}
