//! Hunting: which ships a navy or a pirate goes after, and the chase.
//!
//! A navy hunts the ships of nations it's at war with, a captain her
//! nation wants, anyone it sees attacking its own or an ally's shipping,
//! and anyone diving on its nation's wrecks. A raider preys on any ship but a warship whose colours are
//! hostile to hers. Either one gives chase to the nearest quarry in sight,
//! and engages her at close range.

use bevy::ecs::system::SystemParam;
use bevy::math::FloatOrd;
use bevy::prelude::*;

use crate::alert::SIGHTING_RANGE;
use crate::combat::{DISENGAGE_DISTANCE, Engaged, Struck, Sunk};
use crate::factions::{Colours, GroupKind, Nation, Relations, Standing, Terms};
use crate::patrols::Patrol;
use crate::port::Docked;
use crate::wrecks::{SALVAGE_RANGE, Wreck};

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

impl Hunter {
    /// The hunter a ship under `nation`'s colours, sailing for `serves`,
    /// is, if any: a crown's warships are its navy, the Brethren's are
    /// raiders, and merchantmen hunt no one.
    pub fn of(nation: Nation, serves: GroupKind) -> Option<Self> {
        match serves {
            GroupKind::Navy if nation.is_crown() => Some(Self::Navy(nation)),
            GroupKind::Navy => Some(Self::Raider(nation)),
            GroupKind::Governor | GroupKind::Merchants => None,
        }
    }
}

/// What a hunter can tell of a ship in sight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mark<'a> {
    pub colours: Option<Nation>,
    pub warship: bool,
    /// The colours of the ship she's attacking, if she started a fight.
    pub attacking: Option<Nation>,
    /// Her captain's name with the nations, if she has one to be wanted
    /// for.
    pub standing: Option<&'a Standing>,
    /// The nation whose wreck she's diving on, if she is.
    pub salvaging: Option<Nation>,
}

/// Whether `hunter` goes after a ship marked `mark`.
pub fn is_quarry(hunter: Hunter, mark: &Mark, relations: &Relations) -> bool {
    let (Hunter::Navy(nation) | Hunter::Raider(nation)) = hunter;
    let wanted = mark
        .standing
        .is_some_and(|standing| standing.wanted_by(nation));
    if mark.colours == Some(nation) && !wanted {
        return false;
    }
    let terms = mark.colours.map(|colours| relations.terms(nation, colours));
    match hunter {
        Hunter::Navy(_) => {
            // A nation counts as her own ally.
            let defending = mark
                .attacking
                .is_some_and(|victim| relations.terms(nation, victim) == Terms::Alliance);
            let robbing = mark.salvaging == Some(nation);
            wanted || terms == Some(Terms::War) || defending || robbing
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
            Option<&'static Standing>,
        ),
        (Without<Docked>, Without<Struck>, Without<Sunk>),
    >,
    colours: Query<'w, 's, &'static Colours>,
    wrecks: Query<'w, 's, (&'static Transform, &'static Wreck)>,
    relations: Res<'w, Relations>,
}

impl Sea<'_, '_> {
    /// The nearest ship in sight of `at` that `hunter` goes after, and
    /// where she is.
    pub fn nearest_quarry(&self, hunter: Hunter, at: Vec3) -> Option<(Entity, Vec3)> {
        self.ships
            .iter()
            .filter(|(_, transform, ..)| transform.translation.distance(at) <= SIGHTING_RANGE)
            .filter(|&(_, transform, colours, warship, engaged, standing)| {
                let attacking = engaged
                    .filter(|engaged| engaged.aggressor)
                    .and_then(|engaged| self.colours.get(engaged.enemy).ok())
                    .map(|colours| colours.0);
                let salvaging = self
                    .wrecks
                    .iter()
                    .find(|(lies, _)| {
                        lies.translation.distance(transform.translation) <= SALVAGE_RANGE
                    })
                    .and_then(|(_, wreck)| wreck.nation);
                let mark = Mark {
                    colours: colours.map(|colours| colours.0),
                    warship,
                    attacking,
                    standing,
                    salvaging,
                };
                is_quarry(hunter, &mark, &self.relations)
            })
            .map(|(entity, transform, ..)| (entity, transform.translation))
            .min_by_key(|(_, position)| FloatOrd(position.distance(at)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(colours: Nation) -> Mark<'static> {
        Mark {
            colours: Some(colours),
            warship: false,
            attacking: None,
            standing: None,
            salvaging: None,
        }
    }

    #[test]
    fn navies_hunt_their_enemies_and_whoever_attacks_their_shipping() {
        let relations = Relations::default();
        let spain = Hunter::Navy(Nation::SPAIN);
        let england = Hunter::Navy(Nation::ENGLAND);
        assert!(is_quarry(spain, &mark(Nation::PIRATES), &relations));
        assert!(
            !is_quarry(england, &mark(Nation::PIRATES), &relations),
            "England isn't at war with them yet"
        );
        let raiding = Mark {
            attacking: Some(Nation::ENGLAND),
            ..mark(Nation::PIRATES)
        };
        assert!(is_quarry(england, &raiding, &relations));
        assert!(!is_quarry(spain, &mark(Nation::SPAIN), &relations));
    }

    #[test]
    fn raiders_prey_on_hostile_shipping_but_not_warships_or_friends() {
        let mut relations = Relations::default();
        let raider = Hunter::Raider(Nation::PIRATES);
        assert!(is_quarry(raider, &mark(Nation::ENGLAND), &relations));
        let warship = Mark {
            warship: true,
            ..mark(Nation::ENGLAND)
        };
        assert!(!is_quarry(raider, &warship, &relations));
        relations.set(Nation::ENGLAND, Nation::PIRATES, 0.0);
        assert!(!is_quarry(raider, &mark(Nation::ENGLAND), &relations));
        assert!(!is_quarry(raider, &mark(Nation::PIRATES), &relations));
    }

    #[test]
    fn a_navy_hunts_whoever_dives_on_its_wrecks() {
        let relations = Relations::default();
        let diver = Mark {
            salvaging: Some(Nation::SPAIN),
            ..mark(Nation::ENGLAND)
        };
        assert!(is_quarry(Hunter::Navy(Nation::SPAIN), &diver, &relations));
        assert!(!is_quarry(
            Hunter::Navy(Nation::SPAIN),
            &mark(Nation::ENGLAND),
            &relations
        ));
    }

    #[test]
    fn a_wanted_captain_is_hunted_under_any_colours() {
        let relations = Relations::default();
        let mut standing = Standing::default();
        standing.set(Nation::ENGLAND, -50.0);
        let captain = Mark {
            standing: Some(&standing),
            ..mark(Nation::ENGLAND)
        };
        assert!(is_quarry(
            Hunter::Navy(Nation::ENGLAND),
            &captain,
            &relations
        ));
        let unknown = mark(Nation::ENGLAND);
        assert!(!is_quarry(
            Hunter::Navy(Nation::ENGLAND),
            &unknown,
            &relations
        ));
    }

    #[test]
    fn warships_hunt_and_merchantmen_dont() {
        use GroupKind::*;
        assert_eq!(
            Hunter::of(Nation::ENGLAND, Navy),
            Some(Hunter::Navy(Nation::ENGLAND))
        );
        assert_eq!(
            Hunter::of(Nation::PIRATES, Navy),
            Some(Hunter::Raider(Nation::PIRATES))
        );
        assert_eq!(Hunter::of(Nation::SPAIN, Merchants), None);
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
