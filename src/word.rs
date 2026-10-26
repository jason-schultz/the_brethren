//! Word of mouth: the ledger of what has happened, how word of it spreads,
//! and what each captain has heard.
//!
//! Everything worth talking of goes in the [`Ledger`] as [`News`]: a ship
//! sailing, a captain's deed, a wreck. The ledger holds the truth, where it
//! happened, and the ports where it's known. A thing done in port is known
//! there at once. A thing done at sea is known only to those who saw it:
//! ships in sight, the ship it was done to if she's still free, and the
//! crew of a ship taken, set adrift in the boats to row for the nearest
//! port. A ship that docks tells her port what she carries, and a ship
//! that sails carries what's known in her port to the next, so news
//! travels the trade lanes as fast as the shipping. A sailing is known at
//! once where she sailed from and where she's bound.
//!
//! A nation hears of a thing when it's known in one of its ports, and only
//! then does a deed done to it move the captain's standing and the
//! nations' relations ([`DeedKnown`]). Dead men tell no tales.
//!
//! A merchant who doesn't reach port by the time she's due is posted
//! overdue there, and the talk names the ships that were in those waters
//! when she was lost, if she was: the one who did it among them, if
//! anyone did.
//!
//! A captain who was in those waters can give an account of the loss in
//! port: taken or sunk by one of the others about, or wrecked. It's news
//! like any other, and taken at her word, true or not. A false one is
//! found out where it's known beside word of the loss itself, from a
//! witness or the survivors, and that's a deed of its own.
//!
//! A fight is one deed, however far it goes: a nation that heard of the
//! attack, then hears she was taken, answers only for the rest.
//!
//! What a captain has heard is her own [`Knowledge`]: what she saw; what
//! the taverns told her, which may get a cargo wrong, less often the
//! better she stands with that port's merchants; and the truth of the
//! sailings that concern the groups she serves, which they send her as
//! news. Sailings go stale in a few days, other news in a month.

use bevy::ecs::query::QueryData;
use bevy::math::FloatOrd;
use bevy::prelude::*;

use crate::alert::{Alert, AlertKind, sighting_range};
use crate::clock::{DAY, WorldClock};
use crate::combat::rounds::Dice;
use crate::economy::jobs::deadline;
use crate::factions::{
    CaptainId, Colours, Deed, DeedKind, Flag, Group, GroupKind, Nation, Relations, Standing, Terms,
    reputation,
};
use crate::goods::{Carriage, GoodId, Goods};
use crate::map::Charts;
use crate::port::{Docked, Location, Port};
use crate::schedule::{FrameSet, SimSet};
use crate::service::Service;

/// Word of a sailing goes stale after this long, days...
pub const SAILING_DAYS: f64 = 4.0;
/// ...and other news after this long.
pub const NEWS_DAYS: f64 = 30.0;
/// How fast survivors in the boats make for port, meters a second.
const BOAT_SPEED: f32 = 1.0;
/// Ships within this distance of a ship when she's lost, meters, were "in
/// those waters": seen there, by someone, about then.
pub const IN_THOSE_WATERS: f32 = 30_000.0;
/// The chance a tavern tells a sailing true, for a captain who stands at 0
/// with the port's merchants...
const TRUTH: f32 = 0.6;
/// ...and how much each point of standing adds or takes off it...
const TRUTH_PER_POINT: f32 = 0.004;
/// ...within these bounds.
const LEAST_TRUTH: f32 = 0.4;
const MOST_TRUTH: f32 = 0.95;

/// What a ship sailed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Carrying a contract's goods to the port that needs them.
    Carrying {
        good: GoodId,
        quantity: u32,
        bound_for: Entity,
    },
    /// Cruising for prizes.
    Cruising,
}

impl Purpose {
    pub fn bound_for(self) -> Option<Entity> {
        match self {
            Self::Carrying { bound_for, .. } => Some(bound_for),
            Self::Cruising => None,
        }
    }
}

/// A ship has put to sea from `from`. Written by the captain who sails her.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sailed {
    pub ship: Entity,
    pub from: Entity,
    pub purpose: Purpose,
}

/// Which piece of news, in the [`Ledger`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct NewsId(pub u64);

/// How a ship was lost, with the ship that did it as `S`: an entity, or a
/// name in a save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Fate<S = Entity> {
    Taken(S),
    Sunk(S),
    /// Wrecked aground or on a reef, or foundered, by no one's hand.
    Wrecked,
}

impl<S> Fate<S> {
    /// The ship that did it, if any.
    pub fn by(&self) -> Option<&S> {
        match self {
            Self::Taken(by) | Self::Sunk(by) => Some(by),
            Self::Wrecked => None,
        }
    }

    pub fn map<T>(self, f: impl FnOnce(S) -> T) -> Fate<T> {
        match self {
            Self::Taken(by) => Fate::Taken(f(by)),
            Self::Sunk(by) => Fate::Sunk(f(by)),
            Self::Wrecked => Fate::Wrecked,
        }
    }
}

/// Where something happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Place {
    Port(Entity),
    Sea(Vec3),
}

/// Something that happened, as it is or as it's told.
#[derive(Debug, Clone, PartialEq)]
pub enum Happening {
    /// A ship sailed to do something, and when she's due where she's
    /// bound, if anywhere.
    Sailed {
        ship: Entity,
        name: String,
        purpose: Purpose,
        due: Option<f64>,
    },
    /// A ship hasn't come in where she was due: posted overdue there, with
    /// the ships seen in those waters when she was lost, if she was.
    Overdue {
        ship: Entity,
        name: String,
        from: Entity,
        bound_for: Entity,
        suspects: Vec<(Entity, String)>,
    },
    /// A captain did something to or for one of a nation's groups.
    Deed(DeedNews),
    /// A ship went down or broke up, leaving a wreck if the water's
    /// shallow enough to reach her.
    Wrecked {
        ship: Entity,
        name: String,
        nation: Option<Nation>,
        /// The most of any one good she had aboard.
        cargo: Option<(GoodId, u32)>,
        wreck: Option<Entity>,
    },
    /// A captain's account of how a ship was lost: what she says, not
    /// what happened.
    Account {
        teller: Entity,
        teller_name: String,
        /// Whose name it's on, found out.
        captain: Option<CaptainId>,
        ship: Entity,
        name: String,
        says: Fate,
        /// The name of the ship she blames, if she blames one.
        blames: Option<String>,
    },
}

impl Happening {
    /// Whether it's about `ship`: she sailed, did it, or had it done to her.
    pub fn about(&self, ship: Entity) -> bool {
        match self {
            Self::Sailed { ship: sailed, .. } => *sailed == ship,
            Self::Deed(deed) => {
                deed.doer == ship
                    || deed
                        .victim
                        .as_ref()
                        .is_some_and(|(victim, _)| *victim == ship)
            }
            Self::Overdue {
                ship: overdue,
                suspects,
                ..
            } => *overdue == ship || suspects.iter().any(|(suspect, _)| *suspect == ship),
            Self::Wrecked { .. } => false,
            Self::Account {
                teller,
                ship: lost,
                says,
                ..
            } => [*teller, *lost].contains(&ship) || says.by() == Some(&ship),
        }
    }

