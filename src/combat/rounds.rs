//! Battle by rounds: the abstract way to fight.
//!
//! Instead of sailing it out, the captain picks a move each round (close
//! in, fire a broadside, rake her rigging, break off) and the enemy answers
//! with her usual tactic. Range is a ladder of three steps. When one ship
//! closes and the other opens, the faster is likelier to have her way.
//! Gunfire is exchanged at the range a round starts at, with the same
//! damage as broadsides at sea and a roll of the dice on each.

use bevy::prelude::*;

use super::gunnery::{Damage, damage, hits};
use super::tactics::{BOARDING_RANGE, Strength, Tactic};

/// How far apart the ships are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Range {
    Alongside,
    Gunshot,
    Distant,
}

impl Range {
    /// The range ladder step for ships `distance` meters apart.
    pub fn at(distance: f32) -> Self {
        if distance <= BOARDING_RANGE {
            Self::Alongside
        } else if distance <= Self::Gunshot.distance() * 2.0 {
            Self::Gunshot
        } else {
            Self::Distant
        }
    }

    /// Meters apart, for gunnery.
    pub fn distance(self) -> f32 {
        match self {
            Self::Alongside => BOARDING_RANGE / 2.0,
            Self::Gunshot => 200.0,
            Self::Distant => 600.0,
        }
    }

    fn closer(self) -> Self {
        match self {
            Self::Distant => Self::Gunshot,
            _ => Self::Alongside,
        }
    }

    /// One step farther, or `None` past distant: out of reach.
    fn farther(self) -> Option<Self> {
        match self {
            Self::Alongside => Some(Self::Gunshot),
            Self::Gunshot => Some(Self::Distant),
            Self::Distant => None,
        }
    }
}

/// The captain's move for a round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// Close the range.
    Close,
    /// Fire into her hull and crew.
    Broadside,
    /// Fire high, into her rigging, to stop her running.
    Rake,
    /// Open the range, and get away if already distant.
    BreakOff,
}

/// One ship as a round sees her.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fighter {
    pub strength: Strength,
    /// Her best speed as she is, m/s.
    pub speed: f32,
}

/// What happened in a round.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Round {
    /// The new range, or `None` if the ships have drawn out of reach.
    pub range: Option<Range>,
    /// Damage done to us, and by us.
    pub taken: Damage,
    pub dealt: Damage,
}

/// A fleeing ship fires only her stern chasers: this fraction of a broadside.
const STERN_CHASERS: f32 = 0.5;
/// Raking doubles the damage aloft and halves it below.
const RAKE_ALOFT: f32 = 2.0;
const RAKE_BELOW: f32 = 0.5;

/// A tiny deterministic dice cup (xorshift), so rounds are repeatable.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dice(u64);

impl Default for Dice {
    fn default() -> Self {
        Self(0x9E37_79B9_7F4A_7C15)
    }
}

