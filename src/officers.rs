//! Officers: the captain's afterguard. Each holds one post and brings one
//! plain advantage while he's free to do his duty. They're hired at
//! taverns, or recruited from the ships the captain takes.
//!
//! Every officer has a loyalty the captain can't see. Most are loyal; some
//! are ambitious, and lead mutinies when the men are unhappy; some are
//! turncoats in the pay of the law. What they get up to is in `loyalty`.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::alert::{Alert, AlertKind};
use crate::cargo::Purse;
use crate::clock::{DAY, WorldClock};
use crate::combat::rounds::Dice;
use crate::combat::{Engaged, Timbers};
use crate::economy::Refusal;
use crate::factions::Nation;
use crate::port::{Docked, Location, Port};
use crate::sailing::Rigging;
use crate::schedule::{FrameSet, SimSet};

/// Officers a tavern has looking for a post at any time.
const CANDIDATES: usize = 2;
/// How long one waits for a captain before moving on, seconds.
const WAITS: f64 = 3.0 * DAY;
/// Signing bounties asked, pieces of eight.
const LOWEST_BOUNTY: f32 = 100.0;
const HIGHEST_BOUNTY: f32 = 250.0;
/// A sailing master gets this much more out of her sails.
const SAILING_MASTER_SPEED: f32 = 1.1;
/// A gunner's crews reload in this fraction of the time.
const GUNNER_RELOAD: f32 = 0.75;
/// A surgeon saves this fraction of the wounded.
const SURGEON_SAVES: f64 = 0.3;
/// A first mate leading the boarders makes each worth this many.
const FIRST_MATE_BOARDING: f64 = 1.25;
/// A quartermaster keeps the men's resting morale this much higher.
const QUARTERMASTER_MORALE: f32 = 10.0;
/// A bosun repairs rigging and timbers at sea this much a day...
const BOSUN_RIGGING_PER_DAY: f32 = 0.1;
const BOSUN_TIMBERS_PER_DAY: f32 = 0.05;
/// ...but timbers only this far: the rest takes a shipyard.
const BOSUN_TIMBERS_LIMIT: f32 = 0.75;

const FIRST_NAMES: [&str; 12] = [
    "John",
    "William",
    "Thomas",
    "Edward",
    "Henry",
    "Richard",
    "Samuel",
    "James",
    "Robert",
    "Nathaniel",
    "George",
    "Israel",
];
const SURNAMES: [&str; 16] = [
    "Hale", "Bonnet", "Pell", "Fletcher", "Harker", "Teague", "Moody", "Rackham", "Vane", "Deane",
    "Cocklyn", "Hands", "Ashworth", "Lowther", "Beck", "Rowe",
];

/// An officer's post.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    FirstMate,
    SailingMaster,
    Gunner,
    Bosun,
    Surgeon,
    Quartermaster,
}

/// Every post, in the order they're listed.
pub const ROLES: [Role; 6] = [
    Role::FirstMate,
    Role::SailingMaster,
    Role::Gunner,
    Role::Bosun,
    Role::Surgeon,
    Role::Quartermaster,
];

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Self::FirstMate => "First Mate",
            Self::SailingMaster => "Sailing Master",
            Self::Gunner => "Gunner",
            Self::Bosun => "Bosun",
            Self::Surgeon => "Surgeon",
            Self::Quartermaster => "Quartermaster",
        }
    }

    /// What he does for the ship, in a line.
    pub fn duty(self) -> &'static str {
        match self {
            Self::FirstMate => "leads the boarders: each counts for more",
            Self::SailingMaster => "gets more out of her sails: faster",
            Self::Gunner => "drills the gun crews: quicker reloads",
            Self::Bosun => "repairs her rigging and timbers at sea",
            Self::Surgeon => "saves some of the wounded",
            Self::Quartermaster => "keeps the men content: higher morale",
        }
    }

    fn bit(self) -> u8 {
        1 << ROLES.iter().position(|&role| role == self).unwrap_or(0)
    }
}

/// Where an officer's heart lies. The captain never sees it, only its signs.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loyalty {
    /// Stands by the captain through anything.
    Loyal,
    /// Wants the ship for himself, and leads a mutiny when the men are
    /// unhappy enough to follow.
    Ambitious,
    /// In the law's pay: sells the captain out in its ports.
    Turncoat,
}

/// Where officers come from, which sets how likely each loyalty is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Hired at a tavern under this flag.
    Tavern(Nation),
    /// Brought aboard from a ship the captain took.
    Prisoner,
}

