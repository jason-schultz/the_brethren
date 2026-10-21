//! Nations, the groups they're made of, their colours, how they stand with
//! each other, and each captain's standing with each group.
//!
//! What ships do at sea is reported as [`Deed`]s where it happens:
//! attacking, taking, or sinking a nation's ship, or delivering its
//! contracts. Each deed touches one [`Group`] of a nation. As word of it
//! reaches each nation (see [`crate::word`]), the [`Standing`] of the
//! captain who did it follows, rippling to the rest of the nation
//! and to its friends and enemies. Her standing with a nation is the mean
//! of its groups'. Sink low enough with a nation and she's wanted: its
//! ports turn her away and its navy hunts her. Every ship's deeds also move
//! [`Relations`] between the colours she flies and the nation she touched,
//! so raiding a nation's shipping can bring war. Bad blood fades slowly
//! with time.
//!
//! The nations, their groups, and what each deed is worth are data
//! (`assets/data/factions.ron`, read by [`data`]): adding a nation is
//! adding an entry there.

use std::fmt;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::alert::{Alert, AlertKind};
use crate::clock::DAY;
use crate::combat::Engaged;
use crate::goods::Carriage;
use crate::player::Player;
use crate::schedule::{FrameSet, SimSet};
use crate::service::Service;
use crate::word::{DeedKnown, Happening, Knowledge, Ledger};
pub use data::factions;
use data::{Fleet, GroupData, NationData};

pub mod data;

/// Standing at or below this with a nation means she's wanted by it.
pub const WANTED: f32 = -30.0;
/// Standing runs between these.
pub const LOWEST: f32 = -100.0;
pub const HIGHEST: f32 = 100.0;
/// Bad standing recovers this much a day.
const FORGETTING_PER_DAY: f32 = 1.0;
/// Nations at or below this with each other are at war...
pub const WAR: f32 = -50.0;
/// ...below this, hostile...
const HOSTILE: f32 = -25.0;
/// ...and at or above this, allies.
pub const ALLIANCE: f32 = 50.0;
/// Relations drift back to their starting footing this much a day.
const RELATIONS_DRIFT_PER_DAY: f32 = 0.5;
/// The kinds of group every nation is made of, in a fixed order.
pub const GROUP_KINDS: [GroupKind; 3] =
    [GroupKind::Navy, GroupKind::Governor, GroupKind::Merchants];

/// A nation, as the factions data lists it (see [`data`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Nation(u8);

/// Every nation, in the data's order.
pub fn nations() -> impl Iterator<Item = Nation> + Clone {
    (0..factions().nations.len()).map(|index| Nation(index as u8))
}

/// How many nations there are.
pub fn nation_count() -> usize {
    factions().nations.len()
}

#[cfg(test)]
impl Nation {
    pub const ENGLAND: Self = Self(0);
    pub const SPAIN: Self = Self(1);
    /// The Brethren of the Coast.
    pub const PIRATES: Self = Self(2);
}

impl Nation {
    /// The nation the data keys `key`.
    pub fn named(key: &str) -> Option<Self> {
        factions().index(key).map(|index| Self(index as u8))
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }

    fn data(self) -> &'static NationData {
        &factions().nations[self.index()]
    }

    /// How maps and saves name it.
    pub fn key(self) -> &'static str {
        &self.data().key
    }

    /// Whether she has a king's law, as a crown does and the Brethren
    /// don't: pardons, and a navy that patrols rather than raids.
    pub fn is_crown(self) -> bool {
        self.data().lawful
    }

    /// Her three groups.
    pub fn groups(self) -> [Group; GROUP_KINDS.len()] {
        GROUP_KINDS.map(|kind| Group { nation: self, kind })
    }

    /// Her colours in a word: "English colours".
    pub fn adjective(self) -> &'static str {
        &self.data().adjective
    }

    /// The colour she's shown in, as sRGB.
    pub fn colour(self) -> (f32, f32, f32) {
        self.data().colour
    }

    /// The day of her first Act of Grace, if she proclaims them.
    pub fn first_grace_day(self) -> Option<f64> {
        self.data().grace
    }

    /// The player's standing with each of her groups at the start.
    pub fn starting_standing(self) -> f32 {
        self.data().standing
    }

    /// The chances an officer hired in her taverns is loyal, ambitious, or
    /// a turncoat.
    pub fn officer_odds(self) -> [f32; 3] {
        let (loyal, ambitious, turncoat) = self.data().officers;
        [loyal, ambitious, turncoat]
    }

    /// Her ships, by name.
    pub fn fleet(self) -> &'static Fleet {
        &self.data().fleet
    }
}

