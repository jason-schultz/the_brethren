//! Nations, their colours, how they stand with each other, and the
//! player's standing with each.
//!
//! What ships do at sea is reported as [`Deed`]s where it happens:
//! attacking, taking, or sinking a nation's ship, or delivering its
//! contracts. The player's standing follows her deeds. Sink low enough with
//! a nation and she's wanted: its ports turn her away and its navy hunts
//! her. Every ship's deeds also move [`Relations`] between the colours she
//! flies and the nation she touched, so raiding a nation's shipping can
//! bring war. Bad blood fades slowly with time.

use std::fmt;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::alert::{Alert, AlertKind};
use crate::clock::DAY;
use crate::combat::Engaged;
use crate::player::Player;
use crate::schedule::{FrameSet, SimSet};

/// Every nation, in a fixed order.
pub const NATIONS: [Nation; 3] = [Nation::England, Nation::Spain, Nation::Pirates];
/// Standing at or below this with a nation means she's wanted by it.
pub const WANTED: f32 = -30.0;
/// Standing runs between these.
const LOWEST: f32 = -100.0;
const HIGHEST: f32 = 100.0;
/// Bad standing recovers this much a day.
const FORGETTING_PER_DAY: f32 = 1.0;
/// Nations at or below this with each other are at war...
pub const WAR: f32 = -50.0;
/// ...below this, hostile...
const HOSTILE: f32 = -25.0;
/// ...and at or above this, allies.
pub const ALLIANCE: f32 = 50.0;
/// How nations stood with each other in 1716, and drift back to: England
/// and Spain at an uneasy peace since Utrecht, both preyed on by the
/// Brethren, and Spain's guarda costas at war with them.
const RELATIONS_1716: [f32; PAIRS] = [-20.0, -30.0, -60.0];
/// Relations drift back to their 1716 footing this much a day.
const RELATIONS_DRIFT_PER_DAY: f32 = 0.5;
/// Pairs of different nations.
const PAIRS: usize = NATIONS.len() * (NATIONS.len() - 1) / 2;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Nation {
    England,
    Spain,
    /// The Brethren of the Coast.
    Pirates,
}

impl Nation {
    pub fn index(self) -> usize {
        match self {
            Self::England => 0,
            Self::Spain => 1,
            Self::Pirates => 2,
        }
    }

    /// Her colours in a word: "English colours".
    pub fn adjective(self) -> &'static str {
        match self {
            Self::England => "English",
            Self::Spain => "Spanish",
            Self::Pirates => "pirate",
        }
    }
}

impl fmt::Display for Nation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::England => "England",
            Self::Spain => "Spain",
            Self::Pirates => "the Brethren",
        })
    }
}

/// The colours a ship flies.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colours(pub Nation);

/// The player's standing with each nation.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Standing([f32; NATIONS.len()]);

impl Default for Standing {
    /// An English captain, in good standing at home.
    fn default() -> Self {
        Self([10.0, 0.0, 0.0])
    }
}

impl Standing {
    pub fn with(&self, nation: Nation) -> f32 {
        self.0[nation.index()]
    }

    pub fn set(&mut self, nation: Nation, value: f32) {
        self.0[nation.index()] = value.clamp(LOWEST, HIGHEST);
    }

    pub fn change(&mut self, nation: Nation, by: f32) {
        self.set(nation, self.with(nation) + by);
    }

    pub fn wanted_by(&self, nation: Nation) -> bool {
        self.with(nation) <= WANTED
    }

    /// Bad standing fades over `days`; good standing stays earned.
    pub fn forget(&mut self, days: f32) {
        for nation in NATIONS {
            let value = self.with(nation);
            if value < 0.0 {
                self.set(nation, (value + FORGETTING_PER_DAY * days).min(0.0));
            }
        }
    }
}

/// Every pair of different nations, once each.
pub fn pairs() -> impl Iterator<Item = [Nation; 2]> {
    NATIONS
        .into_iter()
        .enumerate()
        .flat_map(|(i, a)| NATIONS.into_iter().skip(i + 1).map(move |b| [a, b]))
}