    /// Whether it's word of `ship`'s loss itself, from someone who saw it
    /// or lived through it.
    fn witnesses_loss_of(&self, ship: Entity) -> bool {
        match self {
            Self::Deed(deed) => {
                matches!(deed.kind, DeedKind::Captured | DeedKind::Sank)
                    && deed
                        .victim
                        .as_ref()
                        .is_some_and(|(victim, _)| *victim == ship)
            }
            Self::Wrecked { ship: wrecked, .. } => *wrecked == ship,
            Self::Sailed { .. } | Self::Overdue { .. } | Self::Account { .. } => false,
        }
    }
}

/// A deed, as news.
#[derive(Debug, Clone, PartialEq)]
pub struct DeedNews {
    pub kind: DeedKind,
    pub group: Group,
    pub doer: Entity,
    pub doer_name: String,
    /// The captain whose name it touches, if the doer has one.
    pub captain: Option<CaptainId>,
    /// The colours the doer flew.
    pub colours: Option<Nation>,
    /// The ship it was done to, if any, and her name.
    pub victim: Option<(Entity, String)>,
    /// How it moves the captain's standing, group by group, as each nation
    /// hears of it: weighed by the nations' relations when it was done.
    pub standing: Vec<(Group, f32)>,
}

impl DeedNews {
    /// The deed with its worth weighed: a captain's moves her standing as
    /// the relations stand now; a ship with no captain has none to move.
    fn weighed(self, relations: &Relations) -> Self {
        let standing = if self.captain.is_some() {
            reputation(self.kind, self.group, relations)
        } else {
            Vec::new()
        };
        Self { standing, ..self }
    }

    /// How it moves its captain's standing with `nation`'s groups as that
    /// nation hears of it, having answered already for `answered`, the
    /// lesser deed of the same fight, if any: only what's still owed.
    pub fn standing_for(
        &self,
        nation: Nation,
        answered: Option<DeedKind>,
    ) -> impl Iterator<Item = (Group, f32)> + '_ {
        let owed =
            1.0 - answered.map_or(0.0, |answered| answered.standing() / self.kind.standing());
        self.standing
            .iter()
            .filter(move |(group, _)| group.nation == nation)
            .map(move |&(group, by)| (group, by * owed))
    }
}

/// One thing that happened, and how far word of it has got.
#[derive(Debug, Clone, PartialEq)]
pub struct News {
    pub id: NewsId,
    /// [`WorldClock`] time it happened.
    pub at: f64,
    pub place: Place,
    /// The truth of it.
    pub event: Happening,
    /// The ports whose taverns talk of it.
    pub known_at: Vec<Entity>,
    /// Ships that will tell it at the next port they make.
    pub carriers: Vec<Entity>,
    /// Survivors in the boats: the port each makes for, and when she gets
    /// there.
    pub boats: Vec<(Entity, f64)>,
    /// The nations that have heard: those holding a port where it's known.
    pub heard_by: Vec<Nation>,
}

impl News {
    /// When it stops being talked of: a sailing once she's due and a day
    /// over, or after a few days; anything else after a month.
    fn expires(&self) -> f64 {
        match self.event {
            Happening::Sailed { due, .. } => {
                (self.at + SAILING_DAYS * DAY).max(due.map_or(0.0, |due| due + DAY))
            }
            _ => self.at + NEWS_DAYS * DAY,
        }
    }

    fn tell_at(&mut self, port: Entity) {
        if !self.known_at.contains(&port) {
            self.known_at.push(port);
        }
    }
}

/// A ship lost, how and when, and who was in those waters at the time.
#[derive(Debug, Clone, PartialEq)]
pub struct Loss {
    pub ship: Entity,
    pub name: String,
    /// The group she belonged to, if she flew colours.
    pub group: Option<Group>,
    pub fate: Fate,
    pub at: f64,
    pub nearby: Vec<(Entity, String)>,
}

impl Loss {
    /// What `teller` could say of it: wrecked, or taken or sunk by one of
    /// the others about. Nothing, if she wasn't in those waters.
    pub fn accounts(&self, teller: Entity) -> Vec<Fate> {
        if !self.nearby.iter().any(|(ship, _)| *ship == teller) {
            return Vec::new();
        }
        std::iter::once(Fate::Wrecked)
            .chain(
                self.nearby
                    .iter()
                    .filter(|(ship, _)| *ship != teller)
                    .flat_map(|&(ship, _)| [Fate::Taken(ship), Fate::Sunk(ship)]),
            )
            .collect()
    }

    /// The name of a ship that was in those waters.
    pub fn name_of(&self, ship: Entity) -> Option<&str> {
        self.nearby
            .iter()
            .find(|(nearby, _)| *nearby == ship)
            .map(|(_, name)| name.as_str())
    }
}

/// What a nation has answered for, of a fight between two ships: how far
/// it went, as far as that nation has heard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Answered {
    pub doer: Entity,
    pub victim: Entity,
    pub nation: Nation,
    pub kind: DeedKind,
    pub at: f64,
}

/// The truth of everything that's happened, while it's talked of.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Ledger {
    pub news: Vec<News>,
    /// The id the next news will have.
    pub next: NewsId,
    /// Ships lost lately, for the talk when they're overdue, and the truth
    /// of what's said of them.
    pub losses: Vec<Loss>,
    /// What each nation has answered for, fight by fight.
    pub answered: Vec<Answered>,
}

impl Ledger {
    /// Notes a ship lost. The first fate is the true one: a prize burned
    /// was taken first.
    pub fn note_loss(&mut self, loss: Loss) {
        if !self.losses.iter().any(|lost| lost.ship == loss.ship) {
            self.losses.push(loss);
        }
    }

    /// Whether `teller` has given an account of `ship`'s loss that's still
    /// talked of.
    pub fn has_account(&self, teller: Entity, ship: Entity) -> bool {
        self.news.iter().any(|news| {
            matches!(news.event, Happening::Account { teller: told, ship: lost, .. }
                if told == teller && lost == ship)
        })
    }

    /// The losses `teller` could give an account of, and hasn't. A ship
    /// lost before a game was loaded can't be told apart from another, so
    /// there's no accounting for her.
    pub fn accountable(&self, teller: Entity) -> impl Iterator<Item = &Loss> {
        self.losses.iter().filter(move |loss| {
            loss.ship != Entity::PLACEHOLDER
                && !loss.accounts(teller).is_empty()
                && !self.has_account(teller, loss.ship)
        })
    }

    pub fn get(&self, id: NewsId) -> Option<&News> {
        self.news.iter().find(|news| news.id == id)
    }

    /// Records `event`, at `at` in `place`, known in `known_at` and carried
    /// by `carriers` and `boats`. Returns its id.
    pub fn record(
        &mut self,
        at: f64,
        place: Place,
        event: Happening,
        known_at: Vec<Entity>,
        carriers: Vec<Entity>,
        boats: Vec<(Entity, f64)>,
    ) -> NewsId {
        let id = self.next;
        self.next = NewsId(id.0 + 1);
        self.news.push(News {
            id,
            at,
            place,
            event,
            known_at,
            carriers,
            boats,
            heard_by: Vec::new(),
        });
        id
    }
}

/// The ships in sight of `at` that see what happens there: every one but
/// those in `unseeing`.
pub fn witnesses(
    at: Vec3,
    ships: impl Iterator<Item = (Entity, Vec3)>,
    unseeing: &[Entity],
) -> Vec<Entity> {
    ships
        .filter(|&(ship, position)| {
            !unseeing.contains(&ship) && position.distance(at) <= sighting_range()
        })
        .map(|(ship, _)| ship)
        .collect()
}

