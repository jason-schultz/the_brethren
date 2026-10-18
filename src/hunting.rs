//! Hunting: which ships a navy or a pirate goes after, and the chase.
//!
//! A navy hunts the ships of nations it's at war with, a captain her
//! nation wants, and anyone it sees attacking its own or an ally's
//! shipping. A raider preys on any ship but a warship whose colours are
//! hostile to hers. Either one gives chase to the nearest quarry in sight,
//! and engages her at close range.

use bevy::ecs::system::SystemParam;
use bevy::math::FloatOrd;
use bevy::prelude::*;

use crate::alert::SIGHTING_RANGE;
use crate::combat::{DISENGAGE_DISTANCE, Engaged, Struck, Sunk};
use crate::factions::{Colours, Nation, Relations, Standing, Terms};
use crate::patrols::Patrol;
use crate::player::Player;
use crate::port::Docked;

/// A hunter engages her quarry within this range, meters: well inside the
/// range at which battles break off, so a chase doesn't flicker in and out
/// of action.
pub const CLOSE_ACTION: f32 = DISENGAGE_DISTANCE * 0.6;
/// A chase is steered afresh once her quarry has moved this far from where
/// the hunter is heading, meters.
const CHASE_SLACK: f32 = 300.0;

/// Who is hunting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hunter {
    /// A navy ship, under a nation's colours.
    Navy(Nation),
    /// A pirate, under the colours of the Brethren or anyone she likes.
    Raider(Nation),
}

/// What a hunter can tell of a ship in sight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub colours: Option<Nation>,
    pub warship: bool,
    /// The colours of the ship she's attacking, if she started a fight.
    pub attacking: Option<Nation>,
    /// Whether she's the player, and so has standing to be wanted for.
    pub player: bool,
}

/// Whether `hunter` goes after a ship marked `mark`.
pub fn is_quarry(hunter: Hunter, mark: &Mark, relations: &Relations, standing: &Standing) -> bool {
    let (Hunter::Navy(nation) | Hunter::Raider(nation)) = hunter;
    if mark.colours == Some(nation) && !mark.player {
        return false;
    }
    let wanted = mark.player && standing.wanted_by(nation);
    let terms = mark.colours.map(|colours| relations.terms(nation, colours));
    match hunter {
        Hunter::Navy(_) => {
            // A nation counts as her own ally.
            let defending = mark
                .attacking
                .is_some_and(|victim| relations.terms(nation, victim) == Terms::Alliance);
            wanted || terms == Some(Terms::War) || defending
        }
        Hunter::Raider(_) => {
            wanted || (!mark.warship && terms.is_some_and(|terms| terms <= Terms::Hostile))
        }
    }
}

/// What a hunter should do about her quarry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pursuit {
    Engage,
    /// Steer for where she is now.
    SailTo(Vec3),
    /// Already heading her way.
    Hold,
}

/// How a hunter at `at`, heading for `heading_for` if anywhere, chases a
/// quarry at `quarry`.
pub fn pursue(at: Vec3, heading_for: Option<Vec3>, quarry: Vec3) -> Pursuit {
    if at.distance(quarry) <= CLOSE_ACTION {
        Pursuit::Engage
    } else if heading_for.is_none_or(|end| end.distance(quarry) > CHASE_SLACK) {
        Pursuit::SailTo(quarry)
    } else {
        Pursuit::Hold
    }
}

/// The ships at sea a hunter might go after.
#[derive(SystemParam)]
pub struct Sea<'w, 's> {
    ships: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            Option<&'static Colours>,
            Has<Patrol>,
            Option<&'static Engaged>,
            Has<Player>,
        ),
        (Without<Docked>, Without<Struck>, Without<Sunk>),
    >,
    colours: Query<'w, 's, &'static Colours>,
    relations: Res<'w, Relations>,
    standing: Res<'w, Standing>,
}

impl Sea<'_, '_> {
    /// The nearest ship in sight of `at` that `hunter` goes after, and
    /// where she is.
    pub fn nearest_quarry(&self, hunter: Hunter, at: Vec3) -> Option<(Entity, Vec3)> {
        self.ships
            .iter()
            .filter(|(_, transform, ..)| transform.translation.distance(at) <= SIGHTING_RANGE)
            .filter(|&(_, _, colours, warship, engaged, player)| {
                let attacking = engaged
                    .filter(|engaged| engaged.aggressor)
                    .and_then(|engaged| self.colours.get(engaged.enemy).ok())
                    .map(|colours| colours.0);
                let mark = Mark {
                    colours: colours.map(|colours| colours.0),
                    warship,
                    attacking,
                    player,
                };
                is_quarry(hunter, &mark, &self.relations, &self.standing)
            })
            .map(|(entity, transform, ..)| (entity, transform.translation))
            .min_by_key(|(_, position)| FloatOrd(position.distance(at)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(colours: Nation) -> Mark {
        Mark {
            colours: Some(colours),
            warship: false,
            attacking: None,
            player: false,
        }
    }

    #[test]
    fn navies_hunt_their_enemies_and_whoever_attacks_their_shipping() {
        let (relations, standing) = (Relations::default(), Standing::default());
        let spain = Hunter::Navy(Nation::Spain);
        let england = Hunter::Navy(Nation::England);
        assert!(is_quarry(
            spain,
            &mark(Nation::Pirates),
            &relations,
            &standing
        ));
        assert!(
            !is_quarry(england, &mark(Nation::Pirates), &relations, &standing),
            "England isn't at war with them yet"
        );
        let raiding = Mark {
            attacking: Some(Nation::England),
            ..mark(Nation::Pirates)
        };
        assert!(is_quarry(england, &raiding, &relations, &standing));
        assert!(!is_quarry(
            spain,
            &mark(Nation::Spain),
            &relations,
            &standing
        ));
    }

    #[test]
    fn raiders_prey_on_hostile_shipping_but_not_warships_or_friends() {
        let (mut relations, standing) = (Relations::default(), Standing::default());
        let raider = Hunter::Raider(Nation::Pirates);
        assert!(is_quarry(
            raider,
            &mark(Nation::England),
            &relations,
            &standing
        ));
        let warship = Mark {
            warship: true,
            ..mark(Nation::England)
        };
        assert!(!is_quarry(raider, &warship, &relations, &standing));
        relations.set(Nation::England, Nation::Pirates, 0.0);
        assert!(!is_quarry(
            raider,
            &mark(Nation::England),
            &relations,
            &standing
        ));
        assert!(!is_quarry(
            raider,
            &mark(Nation::Pirates),
            &relations,
            &standing
        ));
    }

    #[test]
    fn a_wanted_captain_is_hunted_under_any_colours() {
        let relations = Relations::default();
        let mut standing = Standing::default();
        standing.set(Nation::England, -50.0);
        let player = Mark {
            player: true,
            ..mark(Nation::England)
        };
        assert!(is_quarry(
            Hunter::Navy(Nation::England),
            &player,
            &relations,
            &standing
        ));
    }

    #[test]
    fn a_chase_steers_afresh_only_when_the_quarry_has_moved_on() {
        let at = Vec3::ZERO;
        let quarry = Vec3::new(5000.0, 0.0, 0.0);
        assert_eq!(pursue(at, None, quarry), Pursuit::SailTo(quarry));
        assert_eq!(pursue(at, Some(quarry), quarry), Pursuit::Hold);
        assert_eq!(
            pursue(at, Some(Vec3::new(-5000.0, 0.0, 0.0)), quarry),
            Pursuit::SailTo(quarry)
        );
        assert_eq!(pursue(quarry, None, quarry), Pursuit::Engage);
    }
}