impl fmt::Display for Nation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.data().name)
    }
}

impl fmt::Debug for Nation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.key())
    }
}

impl Serialize for Nation {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

impl<'de> Deserialize<'de> for Nation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = String::deserialize(deserializer)?;
        Self::named(&key).ok_or_else(|| serde::de::Error::custom(format!("unknown nation {key}")))
    }
}

/// What a group of a nation is.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupKind {
    /// Her warships: the crowns' patrols, or the Brethren's raiders.
    Navy,
    /// Her governors, who hold her ports and keep her law.
    Governor,
    /// Her traders and their freight.
    Merchants,
}

impl GroupKind {
    fn index(self) -> usize {
        match self {
            Self::Navy => 0,
            Self::Governor => 1,
            Self::Merchants => 2,
        }
    }

    /// Whose business a contract for goods of `carriage` is: freight is
    /// the merchants', and soldiers the governor's.
    pub fn for_carriage(carriage: Carriage) -> Self {
        match carriage {
            Carriage::Hold => Self::Merchants,
            Carriage::Berths => Self::Governor,
        }
    }
}

/// One of the groups a nation is made of.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Group {
    pub nation: Nation,
    pub kind: GroupKind,
}

impl Group {
    /// The group a ship flying `colours` and sailing for `serves` belongs
    /// to: none, if she flies no colours.
    pub fn of(colours: Option<&Colours>, serves: Option<&Serves>) -> Option<Self> {
        Some(Self {
            nation: colours?.0,
            kind: serves.map_or(GroupKind::Merchants, |serves| serves.0),
        })
    }

    fn data(self) -> &'static GroupData {
        let nation = self.nation.data();
        match self.kind {
            GroupKind::Navy => &nation.navy,
            GroupKind::Governor => &nation.governor,
            GroupKind::Merchants => &nation.merchants,
        }
    }

    /// The title of rank `rank` in it, from 0: "Lieutenant".
    pub fn rank_title(self, rank: usize) -> &'static str {
        &self.data().ranks[rank]
    }

    /// Why it says a new captain should join it.
    pub fn pitch(self) -> &'static str {
        &self.data().pitch
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.data().name)
    }
}

/// Every group of every nation.
pub fn groups() -> impl Iterator<Item = Group> {
    nations().flat_map(Nation::groups)
}

/// The colours a ship flies.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colours(pub Nation);

/// The group of her nation an NPC ship sails for. A ship without one
/// counts as one of her nation's merchantmen.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Serves(pub GroupKind);

/// A captain's standing with each group of each nation: her name with
/// them, kept on the ship she commands. Any captain can have one; for now
/// only the player does, and NPC captains' deeds move only relations.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Standing(Vec<[f32; GROUP_KINDS.len()]>);

/// Who a captain is, whatever ship she's in: deeds remember her by it.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CaptainId(pub u32);

/// Everything a captain carries from ship to ship: who she is, her
/// standing, the groups she serves, and what she's heard. It lives on the
/// ship she commands, and moves with her when she changes ships.
pub type CaptainRecord = (CaptainId, Standing, Service, Knowledge);

impl Default for Standing {
    /// A new captain, as the data has her start.
    fn default() -> Self {
        Self(
            nations()
                .map(|nation| [nation.starting_standing(); GROUP_KINDS.len()])
                .collect(),
        )
    }
}

impl Standing {
    pub fn of(&self, group: Group) -> f32 {
        self.0[group.nation.index()][group.kind.index()]
    }

    pub fn set_group(&mut self, group: Group, value: f32) {
        self.0[group.nation.index()][group.kind.index()] = value.clamp(LOWEST, HIGHEST);
    }

    pub fn change_group(&mut self, group: Group, by: f32) {
        self.set_group(group, self.of(group) + by);
    }