/// The ships of `ships` within [`IN_THOSE_WATERS`] of `at`, but `lost`.
pub fn in_those_waters<'a>(
    at: Vec3,
    lost: Entity,
    ships: impl Iterator<Item = (Entity, &'a Name, Vec3)>,
) -> Vec<(Entity, String)> {
    ships
        .filter(|&(ship, _, position)| ship != lost && position.distance(at) <= IN_THOSE_WATERS)
        .map(|(ship, name, _)| (ship, name.to_string()))
        .collect()
}

/// The port nearest `at`, of `ports`, and when survivors setting out from
/// `at` at `now` get there in the boats.
pub fn boat_to(
    at: Vec3,
    now: f64,
    ports: impl Iterator<Item = (Entity, Vec3)>,
) -> Option<(Entity, f64)> {
    ports
        .min_by_key(|&(_, anchorage)| FloatOrd(anchorage.distance(at)))
        .map(|(port, anchorage)| (port, now + (anchorage.distance(at) / BOAT_SPEED) as f64))
}

/// A nation has heard of a deed: it's known in one of its ports.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeedKnown {
    pub news: NewsId,
    pub nation: Nation,
    /// The lesser deed of the same fight it's already answered for, if
    /// any: it answers only for the rest.
    pub answered: Option<DeedKind>,
}

/// What a nation that hears of a deed of `kind` answers for, having
/// answered for `before` of the same fight: all of it, the rest of it, or
/// nothing if it's heard of as much or worse.
pub fn answer(kind: DeedKind, before: Option<DeedKind>) -> Option<Option<DeedKind>> {
    match (kind.escalation(), before.and_then(DeedKind::escalation)) {
        (Some(now), Some(then)) if now <= then => None,
        _ => Some(before),
    }
}

/// Order a docked captain to give an account of how a ship was lost.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiveAccount {
    pub ship: Entity,
    pub lost: Entity,
    pub says: Fate,
}

/// How a captain heard of something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// She saw it.
    Saw,
    /// In the tavern at this port, as it was told there.
    Tavern(Entity),
    /// From a group she serves, which knows the truth.
    Service(Group),
}

/// What a captain heard of something, and how.
#[derive(Debug, Clone, PartialEq)]
pub struct Tale {
    pub news: NewsId,
    /// When and where it happened.
    pub at: f64,
    pub place: Place,
    /// What she was told.
    pub event: Happening,
    pub heard: Heard,
}

/// What a captain has heard. Kept on the ship she commands, beside her
/// standing.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Knowledge(pub Vec<Tale>);

impl Knowledge {
    /// The latest she's heard about `ship`, if anything.
    pub fn latest_about(&self, ship: Entity) -> Option<&Tale> {
        self.0
            .iter()
            .filter(|tale| tale.event.about(ship))
            .max_by_key(|tale| tale.news)
    }

    /// What she's heard of this news, if anything.
    pub fn of(&self, news: NewsId) -> Option<&Tale> {
        self.0.iter().find(|tale| tale.news == news)
    }

    fn learn(&mut self, tale: Tale) {
        if self.of(tale.news).is_none() {
            self.0.push(tale);
        }
    }
}

/// The chance a tavern tells a sailing true, for a captain standing
/// `standing` with the port's merchants.
pub fn truth(standing: f32) -> f32 {
    (TRUTH + standing * TRUTH_PER_POINT).clamp(LEAST_TRUTH, MOST_TRUTH)
}

/// `purpose` as a tavern tells it. If `roll` (in `[0, 1)`) misses `truth`,
/// the cargo is wrong: another good that travels the same way, chosen by
/// `pick`, also in `[0, 1)`.
pub fn tell(goods: &Goods, purpose: Purpose, truth: f32, roll: f32, pick: f32) -> Purpose {
    let Purpose::Carrying {
        good,
        quantity,
        bound_for,
    } = purpose
    else {
        return purpose;
    };
    let others: Vec<GoodId> = goods
        .ids()
        .filter(|&other| other != good && goods.carriage(other) == goods.carriage(good))
        .collect();
    if roll < truth || others.is_empty() {
        return purpose;
    }
    Purpose::Carrying {
        good: others[(pick * others.len() as f32) as usize % others.len()],
        quantity,
        bound_for,
    }
}

/// Whether `group` would send word of a ship of `nation`, sailing for
/// `serves`, putting to sea for `purpose`. A crown's navy watches for
/// raiders of nations hostile to it, and the Brethren for hostile
/// shipping to take. A governor watches foreign soldiers on the move, and
/// a crown's merchants their own nation's cargoes; the Brethren's fences
/// hear of the same prizes the Flying Gang does.
pub fn concerns(
    group: Group,
    nation: Nation,
    serves: GroupKind,
    purpose: Purpose,
    goods: &Goods,
    relations: &Relations,
) -> bool {
    let terms = relations.terms(group.nation, nation);
    let foe = terms <= Terms::Hostile;
    let prize = foe && serves == GroupKind::Merchants;
    match (group.kind, purpose) {
        (GroupKind::Navy, Purpose::Cruising) => group.nation.is_crown() && foe,
        (GroupKind::Navy, Purpose::Carrying { .. }) => !group.nation.is_crown() && prize,
        (GroupKind::Governor, Purpose::Carrying { good, .. }) => {
            terms < Terms::Alliance && goods.carriage(good) == Carriage::Berths
        }
        (GroupKind::Merchants, Purpose::Carrying { .. }) if group.nation.is_crown() => {
            nation == group.nation && serves == GroupKind::Merchants
        }
        (GroupKind::Merchants, Purpose::Carrying { .. }) => prize,
        (GroupKind::Governor | GroupKind::Merchants, Purpose::Cruising) => false,
    }
}

pub struct WordPlugin;

impl Plugin for WordPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Sailed>()
            .add_message::<DeedKnown>()
            .add_message::<GiveAccount>()
            .init_resource::<Ledger>()
            .add_observer(tell_port)
            .add_observer(carry_news)
            // Orders work while paused, and so do the deeds they bring.
            .add_systems(
                Update,
                (give_accounts, (record_sailings, record_deeds).chain()).after(FrameSet::Orders),
            )
            .add_systems(
                FixedUpdate,
                (post_overdue, spread_news, find_out_lies, hear_in_port)
                    .chain()
                    .in_set(SimSet::Resolve),
            );
    }
}

/// A ship as the news tells of her. A port is no witness, though it has a
/// name and a place.
#[derive(QueryData)]
pub(crate) struct Told {
    entity: Entity,
    name: &'static Name,
    at: &'static Transform,
    docked: Option<&'static Docked>,
    flag: Flag,
    captain: Option<&'static CaptainId>,
}