impl Origin {
    /// Chances of a loyal, an ambitious, and a turncoat officer: as the
    /// factions data has them for a tavern's flag, and for prisoners, who
    /// have the least reason to love the captain.
    pub fn odds(self) -> [f32; 3] {
        match self {
            Self::Tavern(nation) => nation.officer_odds(),
            Self::Prisoner => [0.35, 0.3, 0.35],
        }
    }

    /// The loyalty a roll in `[0, 1)` gives.
    pub fn loyalty(self, roll: f32) -> Loyalty {
        let [loyal, ambitious, _] = self.odds();
        if roll < loyal {
            Loyalty::Loyal
        } else if roll < loyal + ambitious {
            Loyalty::Ambitious
        } else {
            Loyalty::Turncoat
        }
    }
}

/// An officer. He also has a `Name` and a [`Berth`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[require(Name)]
pub struct Officer {
    pub role: Role,
    pub loyalty: Loyalty,
    /// In irons: no duty done, and no mischief either.
    pub confined: bool,
}

/// Where an officer is.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum Berth {
    /// Serving aboard this ship.
    Aboard(Entity),
    /// Held aboard this ship, taken from a prize. He'll sign on if asked.
    Captive(Entity),
    /// Looking for a post at this port's tavern until `until`, for a
    /// signing bounty.
    ForHire {
        port: Entity,
        bounty: u32,
        until: f64,
    },
}

/// The posts filled aboard a ship by officers free to do their duty, and
/// what they're worth. Kept up to date from her officers.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Afterguard(u8);

impl Afterguard {
    pub fn of(roles: impl IntoIterator<Item = Role>) -> Self {
        Self(roles.into_iter().fold(0, |bits, role| bits | role.bit()))
    }

    pub fn has(self, role: Role) -> bool {
        self.0 & role.bit() != 0
    }

    fn pick<T>(self, role: Role, with: T, without: T) -> T {
        if self.has(role) { with } else { without }
    }

    /// Sail handling multiplier.
    pub fn sailing(&self) -> f32 {
        self.pick(Role::SailingMaster, SAILING_MASTER_SPEED, 1.0)
    }

    /// Reload time multiplier.
    pub fn reload(&self) -> f32 {
        self.pick(Role::Gunner, GUNNER_RELOAD, 1.0)
    }

    /// Fraction of casualties that are lost.
    pub fn losses(&self) -> f64 {
        self.pick(Role::Surgeon, 1.0 - SURGEON_SAVES, 1.0)
    }

    /// What each boarder is worth.
    pub fn boarding(&self) -> f64 {
        self.pick(Role::FirstMate, FIRST_MATE_BOARDING, 1.0)
    }

    /// Resting morale added.
    pub fn morale(&self) -> f32 {
        self.pick(Role::Quartermaster, QUARTERMASTER_MORALE, 0.0)
    }
}

/// What a captain can do with an officer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficerAct {
    /// Take on one looking for a post at this port.
    Hire,
    /// Take a prisoner into the ship's service.
    SignOn,
    /// Put him in irons.
    Confine,
    Release,
    /// Put him ashore in port, or maroon him at sea. Prisoners too.
    Dismiss,
}

/// Order `ship`'s captain to do `act` with `officer`.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfficerOrder {
    pub ship: Entity,
    pub officer: Entity,
    pub act: OfficerAct,
}

/// The ship's side of an officer order.
#[derive(Debug, Clone, Copy)]
pub struct Quarterdeck<'a> {
    pub ship: Entity,
    /// The port she lies at, if any.
    pub docked: Option<Entity>,
    pub purse: Purse,
    /// The posts her officers hold, confined or not.
    pub posts: &'a [Role],
}

/// What an officer order costs, or why it can't be done. The screens and
/// the order both use this.
pub fn check_order(
    act: OfficerAct,
    officer: &Officer,
    berth: Berth,
    deck: &Quarterdeck,
) -> Result<u32, Refusal> {
    let vacant = || {
        if deck.posts.contains(&officer.role) {
            Err(Refusal::PostFilled)
        } else {
            Ok(())
        }
    };
    match (act, berth) {
        (OfficerAct::Hire, Berth::ForHire { port, bounty, .. }) => {
            if deck.docked != Some(port) {
                return Err(Refusal::NotHere);
            }
            vacant()?;
            if bounty > deck.purse.0 {
                return Err(Refusal::CantAfford);
            }
            Ok(bounty)
        }
        (OfficerAct::SignOn, Berth::Captive(ship)) if ship == deck.ship => vacant().map(|()| 0),
        (OfficerAct::Confine, Berth::Aboard(ship)) if ship == deck.ship && !officer.confined => {
            Ok(0)
        }
        (OfficerAct::Release, Berth::Aboard(ship)) if ship == deck.ship && officer.confined => {
            Ok(0)
        }
        (OfficerAct::Dismiss, Berth::Aboard(ship) | Berth::Captive(ship)) if ship == deck.ship => {
            Ok(0)
        }
        _ => Err(Refusal::NotHere),
    }
}

