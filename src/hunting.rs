//! Hunting: which ships a navy or a pirate goes after, and the chase.
//!
//! A navy hunts the ships of nations it's at war with, a captain her
//! nation wants, anyone it sees attacking its own or an ally's shipping,
//! and anyone diving on its nation's wrecks. A raider preys on any ship but a warship whose colours are
//! hostile to hers. Either one gives chase to the nearest quarry in sight,
//! and engages her at close range. A stern chase is a long chase: one
//! she's kept up for hours without taking her, she gives up, and lets that
//! quarry be for a day. A ship a warship would hunt runs from her, out of
//! sight.

use bevy::ecs::query::QueryData;
use bevy::ecs::system::SystemParam;
use bevy::math::FloatOrd;
use bevy::prelude::*;

use crate::alert::sighting_range;
use crate::clock::DAY;
use crate::combat::{DISENGAGE_DISTANCE, Engaged, Struck, Sunk};
use crate::crew::RESTING_AT_SEA;
use crate::factions::{Colours, GroupKind, Nation, Relations, Standing, Terms};
use crate::map::Land;
use crate::patrols::Patrol;
use crate::port::Docked;
use crate::tuning::TUNING;
use crate::wrecks::{SALVAGE_RANGE, Wreck};

/// A hunter engages her quarry within this range, meters: well inside the
/// range at which battles break off, so a chase doesn't flicker in and out
/// of action.
pub const CLOSE_ACTION: f32 = DISENGAGE_DISTANCE * 0.6;
/// A chase is steered afresh once her quarry has moved this far from where
/// the hunter is heading, meters.
const CHASE_SLACK: f32 = 300.0;
/// The courses a hunted ship tries, degrees off straight away from her
/// hunter, nearest first: within 90°, so each takes her farther off.
const BOLT_ANGLES: [f32; 6] = [0.0, 15.0, 30.0, 45.0, 60.0, 75.0];
/// She runs only for open water at least this far from land, meters.
const SEA_ROOM: f32 = 500.0;
/// A hunter who gives up a chase lets that quarry be this long. How long
/// she keeps one up, and how long a quarry lost from sight is still the
/// same chase, are tuned (see [`crate::tuning`]).
const LET_BE: f64 = DAY;

/// Who a hunter is after, and the quarry she's given up on. It isn't
/// saved: a loaded game's hunters start their chases afresh.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct Chase {
    after: Option<Hunted>,
    let_be: Option<(Entity, f64)>,
}

/// A chase: the quarry, when it began, and when she was last in sight.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Hunted {
    quarry: Entity,
    since: f64,
    seen: f64,
}

impl Chase {
    /// The quarry she's after, if any, though she may have lost sight of
    /// her for now.
    pub fn quarry(&self) -> Option<Entity> {
        self.after.map(|after| after.quarry)
    }

    /// The quarry she's letting be at `now`, if any.
    pub fn letting_be(&self, now: f64) -> Option<Entity> {
        self.let_be
            .filter(|&(_, until)| now < until)
            .map(|(quarry, _)| quarry)
    }

    /// Keeps after `quarry`, the ship in sight she'd go after, or nobody,
    /// at `now`, with her crew at `morale`: whether she's after a ship in
    /// sight. A quarry lost from sight is remembered a while, and the
    /// chase goes on if she's seen again. After a stern chase, as long as
    /// her crew will stand for, she gives up, and lets her be.
    pub fn keep_after(&mut self, quarry: Option<Entity>, now: f64, morale: f32) -> bool {
        let Some(quarry) = quarry else {
            self.after = self
                .after
                .filter(|after| now - after.seen <= TUNING.lost_sight);
            return false;
        };
        match self.after {
            Some(after) if after.quarry == quarry && now - after.seen <= TUNING.lost_sight => {
                if now - after.since > patience(morale) {
                    self.after = None;
                    self.let_be = Some((quarry, now + LET_BE));
                    return false;
                }
                self.after = Some(Hunted { seen: now, ..after });
            }
            _ => {
                self.after = Some(Hunted {
                    quarry,
                    since: now,
                    seen: now,
                });
            }
        }
        true
    }
}

/// How long a crew at `morale` will keep up a chase, seconds.
pub fn patience(morale: f32) -> f64 {
    let [least, most] = TUNING.patience;
    (TUNING.stern_chase * f64::from(morale / RESTING_AT_SEA)).clamp(least, most)
}

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

/// Where a ship at `at`, heading for `heading_for` if anywhere, runs to from
/// a hunter at `hunter`: out of sight, straight away from her if that's
/// open water, or as near it as there is. `None` if she's running
/// already, or the coast has her cornered.
pub fn evade(land: &Land, at: Vec3, heading_for: Option<Vec3>, hunter: Vec3) -> Option<Vec3> {
    let away = (at - hunter).normalize_or(Vec3::X);
    let running = heading_for.is_some_and(|end| {
        (end - at).dot(away) > 0.0
            && end.distance(hunter) > at.distance(hunter) + sighting_range() / 2.0
    });
    if running {
        return None;
    }
    BOLT_ANGLES
        .iter()
        .flat_map(|&degrees| [degrees, -degrees])
        .map(|degrees| at + Quat::from_rotation_y(degrees.to_radians()) * away * sighting_range())
        .find(|&bolt| land.clearance(bolt, bolt) >= SEA_ROOM)
}