impl Dice {
    /// A roll in `[0, 1)`.
    pub fn roll(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Which way each side wants the range to go: negative closer, positive
/// farther.
fn wants(our_move: Move, their_tactic: Tactic, range: Range) -> (i8, i8) {
    let ours = match our_move {
        Move::Close => -1,
        Move::BreakOff => 1,
        Move::Broadside | Move::Rake => 0,
    };
    let theirs = match their_tactic {
        Tactic::Flee => 1,
        // Fighters and pursuers come within gunshot, and no closer.
        _ if range == Range::Distant => -1,
        _ => 0,
    };
    (ours, theirs)
}

/// Plays one round from `range`: our move against their tactic.
pub fn play(
    range: Range,
    our_move: Move,
    their_tactic: Tactic,
    us: &Fighter,
    them: &Fighter,
    dice: &mut Dice,
) -> Round {
    let (ours, theirs) = wants(our_move, their_tactic, range);
    let direction = if ours == theirs || theirs == 0 {
        ours
    } else if ours == 0 {
        theirs
    } else {
        // A tug of war: the faster ship is likelier to have her way.
        let our_chance = us.speed / (us.speed + them.speed).max(f32::EPSILON);
        if dice.roll() < our_chance {
            ours
        } else {
            theirs
        }
    };
    let new_range = match direction {
        d if d < 0 => Some(range.closer()),
        d if d > 0 => range.farther(),
        _ => Some(range),
    };
    // Shots are exchanged at the range the round starts at, while the ships
    // manoeuvre: a runner takes fire as she opens the distance.
    if range == Range::Distant {
        return Round {
            range: new_range,
            taken: Damage::default(),
            dealt: Damage::default(),
        };
    }
    let distance = range.distance();
    let mut volley =
        |guns: u32, share: f32| damage(hits(guns, distance) * share * (0.5 + dice.roll()));
    let dealt = match our_move {
        Move::Broadside => volley(us.strength.guns_per_side, 1.0),
        Move::Rake => {
            let hit = volley(us.strength.guns_per_side, 1.0);
            Damage {
                timbers: hit.timbers * RAKE_BELOW,
                rigging: hit.rigging * RAKE_ALOFT,
                men: hit.men * RAKE_BELOW as f64,
            }
        }
        Move::Close | Move::BreakOff => Damage::default(),
    };
    let share = if their_tactic == Tactic::Flee {
        STERN_CHASERS
    } else {
        1.0
    };
    let taken = volley(them.strength.guns_per_side, share);
    Round {
        range: new_range,
        taken,
        dealt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fighter(guns_per_side: u32, speed: f32) -> Fighter {
        Fighter {
            strength: Strength {
                guns_per_side,
                crew: 12,
                min_crew: 8,
                stoutness: 1.0,
                timbers: 1.0,
                rigging: 1.0,
            },
            speed,
        }
    }

    #[test]
    fn the_range_ladder_matches_distances() {
        assert_eq!(Range::at(30.0), Range::Alongside);
        assert_eq!(Range::at(300.0), Range::Gunshot);
        assert_eq!(Range::at(2000.0), Range::Distant);
    }

    #[test]
    fn dice_roll_within_bounds_and_vary() {
        let mut dice = Dice::default();
        let rolls: Vec<f32> = (0..1000).map(|_| dice.roll()).collect();
        assert!(rolls.iter().all(|r| (0.0..1.0).contains(r)));
        let mean = rolls.iter().sum::<f32>() / rolls.len() as f32;
        assert!((0.4..0.6).contains(&mean), "mean {mean}");
    }

    #[test]
    fn closing_on_a_willing_enemy_always_works() {
        let mut dice = Dice::default();
        let round = play(
            Range::Distant,
            Move::Close,
            Tactic::Fight,
            &fighter(4, 5.0),
            &fighter(2, 5.0),
            &mut dice,
        );
        assert_eq!(round.range, Some(Range::Gunshot));
    }

    #[test]
    fn a_faster_ship_usually_runs_down_a_fleeing_one() {
        let mut dice = Dice::default();
        let (us, them) = (fighter(4, 6.0), fighter(2, 4.0));
        let caught = (0..1000)
            .filter(|_| {
                play(
                    Range::Gunshot,
                    Move::Close,
                    Tactic::Flee,
                    &us,
                    &them,
                    &mut dice,
                )
                .range
                    == Some(Range::Alongside)
            })
            .count();
        assert!((550..650).contains(&caught), "caught {caught} of 1000");
    }

    #[test]
    fn nobody_fires_out_of_gunshot() {
        let mut dice = Dice::default();
        let round = play(
            Range::Distant,
            Move::Broadside,
            Tactic::Flee,
            &fighter(4, 5.0),
            &fighter(2, 5.0),
            &mut dice,
        );
        assert_eq!(round.range, None, "she ran out of reach");
        assert_eq!(round.dealt, Damage::default());
        assert_eq!(round.taken, Damage::default());
    }

    #[test]
    fn a_broadside_hurts_and_a_fleeing_ship_fires_back_lightly() {
        let mut dice = Dice::default();
        let (us, them) = (fighter(4, 5.0), fighter(4, 5.0));
        let mut dealt = 0.0;
        let mut taken = 0.0;
        for _ in 0..200 {
            let round = play(
                Range::Gunshot,
                Move::Broadside,
                Tactic::Flee,
                &us,
                &them,
                &mut dice,
            );
            dealt += round.dealt.timbers;
            taken += round.taken.timbers;
        }
        assert!(dealt > 0.0 && taken > 0.0);
        assert!(
            taken < dealt * 0.7,
            "stern chasers only: {taken} vs {dealt}"
        );
    }

    #[test]
    fn raking_goes_for_the_rigging() {
        let mut dice = Dice::default();
        let (us, them) = (fighter(4, 5.0), fighter(4, 5.0));
        let rake = play(
            Range::Gunshot,
            Move::Rake,
            Tactic::Fight,
            &us,
            &them,
            &mut dice,
        )
        .dealt;
        let mut dice = Dice::default();
        let broadside = play(
            Range::Gunshot,
            Move::Broadside,
            Tactic::Fight,
            &us,
            &them,
            &mut dice,
        )
        .dealt;
        assert!(rake.rigging > broadside.rigging && rake.timbers < broadside.timbers);
    }
}