    /// With a nation as a whole: the mean of its groups'.
    pub fn with(&self, nation: Nation) -> f32 {
        let groups = nation.groups();
        groups.iter().map(|&group| self.of(group)).sum::<f32>() / groups.len() as f32
    }

    /// Sets every one of a nation's groups to `value`.
    pub fn set(&mut self, nation: Nation, value: f32) {
        for group in nation.groups() {
            self.set_group(group, value);
        }
    }

    /// Raises every one of a nation's groups to at least `floor`.
    pub fn lift(&mut self, nation: Nation, floor: f32) {
        for group in nation.groups() {
            self.set_group(group, self.of(group).max(floor));
        }
    }

    pub fn wanted_by(&self, nation: Nation) -> bool {
        self.with(nation) <= WANTED
    }

    /// Bad standing fades over `days`; good standing stays earned.
    pub fn forget(&mut self, days: f32) {
        for group in groups() {
            let value = self.of(group);
            if value < 0.0 {
                self.set_group(group, (value + FORGETTING_PER_DAY * days).min(0.0));
            }
        }
    }
}

/// Every pair of different nations, once each.
pub fn pairs() -> impl Iterator<Item = [Nation; 2]> {
    nations().flat_map(|a| nations().filter(move |&b| b > a).map(move |b| [a, b]))
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
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Relations(Vec<f32>);

impl Default for Relations {
    /// As the data has them at the start.
    fn default() -> Self {
        Self(
            pairs()
                .map(|[a, b]| factions().starting_relations(a.index(), b.index()))
                .collect(),
        )
    }
}

impl Relations {
    /// The slot for two different nations.
    fn pair(a: Nation, b: Nation) -> Option<usize> {
        let (low, high) = (a.index().min(b.index()), a.index().max(b.index()));
        // Pairs in order: (0, 1), (0, 2), ..., (1, 2), ...
        (low != high).then(|| low * (2 * nation_count() - low - 1) / 2 + high - low - 1)
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

    /// Over `days`, every pair drifts back toward its starting footing.
    pub fn drift(&mut self, days: f32) {
        let step = RELATIONS_DRIFT_PER_DAY * days;
        for ([a, b], value) in pairs().zip(self.0.iter_mut()) {
            let rest = factions().starting_relations(a.index(), b.index());
            *value += (rest - *value).clamp(-step, step);
        }
    }
}

/// Something a ship did that nations take note of. What each is worth is
/// data (see [`data::Worth`]).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Ran a group's errand for a new captain: it knows her now.
    Introduced,
    /// Saw one of its merchantmen safe into port.
    Escorted,
    /// Lost one of its merchantmen she'd been paid to guard.
    LostWard,
    /// Salvaged the wreck of one of its ships, which is its by right.
    Salvaged,
    /// Gave a false account of how one of its ships was lost, and was
    /// found out.
    FalseReport,
}

impl DeedKind {
    /// Every kind of deed.
    pub const ALL: [Self; 11] = [
        Self::Attacked,
        Self::Captured,
        Self::Sank,
        Self::Delivered,
        Self::LostCommand,
        Self::RaisedPort,
        Self::Introduced,
        Self::Escorted,
        Self::LostWard,
        Self::Salvaged,
        Self::FalseReport,
    ];

    /// How much it moves standing with the group it touched.
    pub fn standing(self) -> f32 {
        factions().worth(self).standing
    }

    /// Whether it's a blow struck against a nation, which its enemies
    /// applaud and its allies resent.
    pub fn is_blow(self) -> bool {
        factions().worth(self).blow
    }

    /// How far a fight went, if it's one: an attack, a taking, a sinking.
    /// A fight that goes further is one deed, not two.
    pub fn escalation(self) -> Option<u8> {
        match self {
            Self::Attacked => Some(0),
            Self::Captured => Some(1),
            Self::Sank => Some(2),
            _ => None,
        }
    }
}

/// `doer` did `kind` to or for `group`, to the ship `about` if any. It goes
/// in the ledger of news, and moves standing and relations as nations hear
/// of it (see [`crate::word`]).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deed {
    pub doer: Entity,
    pub kind: DeedKind,
    pub group: Group,
    pub about: Option<Entity>,
}