/// Every sailing goes in the ledger, and a group a captain serves sends
/// her the truth of the sailings that concern it.
fn record_sailings(
    mut sailings: MessageReader<Sailed>,
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    relations: Res<Relations>,
    charts: Charts,
    ships: Query<Told, Without<Port>>,
    mut captains: Query<(Entity, &Service, &mut Knowledge)>,
    mut ledger: ResMut<Ledger>,
    mut alerts: MessageWriter<Alert>,
) {
    let now = clock.elapsed;
    for sailed in sailings.read() {
        let Ok(ship) = ships.get(sailed.ship) else {
            continue;
        };
        // Her last voyage is over.
        ledger.news.retain(
            |news| !matches!(news.event, Happening::Sailed { ship, .. } if ship == sailed.ship),
        );
        // Due as a contract would be: twice a slow merchantman's passage,
        // and a day.
        let due = sailed.purpose.bound_for().and_then(|bound_for| {
            let (from, to) = (charts.anchorage(sailed.from)?, charts.anchorage(bound_for)?);
            Some(deadline(now, charts.passage(from, to)))
        });
        let event = Happening::Sailed {
            ship: sailed.ship,
            name: ship.name.to_string(),
            purpose: sailed.purpose,
            due,
        };
        let known_at = std::iter::once(sailed.from)
            .chain(sailed.purpose.bound_for())
            .collect();
        let id = ledger.record(
            now,
            Place::Port(sailed.from),
            event.clone(),
            known_at,
            Vec::new(),
            Vec::new(),
        );
        let (Some(&Colours(nation)), serves) = (
            ship.flag.colours,
            ship.flag
                .serves
                .map_or(GroupKind::Merchants, |serves| serves.0),
        ) else {
            continue;
        };
        for (captain, service, mut knowledge) in &mut captains {
            let Some(group) =
                service.0.iter().map(|post| post.group).find(|&group| {
                    concerns(group, nation, serves, sailed.purpose, &goods, &relations)
                })
            else {
                continue;
            };
            knowledge.learn(Tale {
                news: id,
                at: now,
                place: Place::Port(sailed.from),
                event: event.clone(),
                heard: Heard::Service(group),
            });
            alerts.write(Alert {
                ship: captain,
                kind: AlertKind::Intelligence {
                    ship: sailed.ship,
                    from: sailed.from,
                    purpose: sailed.purpose,
                    group,
                },
            });
        }
    }
}

/// Every deed goes in the ledger, with who knows of it: captains who saw
/// it know it.
pub(crate) fn record_deeds(
    mut deeds: MessageReader<Deed>,
    clock: Res<WorldClock>,
    relations: Res<Relations>,
    ships: Query<Told, Without<Port>>,
    ports: Query<(Entity, &Transform), With<Port>>,
    mut captains: Query<(Entity, &Service, &mut Knowledge)>,
    mut ledger: ResMut<Ledger>,
) {
    let now = clock.elapsed;
    for deed in deeds.read() {
        let Ok(doer) = ships.get(deed.doer) else {
            continue;
        };
        let at = doer.at;
        // Closing again on a ship she's already attacked, after a chase
        // broke off, is the same attack; taking again a ship she plundered
        // and let go is the same taking. Neither is new while the first is
        // still talked of.
        let again = matches!(deed.kind, DeedKind::Attacked | DeedKind::Captured)
            && ledger.news.iter().any(|news| {
                matches!(&news.event, Happening::Deed(old)
                    if old.kind == deed.kind
                        && old.doer == deed.doer
                        && old.victim.as_ref().map(|(ship, _)| *ship) == deed.about)
            });
        if again {
            continue;
        }
        let victim = deed
            .about
            .and_then(|ship| Some((ship, ships.get(ship).ok()?.name.to_string())));
        let news = DeedNews {
            kind: deed.kind,
            group: deed.group,
            doer: deed.doer,
            doer_name: doer.name.to_string(),
            captain: doer.captain.copied(),
            colours: doer.flag.colours.map(|colours| colours.0),
            victim: victim.clone(),
            standing: Vec::new(),
        }
        .weighed(&relations);
        if deed.kind == DeedKind::Captured
            && let Some((taken, taken_name)) = victim
        {
            let nearby = in_those_waters(
                at.translation,
                taken,
                ships
                    .iter()
                    .map(|ship| (ship.entity, ship.name, ship.at.translation)),
            );
            ledger.note_loss(Loss {
                ship: taken,
                name: taken_name,
                group: Some(deed.group),
                fate: Fate::Taken(deed.doer),
                at: now,
                nearby,
            });
        }
        match doer.docked {
            Some(&Docked { port }) => {
                ledger.record(
                    now,
                    Place::Port(port),
                    Happening::Deed(news),
                    vec![port],
                    Vec::new(),
                    Vec::new(),
                );
            }
            None => {
                let position = at.translation;
                let free_victim = deed.about.filter(|_| deed.kind != DeedKind::Sank);
                // The doer tells only of her own failures.
                let tells_on_herself =
                    matches!(deed.kind, DeedKind::LostCommand | DeedKind::LostWard);
                let mut carriers = witnesses(
                    position,
                    ships
                        .iter()
                        .filter(|ship| ship.docked.is_none())
                        .map(|ship| (ship.entity, ship.at.translation)),
                    &[deed.doer],
                );
                carriers.extend(free_victim.filter(|ship| !carriers.contains(ship)));
                if tells_on_herself {
                    carriers.push(deed.doer);
                }
                // A prize's crew is set adrift, and rows for the nearest port.
                let boats = (deed.kind == DeedKind::Captured)
                    .then(|| {
                        boat_to(
                            position,
                            now,
                            ports.iter().map(|(port, at)| (port, at.translation)),
                        )
                    })
                    .flatten()
                    .into_iter()
                    .collect();
                let saw: Vec<Entity> = carriers.clone();
                let id = ledger.record(
                    now,
                    Place::Sea(position),
                    Happening::Deed(news.clone()),
                    Vec::new(),
                    carriers,
                    boats,
                );
                for (_, _, mut knowledge) in captains
                    .iter_mut()
                    .filter(|(captain, ..)| saw.contains(captain) && *captain != deed.doer)
                {
                    knowledge.learn(Tale {
                        news: id,
                        at: now,
                        place: Place::Sea(position),
                        event: Happening::Deed(news.clone()),
                        heard: Heard::Saw,
                    });
                }
            }
        };
    }
}

/// A ship coming into port tells it everything she carries.
fn tell_port(add: On<Add<Docked>>, docked: Query<&Docked>, mut ledger: ResMut<Ledger>) {
    let ship = add.entity;
    let Ok(&Docked { port }) = docked.get(ship) else {
        return;
    };
    for news in ledger
        .news
        .iter_mut()
        .filter(|news| news.carriers.contains(&ship))
    {
        news.tell_at(port);
    }
    // She's come in where she was bound: no one will look for her now.
    ledger.news.retain(|news| {
        !matches!(news.event, Happening::Sailed { ship: sailed, purpose, .. }
            if sailed == ship && purpose.bound_for() == Some(port))
    });
}