/// How two nations stand with each other, from worst to best.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Terms {
    /// Their navies hunt each other's ships, and their ports turn them away.
    War,
    /// Pirates prey on a hostile nation's shipping.
    Hostile,
    Peace,
    /// Their navies guard each other's shipping.
    Alliance,
}

impl Terms {
    pub fn name(self) -> &'static str {
        match self {
            Self::War => "at war",
            Self::Hostile => "hostile",
            Self::Peace => "at peace",
            Self::Alliance => "allies",
        }
    }
}

/// How the nations stand with each other, from -100 to 100. Each pair has
/// one value, the same both ways.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Relations([f32; PAIRS]);

impl Default for Relations {
    fn default() -> Self {
        Self(RELATIONS_1716)
    }
}

impl Relations {
    /// The slot for two different nations.
    fn pair(a: Nation, b: Nation) -> Option<usize> {
        let (low, high) = (a.index().min(b.index()), a.index().max(b.index()));
        // Pairs in order: (0, 1), (0, 2), ..., (1, 2), ...
        (low != high).then(|| low * (2 * NATIONS.len() - low - 1) / 2 + high - low - 1)
    }

    /// A nation is her own best ally.
    pub fn between(&self, a: Nation, b: Nation) -> f32 {
        Self::pair(a, b).map_or(HIGHEST, |pair| self.0[pair])
    }

    pub fn set(&mut self, a: Nation, b: Nation, value: f32) {
        if let Some(pair) = Self::pair(a, b) {
            self.0[pair] = value.clamp(LOWEST, HIGHEST);
        }
    }

    pub fn change(&mut self, a: Nation, b: Nation, by: f32) {
        self.set(a, b, self.between(a, b) + by);
    }

    pub fn terms(&self, a: Nation, b: Nation) -> Terms {
        match self.between(a, b) {
            value if value <= WAR => Terms::War,
            value if value < HOSTILE => Terms::Hostile,
            value if value < ALLIANCE => Terms::Peace,
            _ => Terms::Alliance,
        }
    }

    pub fn at_war(&self, a: Nation, b: Nation) -> bool {
        self.terms(a, b) == Terms::War
    }

    /// Over `days`, every pair drifts back toward its 1716 footing.
    pub fn drift(&mut self, days: f32) {
        let step = RELATIONS_DRIFT_PER_DAY * days;
        for (value, rest) in self.0.iter_mut().zip(RELATIONS_1716) {
            *value += (rest - *value).clamp(-step, step);
        }
    }
}

/// Something a ship did that nations take note of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeedKind {
    /// Opened an engagement with one of its ships.
    Attacked,
    /// Took one of its ships.
    Captured,
    /// Sank one of its ships.
    Sank,
    /// Delivered a contract to one of its ports.
    Delivered,
    /// Lost her ship to mutiny or the law: word gets around.
    LostCommand,
    /// Lay in one of its ports while the town rose against it.
    RaisedPort,
}

/// `doer` did `kind` to or for `nation`.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deed {
    pub doer: Entity,
    pub kind: DeedKind,
    pub nation: Nation,
}

/// How a deed moves standing: with the nation it touched, and with the
/// Brethren, who admire a bold stroke against anyone else.
pub fn reputation(kind: DeedKind, nation: Nation) -> [(Nation, f32); 2] {
    let (with_nation, with_brethren) = match kind {
        DeedKind::Attacked => (-15.0, 3.0),
        DeedKind::Captured => (-20.0, 5.0),
        DeedKind::Sank => (-25.0, 5.0),
        DeedKind::Delivered => (3.0, 0.0),
        DeedKind::LostCommand => (-10.0, 0.0),
        DeedKind::RaisedPort => (-15.0, 5.0),
    };
    let brethren = if nation == Nation::Pirates {
        0.0
    } else {
        with_brethren
    };
    [(nation, with_nation), (Nation::Pirates, brethren)]
}

/// How a deed done under one nation's colours to another moves relations
/// between them. Nations already at war take no further offence.
pub fn diplomacy(kind: DeedKind) -> f32 {
    match kind {
        DeedKind::Attacked => -2.0,
        DeedKind::Captured => -3.0,
        DeedKind::Sank => -4.0,
        DeedKind::Delivered => 1.0,
        // That's about the captain, not her colours.
        DeedKind::LostCommand => 0.0,
        DeedKind::RaisedPort => -25.0,
    }
}