impl Deed {
    /// `doer` did `kind` to the ship `victim`, flying `colours` and sailing
    /// for `serves`. Nothing, if she flies no colours.
    pub fn to_ship(
        doer: Entity,
        kind: DeedKind,
        victim: Entity,
        colours: Option<&Colours>,
        serves: Option<&Serves>,
    ) -> Option<Self> {
        Some(Self {
            doer,
            kind,
            group: Group::of(colours, serves)?,
            about: Some(victim),
        })
    }
}

/// How a deed of `kind` that touched `group` moves standing. The group
/// takes it all, and the rest of its nation half. A blow against a nation
/// also earns credit with its enemies' navies, and offends its allies'.
pub fn reputation(kind: DeedKind, group: Group, relations: &Relations) -> Vec<(Group, f32)> {
    let by = kind.standing();
    let mut changes: Vec<(Group, f32)> = group
        .nation
        .groups()
        .into_iter()
        .map(|kin| {
            (
                kin,
                if kin == group {
                    by
                } else {
                    by * factions().reputation.kin_share
                },
            )
        })
        .collect();
    if kind.is_blow() {
        for other in nations().filter(|&other| other != group.nation) {
            let share = match relations.terms(other, group.nation) {
                Terms::War | Terms::Hostile => -factions().reputation.neighbour_share,
                Terms::Peace => continue,
                Terms::Alliance => factions().reputation.neighbour_share,
            };
            let navy = Group {
                nation: other,
                kind: GroupKind::Navy,
            };
            changes.push((navy, by * share));
        }
    }
    changes
}

/// How a deed done under one nation's colours to another moves relations
/// between them. Nations already at war take no further offence.
pub fn diplomacy(kind: DeedKind) -> f32 {
    factions().worth(kind).diplomacy
}