/// One of `items`, at random.
fn pick<T: Copy>(dice: &mut Dice, items: &[T]) -> T {
    items[((dice.roll() * items.len() as f32) as usize).min(items.len() - 1)]
}

/// A new officer, with his loyalty rolled for where he comes from.
pub fn recruit(dice: &mut Dice, origin: Origin) -> (Name, Officer) {
    let first = pick(dice, &FIRST_NAMES);
    let last = pick(dice, &SURNAMES);
    let role = pick(dice, &ROLES);
    let officer = Officer {
        role,
        loyalty: origin.loyalty(dice.roll()),
        confined: false,
    };
    (Name::new(format!("{first} {last}")), officer)
}

/// The posts held aboard `ship`, confined officers included.
pub fn posts<'a>(
    ship: Entity,
    officers: impl Iterator<Item = (&'a Officer, &'a Berth)>,
) -> Vec<Role> {
    officers
        .filter(|(_, berth)| **berth == Berth::Aboard(ship))
        .map(|(officer, _)| officer.role)
        .collect()
}

/// Moves every officer aboard or held aboard `from` that `keep` accepts to
/// `to`, as when the captain shifts her flag.
pub fn reassign<'a>(
    berths: impl Iterator<Item = (&'a Officer, Mut<'a, Berth>)>,
    from: Entity,
    to: Entity,
    keep: impl Fn(&Officer) -> bool,
) {
    for (officer, mut berth) in berths {
        if !keep(officer) {
            continue;
        }
        match *berth {
            Berth::Aboard(ship) if ship == from => *berth = Berth::Aboard(to),
            Berth::Captive(ship) if ship == from => *berth = Berth::Captive(to),
            _ => {}
        }
    }
}

pub struct OfficersPlugin;

impl Plugin for OfficersPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OfficerOrder>()
            .add_systems(
                FixedUpdate,
                (
                    muster.in_set(SimSet::Navigation),
                    bosun_repairs.in_set(SimSet::Resolve),
                    staff_taverns.in_set(SimSet::Economy),
                ),
            )
            // Orders work while paused, and so does taking prisoners, since
            // ships are boarded in battles fought by rounds.
            .add_systems(
                Update,
                (
                    carry_out.in_set(FrameSet::Orders),
                    (take_prisoners, discharge_strays).after(FrameSet::Orders),
                ),
            );
    }
}

/// Keeps each ship's afterguard to the officers aboard and free.
fn muster(mut ships: Query<(Entity, &mut Afterguard)>, officers: Query<(&Officer, &Berth)>) {
    for (ship, mut afterguard) in &mut ships {
        let roles = officers
            .iter()
            .filter(|(officer, berth)| **berth == Berth::Aboard(ship) && !officer.confined)
            .map(|(officer, _)| officer.role);
        afterguard.set_if_neq(Afterguard::of(roles));
    }
}

fn bosun_repairs(
    time: Res<Time>,
    mut ships: Query<
        (&Afterguard, &mut Timbers, &mut Rigging),
        (Without<Docked>, Without<Engaged>),
    >,
) {
    let days = (time.delta_secs_f64() / DAY) as f32;
    for (afterguard, mut timbers, mut rigging) in &mut ships {
        if !afterguard.has(Role::Bosun) {
            continue;
        }
        if rigging.0 < 1.0 {
            rigging.0 = (rigging.0 + BOSUN_RIGGING_PER_DAY * days).min(1.0);
        }
        if timbers.0 < BOSUN_TIMBERS_LIMIT {
            timbers.0 = (timbers.0 + BOSUN_TIMBERS_PER_DAY * days).min(BOSUN_TIMBERS_LIMIT);
        }
    }
}

