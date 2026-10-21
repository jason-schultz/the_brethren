//! Pardons: the ways back for a captain a crown wants.
//!
//! Every so often a crown proclaims an Act of Grace. While it's in force, a
//! captain it wants, unless she's notorious, may come into any of its ports
//! with a governor and is pardoned on arrival. At any time, a lawyer in a
//! tavern will carry her petition for a fee that grows with her crimes.
//! Either way, her standing with each of the crown's groups rises to
//! [`PARDONED`]. Short of a pardon, she can win back standing by fighting
//! the crown's enemies. The Brethren pardon no one: they only forget.

use std::fmt;

use bevy::prelude::*;

use crate::alert::{Alert, AlertKind};
use crate::cargo::Purse;
use crate::clock::{DAY, WorldClock};
use crate::factions::{Nation, Standing, nations};
use crate::player::Player;
use crate::port::{Docked, Location, Port};
use crate::schedule::{FrameSet, SimSet};

/// A pardon leaves her this low with every group of the crown.
pub const PARDONED: f32 = -10.0;
/// At or below this with a crown, she's too notorious for an Act of Grace.
pub const NOTORIOUS: f32 = -80.0;
/// A lawyer's fee for every point her standing with a group is below
/// [`PARDONED`], pieces of eight.
const FEE_PER_POINT: f32 = 10.0;
/// A crown proclaims an Act of Grace this often...
const GRACE_EVERY: f64 = 120.0 * DAY;
/// ...and each stays in force this long.
pub const GRACE_DAYS: f64 = 30.0;

/// When a crown first proclaims an Act of Grace, seconds into the game, as
/// the factions data has it.
fn first_grace(nation: Nation) -> Option<f64> {
    nation.first_grace_day().map(|day| day * DAY)
}

/// When the Act of Grace `nation` has in force at `now` runs out, if it
/// has one.
pub fn grace_until(nation: Nation, now: f64) -> Option<f64> {
    let first = first_grace(nation)?;
    let since = (now - first).rem_euclid(GRACE_EVERY);
    (now >= first && since < GRACE_DAYS * DAY).then_some(now - since + GRACE_DAYS * DAY)
}

/// When `nation` next proclaims an Act of Grace after `now`, if it ever does.
pub fn next_grace(nation: Nation, now: f64) -> Option<f64> {
    let first = first_grace(nation)?;
    let proclaimed = ((now - first) / GRACE_EVERY).floor().max(-1.0);
    Some(first + (proclaimed + 1.0) * GRACE_EVERY)
}

/// Whether a captain with `standing` may come in under `nation`'s Act of
/// Grace at `now`: one it wants, but not a notorious one.
pub fn under_grace(standing: &Standing, nation: Nation, now: f64) -> bool {
    standing.wanted_by(nation)
        && standing.with(nation) > NOTORIOUS
        && grace_until(nation, now).is_some()
}

/// Why she can't petition for a pardon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoPetition {
    /// It wants her not, or pardons no one.
    NotWanted,
    /// There's no tavern here to find a lawyer in.
    NoLawyer,
    /// She can't pay the lawyer's fee.
    TooDear(u32),
}

impl fmt::Display for NoPetition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NotWanted => f.write_str("they don't want you"),
            Self::NoLawyer => f.write_str("there's no tavern here"),
            Self::TooDear(fee) => write!(f, "the lawyer asks {fee} pieces of eight"),
        }
    }
}

/// The fee for petitioning `nation` from `port` with `purse` in hand.
/// Both the port screen and the petition itself use this.
pub fn check_petition(
    standing: &Standing,
    nation: Nation,
    purse: Purse,
    port: &Port,
) -> Result<u32, NoPetition> {
    if !nation.is_crown() || !standing.wanted_by(nation) {
        return Err(NoPetition::NotWanted);
    }
    if !port.locations.contains(&Location::Tavern) {
        return Err(NoPetition::NoLawyer);
    }
    let points: f32 = nation
        .groups()
        .iter()
        .map(|&group| (PARDONED - standing.of(group)).max(0.0))
        .sum();
    let fee = (points * FEE_PER_POINT).round() as u32;
    if purse.0 < fee {
        Err(NoPetition::TooDear(fee))
    } else {
        Ok(fee)
    }
}

/// How a captain seeks a pardon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plea {
    /// She came into port under an Act of Grace.
    Grace,
    /// She paid a lawyer.
    Petition,
}

/// A captain seeks `nation`'s pardon.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeekPardon {
    pub captain: Entity,
    pub nation: Nation,
    pub plea: Plea,
}

pub struct PardonsPlugin;

impl Plugin for PardonsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SeekPardon>()
            .add_systems(Update, grant_pardons.in_set(FrameSet::Orders))
            .add_systems(FixedUpdate, proclaim_grace.in_set(SimSet::Resolve));
    }
}