/// A ship at sea, as a hunter sees her.
#[derive(QueryData)]
struct AtSea {
    entity: Entity,
    at: &'static Transform,
    colours: Option<&'static Colours>,
    warship: Has<Patrol>,
    engaged: Option<&'static Engaged>,
    standing: Option<&'static Standing>,
}

/// Ships afloat and under way: not in port, struck, or sunk.
type Afloat = (Without<Docked>, Without<Struck>, Without<Sunk>);

/// The ships at sea a hunter might go after.
#[derive(SystemParam)]
pub struct Sea<'w, 's> {
    ships: Query<'w, 's, AtSea, Afloat>,
    colours: Query<'w, 's, &'static Colours>,
    wrecks: Query<'w, 's, (&'static Transform, &'static Wreck)>,
    relations: Res<'w, Relations>,
}

impl Sea<'_, '_> {
    /// What a hunter can tell of a ship at sea.
    fn mark<'a>(&'a self, ship: &AtSeaItem<'a, '_>) -> Mark<'a> {
        let attacking = ship
            .engaged
            .filter(|engaged| engaged.aggressor)
            .and_then(|engaged| self.colours.get(engaged.enemy).ok())
            .map(|colours| colours.0);
        let salvaging = self
            .wrecks
            .iter()
            .find(|(lies, _)| lies.translation.distance(ship.at.translation) <= SALVAGE_RANGE)
            .and_then(|(_, wreck)| wreck.nation);
        Mark {
            colours: ship.colours.map(|colours| colours.0),
            warship: ship.warship,
            attacking,
            standing: ship.standing,
            salvaging,
        }
    }

    /// The nearest ship in sight of `at` that `hunter` goes after, and
    /// where she is: any but the one she's `letting_be`.
    pub fn nearest_quarry(
        &self,
        hunter: Hunter,
        at: Vec3,
        letting_be: Option<Entity>,
    ) -> Option<(Entity, Vec3)> {
        self.ships
            .iter()
            .filter(|ship| {
                Some(ship.entity) != letting_be
                    && ship.at.translation.distance(at) <= sighting_range()
            })
            .filter(|ship| is_quarry(hunter, &self.mark(ship), &self.relations))
            .map(|ship| (ship.entity, ship.at.translation))
            .min_by_key(|(_, position)| FloatOrd(position.distance(at)))
    }

    /// Where the nearest warship in sight of `ship` that would hunt her
    /// is, if there's one.
    pub fn nearest_hunter(&self, ship: Entity) -> Option<Vec3> {
        let quarry = self.ships.get(ship).ok()?;
        let mark = self.mark(&quarry);
        let at = quarry.at.translation;
        self.ships
            .iter()
            .filter(|hunter| {
                hunter.warship
                    && hunter.at.translation.distance(at) <= sighting_range()
                    && hunter.colours.is_some_and(|colours| {
                        is_quarry(Hunter::Navy(colours.0), &mark, &self.relations)
                    })
            })
            .map(|hunter| hunter.at.translation)
            .min_by_key(|hunter| FloatOrd(hunter.distance(at)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    use crate::map::LandMass;
    use crate::map::data::Terrain;

    #[test]
    fn a_hunter_gives_up_a_stern_chase_and_lets_that_quarry_be_a_day() {
        let (quarry, other) = (
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
        );
        let mut chase = Chase::default();
        // In sight all the while.
        for minutes in 0..=240 {
            assert!(chase.keep_after(Some(quarry), f64::from(minutes) * 60.0, RESTING_AT_SEA));
        }
        assert!(
            !chase.keep_after(Some(quarry), TUNING.stern_chase + 1.0, RESTING_AT_SEA),
            "gave up"
        );
        assert_eq!(chase.letting_be(TUNING.stern_chase + 2.0), Some(quarry));
        assert_eq!(chase.letting_be(TUNING.stern_chase + 2.0 + LET_BE), None);
        assert!(
            chase.keep_after(Some(other), TUNING.stern_chase + 2.0, RESTING_AT_SEA),
            "another is fair game"
        );
        assert!(!chase.keep_after(None, TUNING.stern_chase + 3.0, RESTING_AT_SEA));
        assert!(
            chase.keep_after(
                Some(other),
                TUNING.stern_chase + 3.0 + TUNING.lost_sight + 1.0,
                RESTING_AT_SEA
            ),
            "a chase long lost from sight starts afresh"
        );
        assert!(chase.keep_after(Some(other), 2.0 * TUNING.stern_chase, RESTING_AT_SEA));
    }

    #[test]
    fn an_eager_crew_keeps_up_a_chase_longer_than_a_sullen_one() {
        assert_eq!(
            patience(RESTING_AT_SEA),
            TUNING.stern_chase,
            "a steady crew"
        );
        assert!(patience(90.0) > patience(RESTING_AT_SEA));
        assert!(patience(25.0) < patience(RESTING_AT_SEA));
        assert_eq!(patience(0.0), TUNING.patience[0]);
        assert_eq!(patience(100.0), TUNING.patience[1]);
        // A sullen crew gives up two hours in, where a steady one wouldn't.
        let quarry = Entity::from_raw_u32(1).unwrap();
        let mut chase = Chase::default();
        assert!(chase.keep_after(Some(quarry), 0.0, 20.0));
        for minutes in 1..=120 {
            chase.keep_after(Some(quarry), f64::from(minutes) * 60.0, 20.0);
        }
        assert_eq!(
            chase.letting_be(2.0 * crate::clock::HOUR),
            Some(quarry),
            "gave up"
        );
    }

    #[test]
    fn a_quarry_slipping_out_of_sight_and_back_is_still_a_stern_chase() {
        let quarry = Entity::from_raw_u32(1).unwrap();
        let mut chase = Chase::default();
        let mut now = 0.0;
        // In sight, then over the horizon, every twenty minutes.
        while now <= TUNING.stern_chase {
            assert!(
                chase.keep_after(Some(quarry), now, RESTING_AT_SEA),
                "kept after her at {now}"
            );
            assert!(!chase.keep_after(None, now + 600.0, RESTING_AT_SEA));
            assert_eq!(
                chase.quarry(),
                Some(quarry),
                "remembered while out of sight"
            );
            now += 1_200.0;
        }
        assert!(
            !chase.keep_after(Some(quarry), now, RESTING_AT_SEA),
            "gave up"
        );
        assert_eq!(chase.letting_be(now), Some(quarry));
    }

    #[test]
    fn a_hunted_ship_runs_straight_away_until_she_is_running() {
        let sea = Land::default();
        let hunter = Vec3::ZERO;
        let at = Vec3::new(2_000.0, 0.0, 0.0);
        let bolt = evade(&sea, at, None, hunter).unwrap();
        assert_eq!(bolt, Vec3::new(2_000.0 + sighting_range(), 0.0, 0.0));
        assert_eq!(evade(&sea, at, Some(bolt), hunter), None, "already running");
        assert!(
            evade(&sea, at, Some(Vec3::new(-50_000.0, 0.0, 0.0)), hunter).is_some(),
            "heading home past the hunter is no escape"
        );
    }

    #[test]
    fn a_hunted_ship_runs_for_open_water_not_onto_the_coast() {
        let island = |x0: f32, x1: f32, z0: f32, z1: f32| {
            let outline = vec![
                Vec2::new(x0, z0),
                Vec2::new(x1, z0),
                Vec2::new(x1, z1),
                Vec2::new(x0, z1),
            ];
            LandMass::new("shore", Terrain::Sand, outline, None)
        };
        let hunter = Vec3::ZERO;
        let at = Vec3::new(2_000.0, 0.0, 0.0);
        // A coast dead ahead of her: she bears away for open water.
        let ahead = Land {
            masses: vec![island(6_000.0, 20_000.0, -2_000.0, 2_000.0)],
        };
        let bolt = evade(&ahead, at, None, hunter).unwrap();
        assert!(ahead.clearance(bolt, bolt) >= SEA_ROOM, "{bolt}");
        assert!(
            bolt.distance(hunter) > at.distance(hunter),
            "still farther off"
        );
        // Land all round ahead of her: she's cornered.
        let cornered = Land {
            masses: vec![island(3_000.0, 30_000.0, -30_000.0, 30_000.0)],
        };
        assert_eq!(evade(&cornered, at, None, hunter), None);
    }

    #[test]
    fn a_ship_is_hunted_only_by_a_warship_in_sight_that_would_go_after_her() {
        let mut world = World::new();
        world.init_resource::<Relations>();
        let pirate = world
            .spawn((Colours(Nation::PIRATES), Transform::default()))
            .id();
        let hunted = |world: &mut World| {
            world
                .run_system_once(move |sea: Sea| sea.nearest_hunter(pirate))
                .unwrap()
                .is_some()
        };
        let warship = |nation, x| {
            (
                Patrol::bound_for(0),
                Colours(nation),
                Transform::from_xyz(x, 0.0, 0.0),
            )
        };
        assert!(!hunted(&mut world));
        world.spawn(warship(Nation::ENGLAND, 1_000.0));
        assert!(!hunted(&mut world), "England isn't at war with them yet");
        let spaniard = world
            .spawn(warship(Nation::SPAIN, sighting_range() * 2.0))
            .id();
        assert!(!hunted(&mut world), "out of sight");
        world
            .entity_mut(spaniard)
            .insert(Transform::from_xyz(1_000.0, 0.0, 0.0));
        assert!(hunted(&mut world));
    }

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
