//! Pure tactics: how a captain sizes up an enemy, and where she steers.
//! No ECS types here.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;

use super::gunnery::GUN_RANGE;
use crate::math::{angle_between, direction_to_heading, heading_to_direction, normalize_heading};

/// Ships this close, meters, can board one another.
pub const BOARDING_RANGE: f32 = 60.0;
/// A ship closing to board takes in sail this close to her quarry, meters,
/// and drifts the rest of the way.
pub const LIE_ALONGSIDE: f32 = BOARDING_RANGE * 0.6;
/// How far abeam of her quarry a pursuer lays herself, meters: well inside
/// gun range, with room to turn.
pub const STANDOFF: f32 = 150.0;
/// How far ahead of the quarry the pursuer aims, meters, so she comes up
/// alongside instead of falling in astern.
const LEAD: f32 = 60.0;
/// A pursuer this close to her station alongside matches the quarry's course.
const ON_STATION: f32 = 80.0;
/// Timbers or rigging below this leave a ship unable to fight or run well.
const COMPROMISED: f32 = 0.4;
/// A captain strikes her colours when timbers fall below this...
const STRIKE_TIMBERS: f32 = 0.25;
/// ...or rigging, so she can't run...
const STRIKE_RIGGING: f32 = 0.2;
/// ...or her crew falls below this fraction of the hands she needs.
const STRIKE_CREW: f32 = 0.5;

/// What a ship brings to a fight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strength {
    pub guns_per_side: u32,
    pub crew: u32,
    /// Hands needed to work the ship.
    pub min_crew: u32,
    /// How much shot she takes, as a multiple of a sloop.
    pub stoutness: f32,
    pub timbers: f32,
    pub rigging: f32,
}

impl Strength {
    /// A rough measure of how hard she hits and how long she lasts.
    pub fn power(&self) -> f32 {
        let manned = (self.crew as f32 / self.min_crew.max(1) as f32).min(1.5);
        self.guns_per_side as f32 * self.timbers * self.stoutness * manned
    }

    /// Too battered to fight or run well.
    pub fn compromised(&self) -> bool {
        self.timbers < COMPROMISED || self.rigging < COMPROMISED
    }

    /// Beaten: time to strike her colours.
    pub fn beaten(&self) -> bool {
        self.timbers < STRIKE_TIMBERS
            || self.rigging < STRIKE_RIGGING
            || (self.crew as f32) < self.min_crew as f32 * STRIKE_CREW
    }
}

/// What a ship does in a fight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tactic {
    /// Run her quarry down and lie alongside.
    Pursue,
    /// Turn her broadside on the enemy.
    Fight,
    /// Run from the enemy.
    Flee,
    /// Lay her alongside a struck enemy, to board.
    Close,
}

/// An attacked captain fights if she's at least as strong as her attacker,
/// or the attacker is too battered to press home. Otherwise she runs.
pub fn defend(own: &Strength, enemy: &Strength) -> Tactic {
    if own.power() >= enemy.power() || enemy.compromised() {
        Tactic::Fight
    } else {
        Tactic::Flee
    }
}

/// The course for `tactic`, sailing from `own` against an enemy at `enemy`
/// heading `enemy_heading`, in a wind from `wind_from`. Land is not
/// considered.
pub fn course(
    tactic: Tactic,
    own: Vec3,
    own_heading: f32,
    enemy: Vec3,
    enemy_heading: f32,
    wind_from: f32,
) -> f32 {
    let to_enemy = direction_to_heading(enemy - own);
    match tactic {
        Tactic::Pursue => {
            let station = station_alongside(own, enemy, enemy_heading);
            if own.distance(station) <= ON_STATION {
                enemy_heading
            } else {
                direction_to_heading(station - own)
            }
        }
        Tactic::Fight => {
            if own.distance(enemy) > GUN_RANGE {
                to_enemy
            } else {
                // Broadside on, turning whichever way is shorter.
                let starboard_on = normalize_heading(to_enemy - 90_f32.to_radians());
                let port_on = normalize_heading(to_enemy + 90_f32.to_radians());
                if angle_between(own_heading, starboard_on).abs()
                    <= angle_between(own_heading, port_on).abs()
                {
                    starboard_on
                } else {
                    port_on
                }
            }
        }
        Tactic::Close => to_enemy,
        Tactic::Flee => {
            // Run downwind, her fastest point, if that opens the distance.
            let away = normalize_heading(to_enemy + PI);
            let downwind = normalize_heading(wind_from + PI);
            if angle_between(away, downwind).abs() < FRAC_PI_2 {
                downwind
            } else {
                away
            }
        }
    }
}