/// Changes relations between `doer` and `nation` for a deed of `kind`,
/// less what the lesser deed of the same fight, `answered`, already did,
/// and reports whether that moved them to new terms.
fn record_diplomacy(
    relations: &mut Relations,
    doer: Nation,
    nation: Nation,
    kind: DeedKind,
    answered: Option<DeedKind>,
) -> Option<Terms> {
    let by = diplomacy(kind) - answered.map_or(0.0, diplomacy);
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
            .init_resource::<Relations>()
            .add_systems(
                Update,
                (hear_of_deeds, hoist_colours.in_set(FrameSet::Orders)),
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

/// As a nation hears of a deed, it moves the doer's captain's standing
/// with that nation's groups, and if the deed was done to that nation,
/// relations between it and the doer's colours. The player hears when
/// nations change terms.
fn hear_of_deeds(
    mut known: MessageReader<DeedKnown>,
    ledger: Res<Ledger>,
    mut captains: Query<(Entity, &CaptainId, &mut Standing)>,
    player: Query<Entity, With<Player>>,
    mut relations: ResMut<Relations>,
    mut alerts: MessageWriter<Alert>,
) {
    for &DeedKnown {
        news,
        nation,
        answered,
    } in known.read()
    {
        let Some(Happening::Deed(deed)) = ledger.get(news).map(|news| &news.event) else {
            continue;
        };
        if let Some((captain, _, mut standing)) = captains
            .iter_mut()
            .find(|(_, id, _)| Some(**id) == deed.captain)
        {
            for (group, by) in deed.standing_for(nation, answered) {
                standing.change_group(group, by);
            }
            let tells_against = deed.kind.is_blow() || deed.kind == DeedKind::FalseReport;
            if tells_against && nation == deed.group.nation {
                alerts.write(Alert {
                    ship: captain,
                    kind: AlertKind::WordOut {
                        nation,
                        kind: deed.kind,
                        victim: deed.victim.as_ref().map(|(ship, _)| *ship),
                    },
                });
            }
        }
        if nation != deed.group.nation {
            continue;
        }
        if let Some(doer) = deed.colours
            && let Some(terms) = record_diplomacy(&mut relations, doer, nation, deed.kind, answered)
            && let Ok(ship) = player.single()
        {
            alerts.write(Alert {
                ship,
                kind: AlertKind::Terms {
                    nations: [doer, nation],
                    terms,
                },
            });
        }
    }
}

/// Bad blood fades: every captain's standing, and quarrels between nations.
fn forget(
    time: Res<Time>,
    mut captains: Query<&mut Standing>,
    mut relations: ResMut<Relations>,
    player: Query<Entity, With<Player>>,
    mut alerts: MessageWriter<Alert>,
) {
    let days = (time.delta_secs_f64() / DAY) as f32;
    for mut standing in &mut captains {
        standing.forget(days);
    }
    let before = relations.clone();
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

    const JAMAICA_MERCHANTS: Group = Group {
        nation: Nation::ENGLAND,
        kind: GroupKind::Merchants,
    };

    fn record(standing: &mut Standing, kind: DeedKind, group: Group, relations: &Relations) {
        for (group, by) in reputation(kind, group, relations) {
            standing.change_group(group, by);
        }
    }

    #[test]
    fn piracy_against_england_costs_there_and_pays_with_the_brethren() {
        let mut standing = Standing::default();
        let relations = Relations::default();
        record(
            &mut standing,
            DeedKind::Captured,
            JAMAICA_MERCHANTS,
            &relations,
        );
        assert!(standing.with(Nation::ENGLAND) < 0.0);
        assert!(standing.with(Nation::PIRATES) > 0.0);
        assert_eq!(standing.with(Nation::SPAIN), 0.0, "Spain is at peace");
    }

    #[test]
    fn a_deed_falls_hardest_on_the_group_it_touched() {
        let mut standing = Standing::default();
        standing.set(Nation::ENGLAND, 0.0);
        record(
            &mut standing,
            DeedKind::Sank,
            JAMAICA_MERCHANTS,
            &Relations::default(),
        );
        let [navy, governor, merchants] = Nation::ENGLAND.groups().map(|g| standing.of(g));
        assert_eq!(merchants, -25.0);
        assert_eq!([navy, governor], [-12.5, -12.5]);
        assert_eq!(standing.with(Nation::ENGLAND), (-25.0 - 25.0) / 3.0);
    }

    #[test]
    fn a_blow_against_a_nation_earns_credit_with_its_enemies_and_offends_its_allies() {
        let mut relations = Relations::default();
        relations.set(Nation::ENGLAND, Nation::SPAIN, ALLIANCE);
        let flying_gang = Group {
            nation: Nation::PIRATES,
            kind: GroupKind::Navy,
        };
        let changes = reputation(DeedKind::Sank, flying_gang, &relations);
        let navy = |nation| Group {
            nation,
            kind: GroupKind::Navy,
        };
        let change = |group| changes.iter().find(|(g, _)| *g == group).map(|c| c.1);
        assert!(change(navy(Nation::ENGLAND)) > Some(0.0));
        assert!(change(navy(Nation::SPAIN)) > Some(0.0));
        assert_eq!(change(flying_gang), Some(-25.0));

        let changes = reputation(DeedKind::Sank, JAMAICA_MERCHANTS, &relations);
        let spain = changes.iter().find(|(g, _)| *g == navy(Nation::SPAIN));
        assert!(spain.is_some_and(|&(_, by)| by < 0.0), "England's ally");
        let delivered = reputation(DeedKind::Delivered, JAMAICA_MERCHANTS, &relations);
        assert!(
            delivered.iter().all(|(g, _)| g.nation == Nation::ENGLAND),
            "only blows ripple abroad"
        );
    }

    #[test]
    fn one_prize_is_a_warning_and_a_second_makes_her_wanted() {
        let mut standing = Standing::default();
        let relations = Relations::default();
        let take_a_prize = |standing: &mut Standing| {
            for kind in [DeedKind::Attacked, DeedKind::Captured] {
                record(standing, kind, JAMAICA_MERCHANTS, &relations);
            }
        };
        take_a_prize(&mut standing);
        assert!(standing.with(Nation::ENGLAND) < 0.0);
        assert!(!standing.wanted_by(Nation::ENGLAND), "{standing:?}");
        take_a_prize(&mut standing);
        assert!(standing.wanted_by(Nation::ENGLAND), "{standing:?}");
    }

    #[test]
    fn attacking_the_brethren_earns_no_credit_with_them() {
        let raider = Group {
            nation: Nation::PIRATES,
            kind: GroupKind::Navy,
        };
        for (group, by) in reputation(DeedKind::Attacked, raider, &Relations::default()) {
            assert_eq!(by < 0.0, group.nation == Nation::PIRATES, "{group}");
        }
    }

    #[test]
    fn every_pair_of_nations_has_its_own_relations_the_same_both_ways() {
        let mut relations = Relations::default();
        let mut slots = Vec::new();
        for a in nations() {
            for b in nations() {
                if a != b {
                    slots.push(Relations::pair(a, b).unwrap());
                    assert_eq!(Relations::pair(a, b), Relations::pair(b, a));
                }
            }
        }
        slots.sort();
        slots.dedup();
        assert_eq!(slots, (0..pairs().count()).collect::<Vec<_>>());
        relations.set(Nation::SPAIN, Nation::PIRATES, 70.0);
        assert_eq!(
            relations.terms(Nation::PIRATES, Nation::SPAIN),
            Terms::Alliance
        );
        assert_eq!(relations.between(Nation::ENGLAND, Nation::ENGLAND), HIGHEST);
    }

    #[test]
    fn in_1716_the_brethren_prey_on_england_and_fight_spain() {
        let relations = Relations::default();
        assert_eq!(
            relations.terms(Nation::ENGLAND, Nation::SPAIN),
            Terms::Peace
        );
        assert_eq!(
            relations.terms(Nation::ENGLAND, Nation::PIRATES),
            Terms::Hostile
        );
        assert!(relations.at_war(Nation::SPAIN, Nation::PIRATES));
    }

    #[test]
    fn raiding_a_nation_brings_war_and_then_offends_no_further() {
        let mut relations = Relations::default();
        let took_a_prize = |relations: &mut Relations| {
            [
                (DeedKind::Attacked, None),
                (DeedKind::Captured, Some(DeedKind::Attacked)),
            ]
            .map(|(kind, answered)| {
                record_diplomacy(relations, Nation::PIRATES, Nation::ENGLAND, kind, answered)
            })
            .into_iter()
            .flatten()
            .last()
        };
        assert_eq!(took_a_prize(&mut relations), None, "one prize isn't war");
        let declared = (0..10).find_map(|_| took_a_prize(&mut relations));
        assert_eq!(declared, Some(Terms::War));
        let at_war = relations.between(Nation::ENGLAND, Nation::PIRATES);
        took_a_prize(&mut relations);
        assert_eq!(relations.between(Nation::ENGLAND, Nation::PIRATES), at_war);
    }

    #[test]
    fn delivering_to_another_nation_warms_relations_and_quarrels_drift_back() {
        let mut relations = Relations::default();
        let before = relations.between(Nation::ENGLAND, Nation::PIRATES);
        record_diplomacy(
            &mut relations,
            Nation::PIRATES,
            Nation::ENGLAND,
            DeedKind::Delivered,
            None,
        );
        assert!(relations.between(Nation::ENGLAND, Nation::PIRATES) > before);
        assert_eq!(
            record_diplomacy(
                &mut relations,
                Nation::ENGLAND,
                Nation::ENGLAND,
                DeedKind::Sank,
                None,
            ),
            None,
            "a nation can't fall out with herself"
        );
        relations.set(Nation::ENGLAND, Nation::SPAIN, -80.0);
        relations.drift(20.0);
        assert_eq!(relations.between(Nation::ENGLAND, Nation::SPAIN), -70.0);
        relations.drift(1000.0);
        assert_eq!(relations, Relations::default());
    }

    #[test]
    fn standing_is_bounded_and_bad_blood_fades() {
        let mut standing = Standing::default();
        let armada = Nation::SPAIN.groups()[0];
        standing.change_group(armada, -500.0);
        assert_eq!(standing.of(armada), LOWEST);
        standing.set(Nation::SPAIN, -5.0);
        standing.forget(2.0);
        assert_eq!(standing.with(Nation::SPAIN), -3.0);
        standing.forget(10.0);
        assert_eq!(
            standing.with(Nation::SPAIN),
            0.0,
            "fades to nothing, no further"
        );
        standing.forget(10.0);
        assert_eq!(standing.with(Nation::ENGLAND), 10.0, "good standing stays");
        standing.set(Nation::SPAIN, -50.0);
        standing.set_group(armada, 5.0);
        standing.lift(Nation::SPAIN, -10.0);
        assert_eq!(
            Nation::SPAIN.groups().map(|g| standing.of(g)),
            [5.0, -10.0, -10.0]
        );
    }
}