/// A ship due in port and not come in is posted overdue there. If she was
/// lost, the talk names who was in those waters at the time.
fn post_overdue(clock: Res<WorldClock>, mut ledger: ResMut<Ledger>) {
    let now = clock.elapsed;
    let overdue: Vec<(NewsId, Entity, Happening)> = ledger
        .news
        .iter()
        .filter_map(|news| {
            let Happening::Sailed {
                ship,
                ref name,
                purpose,
                due: Some(due),
            } = news.event
            else {
                return None;
            };
            let (Some(bound_for), Place::Port(from)) = (purpose.bound_for(), news.place) else {
                return None;
            };
            if due > now {
                return None;
            }
            let suspects = ledger
                .losses
                .iter()
                .find(|loss| loss.ship == ship)
                .map_or(Vec::new(), |loss| loss.nearby.clone());
            let event = Happening::Overdue {
                ship,
                name: name.clone(),
                from,
                bound_for,
                suspects,
            };
            Some((news.id, bound_for, event))
        })
        .collect();
    for (sailing, bound_for, event) in overdue {
        ledger.news.retain(|news| news.id != sailing);
        ledger.record(
            now,
            Place::Port(bound_for),
            event,
            vec![bound_for],
            Vec::new(),
            Vec::new(),
        );
    }
    ledger
        .losses
        .retain(|loss| now - loss.at <= NEWS_DAYS * DAY);
}

/// A captain in port gives her account of a loss she was about for, once.
fn give_accounts(
    mut orders: MessageReader<GiveAccount>,
    clock: Res<WorldClock>,
    tellers: Query<(&Name, &Docked, Option<&CaptainId>)>,
    mut ledger: ResMut<Ledger>,
) {
    for order in orders.read() {
        let Ok((teller_name, &Docked { port }, captain)) = tellers.get(order.ship) else {
            continue;
        };
        let Some(loss) = ledger
            .accountable(order.ship)
            .find(|loss| loss.ship == order.lost)
            .filter(|loss| loss.accounts(order.ship).contains(&order.says))
        else {
            continue;
        };
        let event = Happening::Account {
            teller: order.ship,
            teller_name: teller_name.to_string(),
            captain: captain.copied(),
            ship: loss.ship,
            name: loss.name.clone(),
            says: order.says,
            blames: order
                .says
                .by()
                .and_then(|&by| loss.name_of(by))
                .map(str::to_string),
        };
        ledger.record(
            clock.elapsed,
            Place::Port(port),
            event,
            vec![port],
            Vec::new(),
            Vec::new(),
        );
    }
}

/// A false account known in a port beside word of the loss itself is
/// found out there: a deed against the captain who gave it, known there,
/// and the account no longer talked of.
fn find_out_lies(clock: Res<WorldClock>, relations: Res<Relations>, mut ledger: ResMut<Ledger>) {
    let found_out: Vec<(NewsId, Entity, DeedNews)> = ledger
        .news
        .iter()
        .filter_map(|news| {
            let Happening::Account {
                teller,
                ref teller_name,
                captain,
                ship,
                ref name,
                says,
                ..
            } = news.event
            else {
                return None;
            };
            let loss = ledger
                .losses
                .iter()
                .find(|loss| loss.ship == ship && ship != Entity::PLACEHOLDER)?;
            if loss.fate == says {
                return None;
            }
            let port = news.known_at.iter().copied().find(|&port| {
                ledger.news.iter().any(|other| {
                    other.event.witnesses_loss_of(ship) && other.known_at.contains(&port)
                })
            })?;
            let deed = DeedNews {
                kind: DeedKind::FalseReport,
                group: loss.group?,
                doer: teller,
                doer_name: teller_name.clone(),
                captain,
                colours: None,
                victim: Some((ship, name.clone())),
                standing: Vec::new(),
            }
            .weighed(&relations);
            Some((news.id, port, deed))
        })
        .collect();
    for (account, port, deed) in found_out {
        ledger.news.retain(|news| news.id != account);
        ledger.record(
            clock.elapsed,
            Place::Port(port),
            Happening::Deed(deed),
            vec![port],
            Vec::new(),
            Vec::new(),
        );
    }
}

/// A ship leaving port carries what's known there.
fn carry_news(remove: On<Remove<Docked>>, docked: Query<&Docked>, mut ledger: ResMut<Ledger>) {
    let ship = remove.entity;
    let Ok(&Docked { port }) = docked.get(ship) else {
        return;
    };
    for news in ledger
        .news
        .iter_mut()
        .filter(|news| news.known_at.contains(&port) && !news.carriers.contains(&ship))
    {
        news.carriers.push(ship);
    }
}

/// Survivors in the boats reach port; stale news, and carriers no longer
/// afloat, are forgotten; and each nation hears of what's known in its
/// ports.
fn spread_news(
    clock: Res<WorldClock>,
    ships: Query<()>,
    ports: Query<&Port>,
    mut ledger: ResMut<Ledger>,
    mut known: MessageWriter<DeedKnown>,
) {
    let now = clock.elapsed;
    let ledger = &mut *ledger;
    ledger.news.retain(|news| now <= news.expires());
    ledger
        .answered
        .retain(|answered| now - answered.at <= NEWS_DAYS * DAY);
    let answered = &mut ledger.answered;
    for news in &mut ledger.news {
        let landed: Vec<Entity> = news
            .boats
            .iter()
            .filter(|&&(_, arrives)| arrives <= now)
            .map(|&(port, _)| port)
            .collect();
        news.boats.retain(|&(_, arrives)| arrives > now);
        for port in landed {
            news.tell_at(port);
        }
        news.carriers.retain(|&ship| ships.contains(ship));
        for &port in &news.known_at {
            let Ok(&Port { flag: nation, .. }) = ports.get(port) else {
                continue;
            };
            if news.heard_by.contains(&nation) {
                continue;
            }
            news.heard_by.push(nation);
            let Happening::Deed(deed) = &news.event else {
                continue;
            };
            let fight = deed
                .victim
                .as_ref()
                .filter(|_| deed.kind.escalation().is_some())
                .map(|&(victim, _)| (deed.doer, victim));
            let before = fight.and_then(|(doer, victim)| {
                answered.iter_mut().find(|answered| {
                    (answered.doer, answered.victim, answered.nation) == (doer, victim, nation)
                })
            });
            let Some(less) = answer(deed.kind, before.as_ref().map(|before| before.kind)) else {
                continue;
            };
            match (before, fight) {
                (Some(before), _) => before.kind = deed.kind,
                (None, Some((doer, victim))) => answered.push(Answered {
                    doer,
                    victim,
                    nation,
                    kind: deed.kind,
                    at: news.at,
                }),
                (None, None) => {}
            }
            known.write(DeedKnown {
                news: news.id,
                nation,
                answered: less,
            });
        }
    }
}