fn grant_pardons(
    mut pleas: MessageReader<SeekPardon>,
    clock: Res<WorldClock>,
    mut captains: Query<(&Docked, &mut Purse, &mut Standing)>,
    ports: Query<&Port>,
    mut alerts: MessageWriter<Alert>,
) {
    for &SeekPardon {
        captain,
        nation,
        plea,
    } in pleas.read()
    {
        let Ok((docked, mut purse, mut standing)) = captains.get_mut(captain) else {
            continue;
        };
        let granted = match plea {
            Plea::Grace => under_grace(&standing, nation, clock.elapsed),
            Plea::Petition => {
                let fee = ports
                    .get(docked.port)
                    .map(|port| check_petition(&standing, nation, *purse, port));
                if let Ok(Ok(fee)) = fee {
                    purse.0 -= fee;
                }
                matches!(fee, Ok(Ok(_)))
            }
        };
        if granted {
            standing.lift(nation, PARDONED);
            alerts.write(Alert {
                ship: captain,
                kind: AlertKind::Pardoned { nation, plea },
            });
        }
    }
}

/// News when a crown proclaims an Act of Grace.
fn proclaim_grace(
    time: Res<Time>,
    clock: Res<WorldClock>,
    player: Query<Entity, With<Player>>,
    mut alerts: MessageWriter<Alert>,
) {
    let Ok(ship) = player.single() else {
        return;
    };
    let before = clock.elapsed - time.delta_secs_f64();
    for nation in nations() {
        if grace_until(nation, clock.elapsed).is_some() && grace_until(nation, before).is_none() {
            alerts.write(Alert {
                ship,
                kind: AlertKind::Grace { nation },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::{Arrival, SailOrder};
    use crate::new_game::{calm_seas, new_game};
    use crate::port::CastOff;
    use crate::test_support::run_until;
    use crate::time_scale::MAX_STEP;

    #[test]
    fn a_wanted_captain_comes_home_under_an_act_of_grace() {
        let (mut app, player) = new_game();
        calm_seas(&mut app);
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let port_royal = app.world().get::<Docked>(player).unwrap().port;
        app.world_mut().resource_mut::<WorldClock>().elapsed = 50.0 * DAY;
        app.world_mut()
            .get_mut::<Standing>(player)
            .unwrap()
            .set(Nation::ENGLAND, -50.0);
        app.world_mut().write_message(CastOff { ship: player });
        app.update();
        let anchorage = app
            .world()
            .get::<Transform>(port_royal)
            .unwrap()
            .translation;
        app.world_mut().write_message(SailOrder {
            ship: player,
            destination: anchorage,
            on_arrival: Arrival::Dock(port_royal),
        });
        let home = run_until(&mut app, 3600.0, |world| {
            world.get::<Docked>(player).is_some()
        });
        assert!(home, "turned away");
        app.update();
        let standing = app.world().get::<Standing>(player).unwrap();
        assert!(!standing.wanted_by(Nation::ENGLAND));
        // A breath of forgetting since.
        assert!((standing.with(Nation::ENGLAND) - PARDONED).abs() < 0.1);
    }

    #[test]
    fn crowns_proclaim_acts_of_grace_now_and_then_and_the_brethren_never() {
        let first = 45.0 * DAY;
        assert_eq!(grace_until(Nation::ENGLAND, first - 1.0), None);
        assert_eq!(next_grace(Nation::ENGLAND, 0.0), Some(first));
        let until = first + GRACE_DAYS * DAY;
        assert_eq!(grace_until(Nation::ENGLAND, first), Some(until));
        assert_eq!(grace_until(Nation::ENGLAND, until - 1.0), Some(until));
        assert_eq!(grace_until(Nation::ENGLAND, until), None);
        assert_eq!(
            next_grace(Nation::ENGLAND, first),
            Some(first + GRACE_EVERY)
        );
        assert_eq!(
            grace_until(Nation::ENGLAND, first + GRACE_EVERY + DAY),
            Some(until + GRACE_EVERY)
        );
        assert_eq!(grace_until(Nation::PIRATES, first), None);
        assert_eq!(next_grace(Nation::PIRATES, 0.0), None);
    }

    #[test]
    fn only_a_wanted_captain_short_of_notorious_comes_in_under_grace() {
        let during = 50.0 * DAY;
        let mut standing = Standing::default();
        assert!(
            !under_grace(&standing, Nation::ENGLAND, during),
            "not wanted"
        );
        standing.set(Nation::ENGLAND, -40.0);
        assert!(under_grace(&standing, Nation::ENGLAND, during));
        assert!(!under_grace(&standing, Nation::ENGLAND, 0.0), "no Act yet");
        standing.set(Nation::ENGLAND, NOTORIOUS);
        assert!(!under_grace(&standing, Nation::ENGLAND, during));
    }

    #[test]
    fn a_lawyer_charges_by_the_crime() {
        let port = Port {
            flag: Nation::PIRATES,
            town: Vec3::ZERO,
            locations: vec![Location::Tavern],
        };
        let mut standing = Standing::default();
        let rich = Purse(10_000);
        assert_eq!(
            check_petition(&standing, Nation::ENGLAND, rich, &port),
            Err(NoPetition::NotWanted)
        );
        standing.set(Nation::ENGLAND, -40.0);
        // Thirty points short of a pardon with each of three groups.
        assert_eq!(
            check_petition(&standing, Nation::ENGLAND, rich, &port),
            Ok(900)
        );
        standing.set(Nation::ENGLAND, -60.0);
        assert_eq!(
            check_petition(&standing, Nation::ENGLAND, Purse(900), &port),
            Err(NoPetition::TooDear(1500))
        );
        standing.set(Nation::PIRATES, -60.0);
        assert_eq!(
            check_petition(&standing, Nation::PIRATES, rich, &port),
            Err(NoPetition::NotWanted)
        );
    }
}