/// Keeps a few officers waiting at every tavern. Those who've waited long
/// enough move on.
fn staff_taverns(
    clock: Res<WorldClock>,
    mut dice: ResMut<Dice>,
    ports: Query<(Entity, &Port)>,
    officers: Query<(Entity, &Berth), With<Officer>>,
    mut commands: Commands,
) {
    let now = clock.elapsed;
    for (port, details) in &ports {
        if !details.locations.contains(&Location::Tavern) {
            continue;
        }
        let mut waiting = 0;
        for (officer, berth) in &officers {
            if let Berth::ForHire {
                port: at, until, ..
            } = *berth
                && at == port
            {
                if until <= now {
                    commands.entity(officer).despawn();
                } else {
                    waiting += 1;
                }
            }
        }
        for _ in waiting..CANDIDATES {
            let (name, officer) = recruit(&mut dice, Origin::Tavern(details.flag));
            let bounty = LOWEST_BOUNTY.lerp(HIGHEST_BOUNTY, dice.roll()).round() as u32;
            // Staggered, so a tavern's officers don't all leave at once.
            let until = now + WAITS * (0.5 + dice.roll() as f64);
            commands.spawn((
                name,
                officer,
                Berth::ForHire {
                    port,
                    bounty,
                    until,
                },
            ));
        }
    }
}

/// A ship taken, or plundered, gives up her mate as a prisoner.
fn take_prisoners(
    mut alerts: MessageReader<Alert>,
    mut dice: ResMut<Dice>,
    mut commands: Commands,
) {
    for alert in alerts.read() {
        if matches!(
            alert.kind,
            AlertKind::PrizeTaken { .. } | AlertKind::Plundered { .. }
        ) {
            let (name, officer) = recruit(&mut dice, Origin::Prisoner);
            commands.spawn((name, officer, Berth::Captive(alert.ship)));
        }
    }
}

/// Officers of a ship that's gone, or a port that's gone, go with her.
fn discharge_strays(
    officers: Query<(Entity, &Berth), With<Officer>>,
    places: Query<(), Or<(With<Afterguard>, With<Port>)>>,
    mut commands: Commands,
) {
    for (officer, berth) in &officers {
        let place = match *berth {
            Berth::Aboard(ship) | Berth::Captive(ship) => ship,
            Berth::ForHire { port, .. } => port,
        };
        if !places.contains(place) {
            commands.entity(officer).despawn();
        }
    }
}