/// Changes relations between `doer` and `nation` for a deed of `kind`, and
/// reports whether that moved them to new terms.
fn record_diplomacy(
    relations: &mut Relations,
    doer: Nation,
    nation: Nation,
    kind: DeedKind,
) -> Option<Terms> {
    let by = diplomacy(kind);
    let before = relations.terms(doer, nation);
    if doer == nation || (by < 0.0 && before == Terms::War) {
        return None;
    }
    relations.change(doer, nation, by);
    let after = relations.terms(doer, nation);
    (after != before).then_some(after)
}

pub struct FactionsPlugin;

impl Plugin for FactionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Deed>()
            .add_message::<HoistColours>()
            .init_resource::<Standing>()
            .init_resource::<Relations>()
            .add_systems(
                Update,
                (record_deeds, hoist_colours.in_set(FrameSet::Orders)),
            )
            .add_systems(FixedUpdate, forget.in_set(SimSet::Resolve));
    }
}

/// Order a ship to fly a nation's colours. A ship in action has been seen
/// for what she is, and can't change them.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoistColours {
    pub ship: Entity,
    pub nation: Nation,
}

fn hoist_colours(
    mut orders: MessageReader<HoistColours>,
    ships: Query<(), (With<Colours>, Without<Engaged>)>,
    mut commands: Commands,
) {
    for order in orders.read() {
        if ships.contains(order.ship) {
            commands.entity(order.ship).insert(Colours(order.nation));
        }
    }
}

/// The player's deeds change her standing, and every ship's deeds under
/// colours change relations between her colours and the nation she
/// touched. The player hears when nations change terms.
fn record_deeds(
    mut deeds: MessageReader<Deed>,
    doers: Query<(Has<Player>, Option<&Colours>)>,
    player: Query<Entity, With<Player>>,
    mut standing: ResMut<Standing>,
    mut relations: ResMut<Relations>,
    mut alerts: MessageWriter<Alert>,
) {
    for deed in deeds.read() {
        let Ok((is_player, colours)) = doers.get(deed.doer) else {
            continue;
        };
        if is_player {
            for (nation, by) in reputation(deed.kind, deed.nation) {
                standing.change(nation, by);
            }
        }
        if let Some(&Colours(doer)) = colours
            && let Some(terms) = record_diplomacy(&mut relations, doer, deed.nation, deed.kind)
            && let Ok(ship) = player.single()
        {
            alerts.write(Alert {
                ship,
                kind: AlertKind::Terms {
                    nations: [doer, deed.nation],
                    terms,
                },
            });
        }
    }
}