/// Captains forget what's gone stale. A captain lying in a port with a
/// tavern hears what it talks of, as it's told to her.
fn hear_in_port(
    goods: Res<Goods>,
    ledger: Res<Ledger>,
    mut dice: ResMut<Dice>,
    mut captains: Query<(Option<&Docked>, &Standing, &mut Knowledge)>,
    ports: Query<&Port>,
) {
    for (docked, standing, mut knowledge) in &mut captains {
        if !knowledge
            .0
            .iter()
            .all(|tale| ledger.get(tale.news).is_some())
        {
            knowledge.0.retain(|tale| ledger.get(tale.news).is_some());
        }
        let Some(&Docked { port }) = docked else {
            continue;
        };
        let Ok(town) = ports.get(port) else {
            continue;
        };
        if !town.locations.contains(&Location::Tavern) {
            continue;
        }
        let merchants = standing.of(Group {
            nation: town.flag,
            kind: GroupKind::Merchants,
        });
        for news in &ledger.news {
            if !news.known_at.contains(&port) || knowledge.of(news.id).is_some() {
                continue;
            }
            let event = match &news.event {
                Happening::Sailed {
                    ship,
                    name,
                    purpose,
                    due,
                } => Happening::Sailed {
                    ship: *ship,
                    name: name.clone(),
                    purpose: tell(&goods, *purpose, truth(merchants), dice.roll(), dice.roll()),
                    due: *due,
                },
                event => event.clone(),
            };
            knowledge.learn(Tale {
                news: news.id,
                at: news.at,
                place: news.place,
                event,
                heard: Heard::Tavern(port),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factions::CaptainRecord;
    use crate::test_support::{run_checked, run_seconds, sim_app};

    fn carrying(goods: &Goods, key: &str) -> Purpose {
        Purpose::Carrying {
            good: goods.id(key).unwrap(),
            quantity: 20,
            bound_for: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn taverns_tell_it_truer_to_captains_the_merchants_trust() {
        assert!(truth(50.0) > truth(0.0));
        assert!(truth(0.0) > truth(-50.0));
        assert_eq!(truth(1_000.0), MOST_TRUTH, "never quite certain");
        assert_eq!(truth(-1_000.0), LEAST_TRUTH);
    }

    #[test]
    fn a_tale_gets_the_cargo_wrong_but_not_how_it_travels() {
        let goods = Goods::standard();
        let sugar = carrying(&goods, "sugar");
        assert_eq!(tell(&goods, sugar, 0.7, 0.5, 0.3), sugar, "told true");
        let wrong = tell(&goods, sugar, 0.7, 0.9, 0.3);
        let Purpose::Carrying { good, quantity, .. } = wrong else {
            panic!("{wrong:?}");
        };
        assert_ne!(good, goods.id("sugar").unwrap());
        assert_eq!(goods.carriage(good), Carriage::Hold);
        assert_eq!(quantity, 20);
        let soldiers = carrying(&goods, "soldiers");
        let Purpose::Carrying { good, .. } = tell(&goods, soldiers, 0.0, 0.5, 0.99) else {
            panic!();
        };
        assert_eq!(good, goods.sailors, "the only other people aboard ships");
        assert_eq!(
            tell(&goods, Purpose::Cruising, 0.0, 0.9, 0.5),
            Purpose::Cruising
        );
    }

    #[test]
    fn each_group_sends_word_of_its_own_business() {
        let goods = Goods::standard();
        let relations = Relations::default();
        let [royal_navy, governor, jamaica_merchants] = Nation::ENGLAND.groups();
        let [flying_gang, _, fences] = Nation::PIRATES.groups();
        let sugar = carrying(&goods, "sugar");
        let soldiers = carrying(&goods, "soldiers");
        let concern = |group, nation, serves, purpose| {
            concerns(group, nation, serves, purpose, &goods, &relations)
        };
        use GroupKind::*;
        assert!(concern(
            royal_navy,
            Nation::PIRATES,
            Navy,
            Purpose::Cruising
        ));
        assert!(!concern(royal_navy, Nation::SPAIN, Merchants, sugar));
        assert!(concern(governor, Nation::SPAIN, Merchants, soldiers));
        assert!(!concern(governor, Nation::SPAIN, Merchants, sugar));
        assert!(!concern(governor, Nation::ENGLAND, Merchants, soldiers));
        assert!(concern(
            jamaica_merchants,
            Nation::ENGLAND,
            Merchants,
            sugar
        ));
        assert!(!concern(jamaica_merchants, Nation::SPAIN, Merchants, sugar));
        for group in [flying_gang, fences] {
            assert!(concern(group, Nation::ENGLAND, Merchants, sugar), "{group}");
            assert!(!concern(group, Nation::ENGLAND, Navy, Purpose::Cruising));
        }
    }

    /// A harbour: an English port with a tavern, a pirate port, a merchant
    /// of `nation`, and a captain at sea.
    fn harbour(nation: Nation) -> (App, Entity, Entity, Entity) {
        let mut app = sim_app();
        let port = app
            .world_mut()
            .spawn((
                Port {
                    flag: Nation::ENGLAND,
                    town: Vec3::ZERO,
                    locations: vec![Location::Tavern],
                },
                Transform::default(),
            ))
            .id();
        let merchant = app
            .world_mut()
            .spawn((Name::new("Dove"), Colours(nation), Transform::default()))
            .id();
        let captain = app
            .world_mut()
            .spawn((
                Name::new("Player Sloop"),
                CaptainRecord::default(),
                Transform::from_xyz(50_000.0, 0.0, 0.0),
            ))
            .id();
        (app, port, merchant, captain)
    }

    fn knowledge(app: &App, captain: Entity) -> &Knowledge {
        app.world().get::<Knowledge>(captain).unwrap()
    }

    /// The intelligence captains are sent over a second.
    fn intelligence(app: &mut App) -> Vec<Alert> {
        let mut cursor = app.world().resource::<Messages<Alert>>().get_cursor();
        let mut alerts = Vec::new();
        run_checked(app, 1.0, |world| {
            let messages = world.resource::<Messages<Alert>>();
            alerts.extend(
                cursor
                    .read(messages)
                    .filter(|alert| matches!(alert.kind, AlertKind::Intelligence { .. })),
            );
        });
        alerts
    }

    #[test]
    fn a_captain_hears_a_ports_talk_in_its_tavern_and_others_dont() {
        let (mut app, port, merchant, ashore) = harbour(Nation::ENGLAND);
        let at_sea = app
            .world_mut()
            .spawn((CaptainRecord::default(), Transform::default()))
            .id();
        let goods = Goods::standard();
        let sugar = carrying(&goods, "sugar");
        app.world_mut().write_message(Sailed {
            ship: merchant,
            from: port,
            purpose: sugar,
        });
        assert!(intelligence(&mut app).is_empty(), "they serve no one");
        let news = &app.world().resource::<Ledger>().news[0];
        assert!(matches!(news.event, Happening::Sailed { purpose, .. } if purpose == sugar));
        assert!(knowledge(&app, ashore).latest_about(merchant).is_none());

        app.world_mut().entity_mut(ashore).insert(Docked { port });
        run_seconds(&mut app, 1.0);
        let tale = knowledge(&app, ashore).latest_about(merchant).unwrap();
        assert_eq!(tale.heard, Heard::Tavern(port));
        assert!(
            knowledge(&app, at_sea).0.is_empty(),
            "each hears for herself"
        );

        app.world_mut().resource_mut::<WorldClock>().elapsed += SAILING_DAYS * DAY + 1.0;
        run_seconds(&mut app, 1.0);
        assert!(app.world().resource::<Ledger>().news.is_empty(), "stale");
        assert!(knowledge(&app, ashore).0.is_empty(), "and forgotten");
    }

    #[test]
    fn a_group_she_serves_sends_a_captain_true_word_of_its_business() {
        let (mut app, port, merchant, captain) = harbour(Nation::ENGLAND);
        let goods = Goods::standard();
        let jamaica_merchants = Nation::ENGLAND.groups()[2];
        app.world_mut().get_mut::<Service>(captain).unwrap().0 = vec![crate::service::Post {
            group: jamaica_merchants,
            rank: 0,
        }];
        let purpose = carrying(&goods, "sugar");
        app.world_mut().write_message(Sailed {
            ship: merchant,
            from: port,
            purpose,
        });
        assert_eq!(
            intelligence(&mut app),
            [Alert {
                ship: captain,
                kind: AlertKind::Intelligence {
                    ship: merchant,
                    from: port,
                    purpose,
                    group: jamaica_merchants,
                },
            }]
        );
        let tale = knowledge(&app, captain).latest_about(merchant).unwrap();
        assert_eq!(tale.heard, Heard::Service(jamaica_merchants));
        assert!(matches!(tale.event, Happening::Sailed { purpose: told, .. } if told == purpose));
    }

    fn deeds_known(app: &mut App, seconds: f32) -> Vec<DeedKnown> {
        let mut cursor = app.world().resource::<Messages<DeedKnown>>().get_cursor();
        let mut known = Vec::new();
        run_checked(app, seconds, |world| {
            known.extend(cursor.read(world.resource::<Messages<DeedKnown>>()));
        });
        known
    }

    #[test]
    fn a_deed_unseen_at_sea_is_known_only_when_a_witness_makes_port() {
        let (mut app, port, merchant, captain) = harbour(Nation::ENGLAND);
        let jamaica_merchants = Nation::ENGLAND.groups()[2];
        app.world_mut().write_message(Deed {
            doer: captain,
            kind: DeedKind::Attacked,
            group: jamaica_merchants,
            about: None,
        });
        assert!(deeds_known(&mut app, 1.0).is_empty(), "no one saw");

        // The same again, with the Dove in sight.
        app.world_mut()
            .get_mut::<Transform>(merchant)
            .unwrap()
            .translation = Vec3::new(50_100.0, 0.0, 0.0);
        app.world_mut().write_message(Deed {
            doer: captain,
            kind: DeedKind::Attacked,
            group: jamaica_merchants,
            about: Some(merchant),
        });
        assert!(deeds_known(&mut app, 1.0).is_empty(), "she's still at sea");
        app.world_mut().entity_mut(merchant).insert(Docked { port });
        let known = deeds_known(&mut app, 1.0);
        assert_eq!(known.len(), 1, "England hears once: {known:?}");
        assert_eq!(known[0].nation, Nation::ENGLAND);
    }

    #[test]
    fn a_port_beside_a_taking_is_no_witness_to_it() {
        let (mut app, port, merchant, captain) = harbour(Nation::ENGLAND);
        app.world_mut()
            .entity_mut(port)
            .insert(Name::new("Tortuga"));
        app.world_mut()
            .get_mut::<Transform>(captain)
            .unwrap()
            .translation = Vec3::new(100.0, 0.0, 0.0);
        app.world_mut().write_message(Deed {
            doer: captain,
            kind: DeedKind::Captured,
            group: Nation::ENGLAND.groups()[2],
            about: Some(merchant),
        });
        app.update();
        let ledger = app.world().resource::<Ledger>();
        let nearby: Vec<Entity> = ledger.losses[0]
            .nearby
            .iter()
            .map(|(ship, _)| *ship)
            .collect();
        assert_eq!(nearby, [captain]);
        assert!(
            ledger
                .news
                .iter()
                .all(|news| !news.carriers.contains(&port))
        );
    }

    #[test]
    fn closing_again_on_the_same_ship_is_the_same_deed() {
        let (mut app, _, merchant, captain) = harbour(Nation::ENGLAND);
        for kind in [DeedKind::Attacked, DeedKind::Captured].repeat(3) {
            app.world_mut().write_message(Deed {
                doer: captain,
                kind,
                group: Nation::ENGLAND.groups()[2],
                about: Some(merchant),
            });
            app.update();
        }
        assert_eq!(
            app.world().resource::<Ledger>().news.len(),
            2,
            "one of each"
        );
    }

    #[test]
    fn a_fight_that_goes_further_is_answered_for_the_rest_and_no_more() {
        use DeedKind::*;
        assert_eq!(answer(Attacked, None), Some(None));
        assert_eq!(answer(Captured, Some(Attacked)), Some(Some(Attacked)));
        assert_eq!(answer(Sank, Some(Captured)), Some(Some(Captured)));
        assert_eq!(answer(Attacked, Some(Captured)), None, "heard of worse");
        assert_eq!(answer(Captured, Some(Captured)), None);
    }

    #[test]
    fn a_nation_that_heard_of_the_attack_and_then_the_taking_answers_for_one_taking() {
        let (mut app, port, merchant, captain) = harbour(Nation::ENGLAND);
        let jamaica_merchants = Nation::ENGLAND.groups()[2];
        app.world_mut().entity_mut(captain).insert(Docked { port });
        let before = app
            .world()
            .get::<Standing>(captain)
            .unwrap()
            .of(jamaica_merchants);
        let mut answered = Vec::new();
        for kind in [DeedKind::Attacked, DeedKind::Captured] {
            app.world_mut().write_message(Deed {
                doer: captain,
                kind,
                group: jamaica_merchants,
                about: Some(merchant),
            });
            answered.extend(
                deeds_known(&mut app, 1.0)
                    .iter()
                    .map(|known| known.answered),
            );
        }
        assert_eq!(answered, [None, Some(DeedKind::Attacked)]);
        let standing = app.world().get::<Standing>(captain).unwrap();
        assert_eq!(
            standing.of(jamaica_merchants) - before,
            DeedKind::Captured.standing()
        );
    }

    /// A ship lost with the captain and the Ranger about: `fate` is how.
    fn lost_with_us_about(
        app: &mut App,
        merchant: Entity,
        captain: Entity,
        fate: impl FnOnce(Entity) -> Fate,
    ) -> Entity {
        let ranger = app
            .world_mut()
            .spawn((Name::new("Ranger"), Transform::default()))
            .id();
        app.world_mut().resource_mut::<Ledger>().note_loss(Loss {
            ship: merchant,
            name: "Dove".to_string(),
            group: Some(Nation::ENGLAND.groups()[2]),
            fate: fate(ranger),
            at: 0.0,
            nearby: vec![
                (captain, "Player Sloop".to_string()),
                (ranger, "Ranger".to_string()),
            ],
        });
        ranger
    }

    #[test]
    fn a_captain_can_account_only_for_a_loss_she_was_about_for() {
        let (mut app, _, merchant, captain) = harbour(Nation::ENGLAND);
        let ranger = lost_with_us_about(&mut app, merchant, captain, Fate::Taken);
        let ledger = app.world().resource::<Ledger>();
        let loss = &ledger.losses[0];
        assert_eq!(
            loss.accounts(captain),
            [Fate::Wrecked, Fate::Taken(ranger), Fate::Sunk(ranger)],
            "she can't blame herself"
        );
        assert!(loss.accounts(merchant).is_empty(), "she wasn't about");
        assert_eq!(ledger.accountable(captain).count(), 1);
    }

    /// The captain, docked, says how the Dove was lost; then word of the
    /// taking itself comes in. The ledger after.
    fn account_then_the_truth(
        fate: impl FnOnce(Entity) -> Fate,
        says: impl FnOnce(Entity) -> Fate,
    ) -> Ledger {
        let (mut app, port, merchant, captain) = harbour(Nation::ENGLAND);
        let ranger = lost_with_us_about(&mut app, merchant, captain, fate);
        app.world_mut().entity_mut(captain).insert(Docked { port });
        app.world_mut().write_message(GiveAccount {
            ship: captain,
            lost: merchant,
            says: says(ranger),
        });
        run_seconds(&mut app, 1.0);
        let ledger = app.world().resource::<Ledger>();
        assert!(ledger.has_account(captain, merchant), "taken at her word");
        assert_eq!(ledger.accountable(captain).count(), 0, "once is enough");
        let taking = DeedNews {
            kind: DeedKind::Captured,
            group: Nation::ENGLAND.groups()[2],
            doer: ranger,
            doer_name: "Ranger".to_string(),
            captain: None,
            colours: None,
            victim: Some((merchant, "Dove".to_string())),
            standing: Vec::new(),
        };
        app.world_mut().resource_mut::<Ledger>().record(
            0.0,
            Place::Sea(Vec3::ZERO),
            Happening::Deed(taking),
            vec![port],
            Vec::new(),
            Vec::new(),
        );
        run_seconds(&mut app, 1.0);
        app.world().resource::<Ledger>().clone()
    }

    fn found_out(ledger: &Ledger) -> bool {
        ledger.news.iter().any(|news| {
            matches!(&news.event, Happening::Deed(deed) if deed.kind == DeedKind::FalseReport)
        })
    }

    #[test]
    fn a_false_account_is_found_out_where_word_of_the_loss_comes_in() {
        let ledger = account_then_the_truth(Fate::Taken, |_| Fate::Wrecked);
        assert!(found_out(&ledger), "{:?}", ledger.news);
        assert!(
            !ledger
                .news
                .iter()
                .any(|news| matches!(news.event, Happening::Account { .. })),
            "no one believes it now"
        );
    }

    #[test]
    fn a_true_account_stands() {
        let ledger = account_then_the_truth(Fate::Taken, Fate::Taken);
        assert!(!found_out(&ledger));
    }

    /// Nassau, 90 km off, and the Dove sailing there from the harbour with
    /// sugar. Returns Nassau and when she's due.
    fn bound_for_nassau(app: &mut App, port: Entity, merchant: Entity) -> (Entity, f64) {
        let nassau = app
            .world_mut()
            .spawn((
                Port {
                    flag: Nation::PIRATES,
                    town: Vec3::ZERO,
                    locations: vec![Location::Tavern],
                },
                Transform::from_xyz(0.0, 0.0, 90_000.0),
            ))
            .id();
        let goods = Goods::standard();
        app.world_mut().write_message(Sailed {
            ship: merchant,
            from: port,
            purpose: Purpose::Carrying {
                good: goods.id("sugar").unwrap(),
                quantity: 20,
                bound_for: nassau,
            },
        });
        app.update();
        let due = app
            .world()
            .resource::<Ledger>()
            .news
            .iter()
            .find_map(|news| match news.event {
                Happening::Sailed { due, .. } => due,
                _ => None,
            })
            .expect("she's due somewhere");
        (nassau, due)
    }

    #[test]
    fn a_ship_lost_unseen_is_posted_overdue_with_whoever_was_about() {
        let (mut app, port, merchant, captain) = harbour(Nation::ENGLAND);
        let (nassau, due) = bound_for_nassau(&mut app, port, merchant);
        assert!(due > 2.0 * 90_000.0 / 2.5, "twice a slow passage, and more");
        // The captain takes her, 20 km off, with no one in sight.
        app.world_mut()
            .get_mut::<Transform>(captain)
            .unwrap()
            .translation = Vec3::new(0.0, 0.0, 20_000.0);
        app.world_mut()
            .get_mut::<Transform>(merchant)
            .unwrap()
            .translation = Vec3::new(0.0, 0.0, 20_050.0);
        app.world_mut().write_message(Deed {
            doer: captain,
            kind: DeedKind::Captured,
            group: Nation::ENGLAND.groups()[2],
            about: Some(merchant),
        });
        app.update();
        app.world_mut().despawn(merchant);
        app.world_mut().resource_mut::<WorldClock>().elapsed = due + 1.0;
        run_seconds(&mut app, 1.0);
        let ledger = app.world().resource::<Ledger>();
        let overdue = ledger
            .news
            .iter()
            .find(|news| matches!(news.event, Happening::Overdue { .. }))
            .expect("posted overdue");
        assert_eq!(overdue.known_at, [nassau]);
        let Happening::Overdue { suspects, .. } = &overdue.event else {
            unreachable!();
        };
        assert_eq!(suspects, &[(captain, "Player Sloop".to_string())]);
        assert!(
            !ledger
                .news
                .iter()
                .any(|news| matches!(news.event, Happening::Sailed { .. })),
            "no one looks for her sailing now"
        );
    }

    #[test]
    fn a_ship_that_comes_in_is_never_posted_overdue() {
        let (mut app, port, merchant, _) = harbour(Nation::ENGLAND);
        let (nassau, due) = bound_for_nassau(&mut app, port, merchant);
        app.world_mut()
            .entity_mut(merchant)
            .insert(Docked { port: nassau });
        app.update();
        app.world_mut().resource_mut::<WorldClock>().elapsed = due + 1.0;
        run_seconds(&mut app, 1.0);
        let ledger = app.world().resource::<Ledger>();
        assert!(
            !ledger
                .news
                .iter()
                .any(|news| matches!(news.event, Happening::Overdue { .. })),
            "{:?}",
            ledger.news
        );
    }

    #[test]
    fn a_prizes_crew_rows_for_the_nearest_port_and_tells() {
        let (mut app, _, merchant, captain) = harbour(Nation::ENGLAND);
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(crate::time_scale::MAX_STEP);
        app.world_mut().write_message(Deed {
            doer: captain,
            kind: DeedKind::Captured,
            group: Nation::ENGLAND.groups()[2],
            about: Some(merchant),
        });
        app.update();
        // 50 km at a meter a second: about 14 hours in the boats.
        let rowing = 50_000.0 / BOAT_SPEED;
        assert!(
            deeds_known(&mut app, rowing * 0.9).is_empty(),
            "still rowing"
        );
        assert_eq!(deeds_known(&mut app, rowing * 0.2).len(), 1);
    }

    #[test]
    fn a_ship_sailing_carries_her_ports_news_to_the_next() {
        let (mut app, port, merchant, captain) = harbour(Nation::ENGLAND);
        let nassau = app
            .world_mut()
            .spawn((
                Port {
                    flag: Nation::PIRATES,
                    town: Vec3::ZERO,
                    locations: vec![Location::Tavern],
                },
                Transform::from_xyz(0.0, 0.0, 90_000.0),
            ))
            .id();
        app.world_mut().entity_mut(captain).insert(Docked { port });
        app.world_mut().write_message(Deed {
            doer: captain,
            kind: DeedKind::Delivered,
            group: Nation::ENGLAND.groups()[2],
            about: None,
        });
        assert_eq!(
            deeds_known(&mut app, 1.0).len(),
            1,
            "done in port, known there"
        );
        app.world_mut().entity_mut(merchant).insert(Docked { port });
        app.update();
        app.world_mut().entity_mut(merchant).remove::<Docked>();
        app.world_mut()
            .entity_mut(merchant)
            .insert(Docked { port: nassau });
        let known = deeds_known(&mut app, 1.0);
        assert_eq!(
            known.iter().map(|known| known.nation).collect::<Vec<_>>(),
            [Nation::PIRATES],
            "carried to Nassau"
        );
    }
}