/// The point [`STANDOFF`] abeam of the enemy, on whichever side `own` is,
/// a little ahead of her.
fn station_alongside(own: Vec3, enemy: Vec3, enemy_heading: f32) -> Vec3 {
    let ahead = heading_to_direction(enemy_heading);
    let starboard = heading_to_direction(enemy_heading + 90_f32.to_radians());
    let side = if (own - enemy).dot(starboard) >= 0.0 {
        starboard
    } else {
        -starboard
    };
    enemy + side * STANDOFF + ahead * LEAD
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::gunnery::{Broadside, bearing_broadside};

    fn sloop(guns_per_side: u32, crew: u32) -> Strength {
        Strength {
            guns_per_side,
            crew,
            min_crew: 8,
            stoutness: 1.0,
            timbers: 1.0,
            rigging: 1.0,
        }
    }

    #[test]
    fn a_weaker_ship_runs_and_a_stronger_one_fights() {
        let merchant = sloop(2, 10);
        let raider = sloop(4, 40);
        assert_eq!(defend(&merchant, &raider), Tactic::Flee);
        assert_eq!(defend(&raider, &merchant), Tactic::Fight);
    }

    #[test]
    fn a_stout_ship_stands_up_to_more_guns() {
        let raider = sloop(4, 40);
        let fluyt = Strength {
            stoutness: 2.0,
            ..sloop(3, 40)
        };
        assert_eq!(defend(&fluyt, &raider), Tactic::Fight);
        assert_eq!(defend(&sloop(3, 40), &raider), Tactic::Flee);
    }

    #[test]
    fn a_battered_attacker_is_stood_up_to() {
        let merchant = sloop(2, 10);
        let raider = Strength {
            rigging: 0.3,
            ..sloop(4, 40)
        };
        assert_eq!(defend(&merchant, &raider), Tactic::Fight);
    }

    #[test]
    fn heavy_losses_beat_a_ship() {
        assert!(!sloop(2, 10).beaten());
        assert!(
            Strength {
                timbers: 0.2,
                ..sloop(2, 10)
            }
            .beaten()
        );
        assert!(
            Strength {
                rigging: 0.1,
                ..sloop(2, 10)
            }
            .beaten()
        );
        assert!(sloop(2, 3).beaten(), "too few hands");
    }

    #[test]
    fn a_fleeing_ship_runs_downwind_if_that_opens_the_distance() {
        let south = Vec3::new(0.0, 0.0, 500.0);
        // Wind from the south-south-west: downwind is north-north-east, away from the enemy.
        let wind_from = 200_f32.to_radians();
        let downwind = course(Tactic::Flee, Vec3::ZERO, 0.0, south, 0.0, wind_from);
        assert!((downwind - 20_f32.to_radians()).abs() < 1e-5);
        // Wind from the north: downwind is toward the enemy, so run straight away.
        let away = course(Tactic::Flee, Vec3::ZERO, 0.0, south, 0.0, 0.0);
        assert!(angle_between(away, 0.0).abs() < 1e-5);
    }

    #[test]
    fn a_fighting_ship_in_range_turns_her_broadside_on() {
        let enemy = Vec3::new(300.0, 0.0, 0.0);
        let course = course(Tactic::Fight, Vec3::ZERO, 0.2, enemy, 0.0, 0.0);
        assert!(bearing_broadside(course, Vec3::ZERO, enemy).is_some());
    }

    #[test]
    fn a_pursuer_makes_for_a_station_alongside_and_then_keeps_pace() {
        // Quarry heading north from the origin; pursuer far astern to starboard.
        let quarry = Vec3::ZERO;
        let far = Vec3::new(500.0, 0.0, 2000.0);
        let course_far = course(Tactic::Pursue, far, 0.0, quarry, 0.0, 0.0);
        let station = station_alongside(far, quarry, 0.0);
        assert!((course_far - direction_to_heading(station - far)).abs() < 1e-5);
        assert!(station.x > 0.0, "comes up on the side she's already on");

        let alongside = station + Vec3::new(10.0, 0.0, 10.0);
        assert_eq!(
            course(Tactic::Pursue, alongside, 0.0, quarry, 0.0, 0.0),
            0.0
        );
        assert_eq!(
            bearing_broadside(0.0, alongside, quarry),
            Some(Broadside::Port),
            "on station, her port broadside bears"
        );
    }
}