fn carry_out(
    mut orders: MessageReader<OfficerOrder>,
    mut ships: Query<(Option<&Docked>, &mut Purse)>,
    mut officers: Query<(Entity, &mut Officer, &mut Berth)>,
    mut commands: Commands,
) {
    for order in orders.read() {
        let Ok((docked, mut purse)) = ships.get_mut(order.ship) else {
            continue;
        };
        let posts = posts(
            order.ship,
            officers.iter().map(|(_, officer, berth)| (officer, berth)),
        );
        let Ok((entity, mut officer, mut berth)) = officers.get_mut(order.officer) else {
            continue;
        };
        let deck = Quarterdeck {
            ship: order.ship,
            docked: docked.map(|docked| docked.port),
            purse: *purse,
            posts: &posts,
        };
        let Ok(cost) = check_order(order.act, &officer, *berth, &deck) else {
            continue;
        };
        purse.0 -= cost;
        match order.act {
            OfficerAct::Hire | OfficerAct::SignOn => *berth = Berth::Aboard(order.ship),
            OfficerAct::Confine => officer.confined = true,
            OfficerAct::Release => officer.confined = false,
            OfficerAct::Dismiss => commands.entity(entity).despawn(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{run_seconds, sim_app};

    fn officer(role: Role) -> Officer {
        Officer {
            role,
            loyalty: Loyalty::Loyal,
            confined: false,
        }
    }

    fn entity(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn each_post_brings_its_own_advantage() {
        let none = Afterguard::default();
        let all = Afterguard::of(ROLES);
        assert!(all.sailing() > none.sailing());
        assert!(all.reload() < none.reload(), "quicker reloads");
        assert!(all.losses() < none.losses());
        assert!(all.boarding() > none.boarding());
        assert!(all.morale() > none.morale());
        let gunner = Afterguard::of([Role::Gunner]);
        assert!(gunner.has(Role::Gunner) && !gunner.has(Role::Bosun));
        assert_eq!(gunner.sailing(), none.sailing(), "only his own post");
    }

    #[test]
    fn rolls_cover_every_loyalty_in_proportion() {
        for origin in [
            Origin::Tavern(Nation::ENGLAND),
            Origin::Tavern(Nation::PIRATES),
            Origin::Prisoner,
        ] {
            assert!((origin.odds().iter().sum::<f32>() - 1.0).abs() < 1e-6);
            assert_eq!(origin.loyalty(0.0), Loyalty::Loyal);
            assert_eq!(origin.loyalty(0.999), Loyalty::Turncoat);
        }
        assert_eq!(
            Origin::Tavern(Nation::PIRATES).loyalty(0.6),
            Loyalty::Ambitious,
            "Nassau breeds ambition"
        );
        assert_eq!(Origin::Tavern(Nation::ENGLAND).loyalty(0.5), Loyalty::Loyal);
    }

    #[test]
    fn hiring_needs_the_port_the_money_and_a_vacant_post() {
        let (ship, port) = (entity(1), entity(2));
        let gunner = officer(Role::Gunner);
        let for_hire = Berth::ForHire {
            port,
            bounty: 150,
            until: 0.0,
        };
        let deck = |docked, purse, posts| Quarterdeck {
            ship,
            docked,
            purse: Purse(purse),
            posts,
        };
        let hire = |deck: &Quarterdeck| check_order(OfficerAct::Hire, &gunner, for_hire, deck);
        assert_eq!(hire(&deck(Some(port), 200, &[])), Ok(150));
        assert_eq!(hire(&deck(None, 200, &[])), Err(Refusal::NotHere));
        assert_eq!(hire(&deck(Some(port), 100, &[])), Err(Refusal::CantAfford));
        assert_eq!(
            hire(&deck(Some(port), 200, &[Role::Gunner])),
            Err(Refusal::PostFilled)
        );
    }

    #[test]
    fn only_her_own_officers_can_be_confined_released_or_dismissed() {
        let (ship, other) = (entity(1), entity(2));
        let deck = Quarterdeck {
            ship,
            docked: None,
            purse: Purse(0),
            posts: &[],
        };
        let mut bosun = officer(Role::Bosun);
        let check = |act, officer: &Officer, berth| check_order(act, officer, berth, &deck);
        assert!(check(OfficerAct::Confine, &bosun, Berth::Aboard(ship)).is_ok());
        assert!(check(OfficerAct::Release, &bosun, Berth::Aboard(ship)).is_err());
        assert!(check(OfficerAct::Confine, &bosun, Berth::Aboard(other)).is_err());
        bosun.confined = true;
        assert!(check(OfficerAct::Release, &bosun, Berth::Aboard(ship)).is_ok());
        assert!(check(OfficerAct::Dismiss, &bosun, Berth::Captive(ship)).is_ok());
        assert!(check(OfficerAct::SignOn, &bosun, Berth::Captive(ship)).is_ok());
        assert!(check(OfficerAct::SignOn, &bosun, Berth::Captive(other)).is_err());
    }

    #[test]
    fn officers_aboard_make_the_afterguard_unless_in_irons() {
        let mut app = sim_app();
        let ship = app
            .world_mut()
            .spawn((Afterguard::default(), Purse(0)))
            .id();
        let gunner = app
            .world_mut()
            .spawn((officer(Role::Gunner), Berth::Aboard(ship)))
            .id();
        app.world_mut()
            .spawn((officer(Role::Bosun), Berth::Captive(ship)));
        run_seconds(&mut app, 1.0);
        assert_eq!(
            *app.world().get::<Afterguard>(ship).unwrap(),
            Afterguard::of([Role::Gunner])
        );
        app.world_mut().write_message(OfficerOrder {
            ship,
            officer: gunner,
            act: OfficerAct::Confine,
        });
        run_seconds(&mut app, 1.0);
        assert_eq!(
            *app.world().get::<Afterguard>(ship).unwrap(),
            Afterguard::default()
        );
    }

    #[test]
    fn taverns_keep_officers_waiting_and_a_hired_one_joins_the_ship() {
        let (mut app, player) = crate::new_game::new_game();
        app.update();
        let port = app.world().get::<Docked>(player).unwrap().port;
        let candidates: Vec<(Entity, u32)> = app
            .world_mut()
            .query::<(Entity, &Berth)>()
            .iter(app.world())
            .filter_map(|(entity, berth)| match *berth {
                Berth::ForHire {
                    port: at, bounty, ..
                } if at == port => Some((entity, bounty)),
                _ => None,
            })
            .collect();
        assert_eq!(candidates.len(), CANDIDATES);
        let (officer, bounty) = candidates[0];
        let purse = app.world().get::<Purse>(player).unwrap().0;
        app.world_mut().write_message(OfficerOrder {
            ship: player,
            officer,
            act: OfficerAct::Hire,
        });
        app.update();
        assert_eq!(
            app.world().get::<Berth>(officer),
            Some(&Berth::Aboard(player))
        );
        assert_eq!(app.world().get::<Purse>(player).unwrap().0, purse - bounty);
    }
}