/// Bad blood fades: the player's standing, and quarrels between nations.
fn forget(
    time: Res<Time>,
    mut standing: ResMut<Standing>,
    mut relations: ResMut<Relations>,
    player: Query<Entity, With<Player>>,
    mut alerts: MessageWriter<Alert>,
) {
    let days = (time.delta_secs_f64() / DAY) as f32;
    standing.forget(days);
    let before = *relations;
    relations.drift(days);
    for [a, b] in pairs() {
        let terms = relations.terms(a, b);
        if terms != before.terms(a, b)
            && let Ok(ship) = player.single()
        {
            alerts.write(Alert {
                ship,
                kind: AlertKind::Terms {
                    nations: [a, b],
                    terms,
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piracy_against_england_costs_there_and_pays_with_the_brethren() {
        let mut standing = Standing::default();
        for (nation, by) in reputation(DeedKind::Captured, Nation::England) {
            standing.change(nation, by);
        }
        assert!(standing.with(Nation::England) < 0.0);
        assert!(standing.with(Nation::Pirates) > 0.0);
        assert_eq!(standing.with(Nation::Spain), 0.0);
    }

    #[test]
    fn one_prize_is_a_warning_and_a_second_makes_her_wanted() {
        let mut standing = Standing::default();
        let take_a_prize = |standing: &mut Standing| {
            for kind in [DeedKind::Attacked, DeedKind::Captured] {
                for (nation, by) in reputation(kind, Nation::England) {
                    standing.change(nation, by);
                }
            }
        };
        take_a_prize(&mut standing);
        assert!(standing.with(Nation::England) < 0.0);
        assert!(!standing.wanted_by(Nation::England), "{standing:?}");
        take_a_prize(&mut standing);
        assert!(standing.wanted_by(Nation::England), "{standing:?}");
    }

    #[test]
    fn attacking_the_brethren_earns_no_credit_with_them() {
        let [(_, against), (_, credit)] = reputation(DeedKind::Attacked, Nation::Pirates);
        assert!(against < 0.0);
        assert_eq!(credit, 0.0);
    }

    #[test]
    fn every_pair_of_nations_has_its_own_relations_the_same_both_ways() {
        let mut relations = Relations::default();
        let mut slots = Vec::new();
        for a in NATIONS {
            for b in NATIONS {
                if a != b {
                    slots.push(Relations::pair(a, b).unwrap());
                    assert_eq!(Relations::pair(a, b), Relations::pair(b, a));
                }
            }
        }
        slots.sort();
        slots.dedup();
        assert_eq!(slots, (0..PAIRS).collect::<Vec<_>>());
        relations.set(Nation::Spain, Nation::Pirates, 70.0);
        assert_eq!(
            relations.terms(Nation::Pirates, Nation::Spain),
            Terms::Alliance
        );
        assert_eq!(relations.between(Nation::England, Nation::England), HIGHEST);
    }

    #[test]
    fn in_1716_the_brethren_prey_on_england_and_fight_spain() {
        let relations = Relations::default();
        assert_eq!(
            relations.terms(Nation::England, Nation::Spain),
            Terms::Peace
        );
        assert_eq!(
            relations.terms(Nation::England, Nation::Pirates),
            Terms::Hostile
        );
        assert!(relations.at_war(Nation::Spain, Nation::Pirates));
    }

    #[test]
    fn raiding_a_nation_brings_war_and_then_offends_no_further() {
        let mut relations = Relations::default();
        let took_a_prize = |relations: &mut Relations| {
            [DeedKind::Attacked, DeedKind::Captured]
                .map(|kind| record_diplomacy(relations, Nation::Pirates, Nation::England, kind))
                .into_iter()
                .flatten()
                .last()
        };
        assert_eq!(took_a_prize(&mut relations), None, "one prize isn't war");
        let declared = (0..10).find_map(|_| took_a_prize(&mut relations));
        assert_eq!(declared, Some(Terms::War));
        let at_war = relations.between(Nation::England, Nation::Pirates);
        took_a_prize(&mut relations);
        assert_eq!(relations.between(Nation::England, Nation::Pirates), at_war);
    }

    #[test]
    fn delivering_to_another_nation_warms_relations_and_quarrels_drift_back() {
        let mut relations = Relations::default();
        let before = relations.between(Nation::England, Nation::Pirates);
        record_diplomacy(
            &mut relations,
            Nation::Pirates,
            Nation::England,
            DeedKind::Delivered,
        );
        assert!(relations.between(Nation::England, Nation::Pirates) > before);
        assert_eq!(
            record_diplomacy(
                &mut relations,
                Nation::England,
                Nation::England,
                DeedKind::Sank
            ),
            None,
            "a nation can't fall out with herself"
        );
        relations.set(Nation::England, Nation::Spain, -80.0);
        relations.drift(20.0);
        assert_eq!(relations.between(Nation::England, Nation::Spain), -70.0);
        relations.drift(1000.0);
        assert_eq!(relations, Relations::default());
    }

    #[test]
    fn standing_is_bounded_and_bad_blood_fades() {
        let mut standing = Standing::default();
        standing.change(Nation::Spain, -500.0);
        assert_eq!(standing.with(Nation::Spain), LOWEST);
        standing.set(Nation::Spain, -5.0);
        standing.forget(2.0);
        assert_eq!(standing.with(Nation::Spain), -3.0);
        standing.forget(10.0);
        assert_eq!(
            standing.with(Nation::Spain),
            0.0,
            "fades to nothing, no further"
        );
        standing.forget(10.0);
        assert_eq!(standing.with(Nation::England), 10.0, "good standing stays");
    }
}
