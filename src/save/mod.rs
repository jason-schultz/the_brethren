//! Saving and loading: the world's changing state as plain data.
//!
//! The map, goods, and ship classes are built into the game, so a save holds
//! only what changes: the clock, nations' relations, ports' stocks and
//! allegiance, ships (with their captains' standing and service),
//! contracts, escorts, officers, wrecks, and the ledger of news. Things refer to each other
//! by name rather than by entity, which keeps the file readable and stable. Ships under way are given their voyage again as an
//! ordinary [`SailOrder`], so routes are planned the usual way.

pub mod quicksave;

use std::fmt;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::alert::InSight;
use crate::allegiance::Favour;
use crate::cargo::{Hold, Lading, Purse};
use crate::clock::WorldClock;
use crate::combat::{Battery, Timbers};
use crate::crew::Crew;
use crate::economy::jobs::{Carrier, Job};
use crate::economy::market::Market;
use crate::escorts::Escort;
use crate::factions::{
    CaptainId, Colours, DeedKind, Group, Nation, Relations, Standing, groups, nations, pairs,
};
use crate::goods::{GoodId, Goods};
use crate::math::heading_to_rotation;
use crate::merchants::{Merchant, Plan};
use crate::navigation::{Arrival, Route, SailOrder};
use crate::officers::{Berth, Loyalty, Officer, Role};
use crate::patrols::Patrol;
use crate::player::Player;
use crate::port::{Docked, Port};
use crate::prizes::{Prize, PrizeOrders};
use crate::raiders::{Raid, Raider};
use crate::sailing::{Aground, Heading, Rigging, Speed};
use crate::service::{Post, Service};
use crate::ship_class::ShipClass;
use crate::word::{
    Answered, DeedNews, Fate, Happening, Heard, Knowledge, Ledger, Loss, News, NewsId, Place,
    Purpose, Tale,
};
use crate::wrecks::Wreck;

/// Bumped whenever the format changes incompatibly.
pub const SAVE_VERSION: u32 = 14;

/// A saved game.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SaveGame {
    pub version: u32,
    /// [`WorldClock`] seconds.
    pub clock: f64,
    /// How each pair of nations stands with each other.
    pub relations: Vec<([Nation; 2], f32)>,
    pub ports: Vec<PortSave>,
    pub ships: Vec<ShipSave>,
    pub jobs: Vec<JobSave>,
    pub escorts: Vec<EscortSave>,
    pub officers: Vec<OfficerSave>,
    pub wrecks: Vec<WreckSave>,
    /// The ledger of news.
    pub ledger: LedgerSave,
}

/// A [`Wreck`], with ships by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WreckSave {
    pub name: String,
    pub position: [f32; 3],
    pub nation: Option<Nation>,
    pub cargo: Cargo,
    pub purse: u32,
    pub until: f64,
    pub salvors: Vec<String>,
}

/// The [`Ledger`], with ships, ports, and goods by name.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct LedgerSave {
    pub next: u64,
    pub news: Vec<NewsSave>,
    pub losses: Vec<LossSave>,
    pub answered: Vec<AnsweredSave>,
}

/// An [`Answered`], with ships by name. Only fights between ships still
/// afloat are kept.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AnsweredSave {
    pub doer: String,
    pub victim: String,
    pub nation: Nation,
    pub kind: DeedKind,
    pub at: f64,
}

/// A [`Loss`], with ships by name, or `None` for those no longer afloat.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LossSave {
    pub ship: Option<String>,
    pub name: String,
    pub group: Option<Group>,
    pub fate: Fate<Option<String>>,
    pub at: f64,
    pub nearby: Vec<(Option<String>, String)>,
}

/// One piece of [`News`]. Ships it names that are no longer afloat are
/// `None`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NewsSave {
    pub id: u64,
    pub at: f64,
    pub place: PlaceSave,
    pub event: HappeningSave,
    pub known_at: Vec<String>,
    pub carriers: Vec<String>,
    pub boats: Vec<(String, f64)>,
    pub heard_by: Vec<Nation>,
}

/// A [`Place`], with ports by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PlaceSave {
    Port(String),
    Sea([f32; 3]),
}

/// A [`Happening`], with ships, ports, and goods by name, and `None` for a
/// ship no longer afloat.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum HappeningSave {
    Sailed {
        ship: Option<String>,
        name: String,
        purpose: PurposeSave,
        due: Option<f64>,
    },
    Overdue {
        ship: Option<String>,
        name: String,
        from: String,
        bound_for: String,
        suspects: Vec<(Option<String>, String)>,
    },
    Deed {
        kind: DeedKind,
        group: Group,
        doer: Option<String>,
        doer_name: String,
        captain: Option<u32>,
        colours: Option<Nation>,
        victim: Option<(Option<String>, String)>,
        standing: Vec<(Group, f32)>,
    },
    Wrecked {
        ship: Option<String>,
        name: String,
        nation: Option<Nation>,
        cargo: Option<(String, u32)>,
        wreck: Option<String>,
    },
    Account {
        teller: Option<String>,
        teller_name: String,
        captain: Option<u32>,
        ship: Option<String>,
        name: String,
        says: Fate<Option<String>>,
        blames: Option<String>,
    },
}

/// A merchant's request for an [`Escort`], with ships and ports by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EscortSave {
    /// `None` once she's lost.
    pub ward: Option<String>,
    pub nation: Nation,
    pub origin: String,
    pub destination: String,
    pub fee: u32,
    /// The ship that took it on, if any.
    pub escort: Option<String>,
    /// When the ward was lost, if she was.
    pub lost_at: Option<f64>,
}

/// A [`Purpose`], with ports and goods by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PurposeSave {
    Carrying {
        good: String,
        quantity: u32,
        bound_for: String,
    },
    Cruising,
}

/// How a captain [`Heard`] of something, with ports by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum HeardSave {
    Saw,
    Tavern(String),
    Service(Group),
}

/// An officer, with where he is by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OfficerSave {
    pub name: String,
    pub role: Role,
    pub loyalty: Loyalty,
    pub confined: bool,
    pub berth: BerthSave,
}

/// An officer's [`Berth`], with ships and ports by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum BerthSave {
    Aboard(String),
    Captive(String),
    ForHire {
        port: String,
        bounty: u32,
        until: f64,
    },
}

/// Quantities keyed by good, as the goods data names them.
pub type Cargo = Vec<(String, f64)>;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PortSave {
    pub name: String,
    /// Who holds it now.
    pub flag: Nation,
    /// How it regards each nation.
    pub favour: Vec<(Nation, f32)>,
    pub stock: Cargo,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ShipSave {
    pub name: String,
    pub class: String,
    pub captain: Captain,
    pub position: [f32; 3],
    pub heading: f32,
    pub speed: f32,
    pub aground: bool,
    pub guns: u32,
    /// Battle damage: 1 is sound.
    pub timbers: f32,
    pub rigging: f32,
    pub crew: Crew,
    /// The nation whose colours she flies, if any.
    pub colours: Option<Nation>,
    pub purse: u32,
    pub owned: Cargo,
    pub consigned: Cargo,
    /// The port she lies at.
    pub docked: Option<String>,
    pub voyage: Option<Voyage>,
    /// Her captain's name with the nations, if she has one.
    pub record: Option<RecordSave>,
}

/// A captain's record: her [`Standing`] with every group, the groups she
/// serves, and what she's heard.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RecordSave {
    pub captain: u32,
    pub standing: Vec<(Group, f32)>,
    pub service: Vec<Post>,
    pub knowledge: Vec<TaleSave>,
}

/// A [`Tale`] a captain heard, with ships, ports, and goods by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TaleSave {
    pub news: u64,
    pub at: f64,
    pub place: PlaceSave,
    pub event: HappeningSave,
    pub heard: HeardSave,
}

/// Who commands a ship.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Captain {
    Player,
    Merchant(PlanSave),
    /// A prize crew, under the orders of the ship that took her.
    Prize {
        captor: String,
        orders: OrdersSave,
    },
    /// A navy patrol, bound for this port on her beat.
    Patrol {
        next: usize,
    },
    Raider(Raid),
}

/// A prize's [`PrizeOrders`], with ports by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrdersSave {
    Follow,
    Wait,
    SailTo(String),
}

/// A merchant's [`Plan`], with ports by name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PlanSave {
    AtSea,
    InPort { until: f64 },
    Loading { bound_for: String, sails_at: f64 },
    Salvaging { wreck: String },
}

/// Where a ship under way is bound.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Voyage {
    pub destination: [f32; 3],
    /// The port to dock at on arrival, if any.
    pub dock_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JobSave {
    pub good: String,
    pub quantity: u32,
    pub origin: String,
    pub destination: String,
    pub payment: u32,
    pub deadline: f64,
    /// The ship carrying it, if taken.
    pub carrier: Option<String>,
    /// The group it introduces the player to, if it's an errand for her.
    pub introduction: Option<Group>,
}

/// Why a save can't be loaded. A failed load leaves the game untouched.
#[derive(Debug, PartialEq)]
pub enum LoadError {
    Parse(String),
    Version(u32),
    /// The save names a port, good, class, or ship this game doesn't have.
    Unknown(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "unreadable save: {error}"),
            Self::Version(version) => write!(f, "save is version {version}, need {SAVE_VERSION}"),
            Self::Unknown(what) => write!(f, "save mentions unknown {what}"),
        }
    }
}

impl SaveGame {
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .expect("save data always serializes")
    }

    pub fn from_ron(source: &str) -> Result<Self, LoadError> {
        let save: Self = ron::from_str(source).map_err(|e| LoadError::Parse(e.to_string()))?;
        if save.version != SAVE_VERSION {
            return Err(LoadError::Version(save.version));
        }
        Ok(save)
    }
}

/// Looks up entities and goods by name while loading.
struct Names {
    ports: Vec<(String, Entity)>,
    ships: Vec<(String, Entity)>,
}

impl Names {
    fn port(&self, name: &str) -> Result<Entity, LoadError> {
        find(&self.ports, name).ok_or_else(|| LoadError::Unknown(format!("port {name}")))
    }

    fn ship(&self, name: &str) -> Result<Entity, LoadError> {
        find(&self.ships, name).ok_or_else(|| LoadError::Unknown(format!("ship {name}")))
    }
}

fn find(names: &[(String, Entity)], name: &str) -> Option<Entity> {
    names.iter().find(|(n, _)| n == name).map(|&(_, e)| e)
}

fn good(goods: &Goods, key: &str) -> Result<GoodId, LoadError> {
    goods
        .id(key)
        .ok_or_else(|| LoadError::Unknown(format!("good {key}")))
}

fn cargo_save(goods: &Goods, lading: &Lading) -> Cargo {
    lading
        .iter()
        .map(|(good, quantity)| (goods.get(good).key.clone(), quantity))
        .collect()
}

fn cargo_load(goods: &Goods, cargo: &Cargo) -> Result<Lading, LoadError> {
    let mut lading = Lading::default();
    for (key, quantity) in cargo {
        lading.add(good(goods, key)?, *quantity);
    }
    Ok(lading)
}

fn name_of(names: &Query<&Name>, entity: Entity) -> String {
    names
        .get(entity)
        .map_or_else(|_| "?".into(), |n| n.to_string())
}

fn purpose_save(goods: &Goods, names: &Query<&Name>, purpose: Purpose) -> PurposeSave {
    match purpose {
        Purpose::Carrying {
            good,
            quantity,
            bound_for,
        } => PurposeSave::Carrying {
            good: goods.get(good).key.clone(),
            quantity,
            bound_for: name_of(names, bound_for),
        },
        Purpose::Cruising => PurposeSave::Cruising,
    }
}

fn purpose_load(goods: &Goods, names: &Names, purpose: &PurposeSave) -> Result<Purpose, LoadError> {
    Ok(match purpose {
        PurposeSave::Carrying {
            good: key,
            quantity,
            bound_for,
        } => Purpose::Carrying {
            good: good(goods, key)?,
            quantity: *quantity,
            bound_for: names.port(bound_for)?,
        },
        PurposeSave::Cruising => Purpose::Cruising,
    })
}

fn heard_save(names: &Query<&Name>, heard: Heard) -> HeardSave {
    match heard {
        Heard::Saw => HeardSave::Saw,
        Heard::Tavern(port) => HeardSave::Tavern(name_of(names, port)),
        Heard::Service(group) => HeardSave::Service(group),
    }
}

fn heard_load(names: &Names, heard: &HeardSave) -> Result<Heard, LoadError> {
    Ok(match heard {
        HeardSave::Saw => Heard::Saw,
        HeardSave::Tavern(port) => Heard::Tavern(names.port(port)?),
        HeardSave::Service(group) => Heard::Service(*group),
    })
}

/// A ship or wreck named in the news, if she's still about.
fn afloat_save(names: &Query<&Name>, ship: Entity) -> Option<String> {
    names.get(ship).ok().map(|name| name.to_string())
}

/// A ship or wreck named in the news, or nobody if she's gone.
fn afloat_load(names: &Names, ship: &Option<String>) -> Entity {
    ship.as_deref()
        .and_then(|name| find(&names.ships, name))
        .unwrap_or(Entity::PLACEHOLDER)
}

fn place_save(names: &Query<&Name>, place: Place) -> PlaceSave {
    match place {
        Place::Port(port) => PlaceSave::Port(name_of(names, port)),
        Place::Sea(at) => PlaceSave::Sea(at.to_array()),
    }
}

fn place_load(names: &Names, place: &PlaceSave) -> Result<Place, LoadError> {
    Ok(match place {
        PlaceSave::Port(port) => Place::Port(names.port(port)?),
        PlaceSave::Sea(at) => Place::Sea(Vec3::from_array(*at)),
    })
}

fn happening_save(goods: &Goods, names: &Query<&Name>, event: &Happening) -> HappeningSave {
    match event {
        Happening::Sailed {
            ship,
            name,
            purpose,
            due,
        } => HappeningSave::Sailed {
            ship: afloat_save(names, *ship),
            name: name.clone(),
            purpose: purpose_save(goods, names, *purpose),
            due: *due,
        },
        Happening::Overdue {
            ship,
            name,
            from,
            bound_for,
            suspects,
        } => HappeningSave::Overdue {
            ship: afloat_save(names, *ship),
            name: name.clone(),
            from: name_of(names, *from),
            bound_for: name_of(names, *bound_for),
            suspects: suspects
                .iter()
                .map(|(ship, name)| (afloat_save(names, *ship), name.clone()))
                .collect(),
        },
        Happening::Deed(deed) => HappeningSave::Deed {
            kind: deed.kind,
            group: deed.group,
            doer: afloat_save(names, deed.doer),
            doer_name: deed.doer_name.clone(),
            captain: deed.captain.map(|captain| captain.0),
            colours: deed.colours,
            victim: deed
                .victim
                .as_ref()
                .map(|(ship, name)| (afloat_save(names, *ship), name.clone())),
            standing: deed.standing.clone(),
        },
        Happening::Wrecked {
            ship,
            name,
            nation,
            cargo,
            wreck,
        } => HappeningSave::Wrecked {
            ship: afloat_save(names, *ship),
            name: name.clone(),
            nation: *nation,
            cargo: cargo.map(|(good, tons)| (goods.get(good).key.clone(), tons)),
            wreck: wreck.and_then(|wreck| afloat_save(names, wreck)),
        },
        Happening::Account {
            teller,
            teller_name,
            captain,
            ship,
            name,
            says,
            blames,
        } => HappeningSave::Account {
            teller: afloat_save(names, *teller),
            teller_name: teller_name.clone(),
            captain: captain.map(|captain| captain.0),
            ship: afloat_save(names, *ship),
            name: name.clone(),
            says: says.map(|by| afloat_save(names, by)),
            blames: blames.clone(),
        },
    }
}

fn happening_load(
    goods: &Goods,
    names: &Names,
    event: &HappeningSave,
) -> Result<Happening, LoadError> {
    Ok(match event {
        HappeningSave::Sailed {
            ship,
            name,
            purpose,
            due,
        } => Happening::Sailed {
            ship: afloat_load(names, ship),
            name: name.clone(),
            purpose: purpose_load(goods, names, purpose)?,
            due: *due,
        },
        HappeningSave::Overdue {
            ship,
            name,
            from,
            bound_for,
            suspects,
        } => Happening::Overdue {
            ship: afloat_load(names, ship),
            name: name.clone(),
            from: names.port(from)?,
            bound_for: names.port(bound_for)?,
            suspects: suspects
                .iter()
                .map(|(ship, name)| (afloat_load(names, ship), name.clone()))
                .collect(),
        },
        HappeningSave::Deed {
            kind,
            group,
            doer,
            doer_name,
            captain,
            colours,
            victim,
            standing,
        } => Happening::Deed(DeedNews {
            kind: *kind,
            group: *group,
            doer: afloat_load(names, doer),
            doer_name: doer_name.clone(),
            captain: captain.map(CaptainId),
            colours: *colours,
            victim: victim
                .as_ref()
                .map(|(ship, name)| (afloat_load(names, ship), name.clone())),
            standing: standing.clone(),
        }),
        HappeningSave::Wrecked {
            ship,
            name,
            nation,
            cargo,
            wreck,
        } => Happening::Wrecked {
            ship: afloat_load(names, ship),
            name: name.clone(),
            nation: *nation,
            cargo: cargo
                .as_ref()
                .map(|(key, tons)| Ok::<_, LoadError>((good(goods, key)?, *tons)))
                .transpose()?,
            wreck: wreck.as_ref().map(|_| afloat_load(names, wreck)),
        },
        HappeningSave::Account {
            teller,
            teller_name,
            captain,
            ship,
            name,
            says,
            blames,
        } => Happening::Account {
            teller: afloat_load(names, teller),
            teller_name: teller_name.clone(),
            captain: captain.map(CaptainId),
            ship: afloat_load(names, ship),
            name: name.clone(),
            says: says.clone().map(|by| afloat_load(names, &by)),
            blames: blames.clone(),
        },
    })
}

fn tale_save(goods: &Goods, names: &Query<&Name>, tale: &Tale) -> TaleSave {
    TaleSave {
        news: tale.news.0,
        at: tale.at,
        place: place_save(names, tale.place),
        event: happening_save(goods, names, &tale.event),
        heard: heard_save(names, tale.heard),
    }
}

fn tale_load(goods: &Goods, names: &Names, tale: &TaleSave) -> Result<Tale, LoadError> {
    Ok(Tale {
        news: NewsId(tale.news),
        at: tale.at,
        place: place_load(names, &tale.place)?,
        event: happening_load(goods, names, &tale.event)?,
        heard: heard_load(names, &tale.heard)?,
    })
}

/// Captures the world's state.
pub fn capture(world: &mut World) -> SaveGame {
    world
        .run_system_cached(capture_system)
        .expect("capturing reads only resources every simulation has")
}

fn capture_system(
    clock: Res<WorldClock>,
    relations: Res<Relations>,
    goods: Res<Goods>,
    names: Query<&Name>,
    ports: Query<(&Name, &Port, &Favour, &Market)>,
    ships: Query<(
        &Name,
        &ShipClass,
        (
            Has<Player>,
            Option<&Merchant>,
            Option<(&Prize, &PrizeOrders)>,
            Option<&Patrol>,
            Option<&Raider>,
        ),
        &Transform,
        (&Heading, &Speed, &Aground),
        (&Battery, &Timbers, &Rigging),
        (&Crew, &Purse, &Hold, Option<&Colours>),
        Option<&Docked>,
        (
            Option<&Route>,
            Option<(&CaptainId, &Standing, &Service, &Knowledge)>,
        ),
    )>,
    jobs: Query<(&Job, Option<&Carrier>)>,
    escorts: Query<&Escort>,
    officers: Query<(&Name, &Officer, &Berth)>,
    wrecks: Query<(&Name, &Transform, &Wreck)>,
    ledger: Res<Ledger>,
) -> SaveGame {
    let ports = ports
        .iter()
        .map(|(name, port, favour, market)| PortSave {
            name: name.to_string(),
            flag: port.flag,
            favour: nations()
                .map(|nation| (nation, favour.of(nation)))
                .collect(),
            stock: market
                .entries
                .iter()
                .map(|entry| (goods.get(entry.good).key.clone(), entry.stock))
                .collect(),
        })
        .collect();
    let ships: Vec<ShipSave> = ships
        .iter()
        .filter_map(
            |(
                name,
                class,
                command,
                transform,
                motion,
                fighting,
                aboard,
                docked,
                (route, record),
            )| {
                let (player, merchant, prize, patrol, raider) = command;
                let captain = match (player, merchant, prize) {
                    (true, ..) => Captain::Player,
                    (false, _, None) if let Some(raider) = raider => Captain::Raider(raider.plan),
                    (false, _, None) if let Some(patrol) = patrol => {
                        Captain::Patrol { next: patrol.next }
                    }
                    (false, _, Some((prize, orders))) => Captain::Prize {
                        captor: name_of(&names, prize.captor),
                        orders: match *orders {
                            PrizeOrders::Follow => OrdersSave::Follow,
                            PrizeOrders::Wait => OrdersSave::Wait,
                            PrizeOrders::SailTo(port) => OrdersSave::SailTo(name_of(&names, port)),
                        },
                    },
                    (false, Some(merchant), None) => Captain::Merchant(match merchant.plan {
                        Plan::AtSea => PlanSave::AtSea,
                        Plan::InPort { until } => PlanSave::InPort { until },
                        Plan::Loading {
                            bound_for,
                            sails_at,
                        } => PlanSave::Loading {
                            bound_for: name_of(&names, bound_for),
                            sails_at,
                        },
                        Plan::Salvaging { wreck } => PlanSave::Salvaging {
                            wreck: name_of(&names, wreck),
                        },
                    }),
                    // A ship nobody commands has nothing to save.
                    (false, None, None) => return None,
                };
                let (heading, speed, aground) = motion;
                let (battery, timbers, rigging) = fighting;
                let (crew, purse, hold, colours) = aboard;
                Some(ShipSave {
                    name: name.to_string(),
                    class: class.name.to_string(),
                    captain,
                    position: transform.translation.to_array(),
                    heading: heading.0,
                    speed: speed.0,
                    aground: aground.0,
                    guns: battery.guns,
                    timbers: timbers.0,
                    rigging: rigging.0,
                    crew: *crew,
                    colours: colours.map(|colours| colours.0),
                    purse: purse.0,
                    owned: cargo_save(&goods, &hold.owned),
                    consigned: cargo_save(&goods, &hold.consigned),
                    docked: docked.map(|docked| name_of(&names, docked.port)),
                    record: record.map(|(id, standing, service, knowledge)| RecordSave {
                        captain: id.0,
                        standing: groups().map(|group| (group, standing.of(group))).collect(),
                        service: service.0.clone(),
                        knowledge: knowledge
                            .0
                            .iter()
                            .map(|tale| tale_save(&goods, &names, tale))
                            .collect(),
                    }),
                    voyage: route.and_then(|route| {
                        Some(Voyage {
                            destination: route.waypoints.back()?.to_array(),
                            dock_at: match route.on_arrival {
                                Arrival::Dock(port) => Some(name_of(&names, port)),
                                Arrival::HeaveTo => None,
                            },
                        })
                    }),
                })
            },
        )
        .collect();
    let saved_names: Vec<String> = ships.iter().map(|ship| ship.name.clone()).collect();
    let saved = |ship| {
        names
            .get(ship)
            .is_ok_and(|name| saved_names.iter().any(|saved| saved == name.as_str()))
    };
    let jobs = jobs
        .iter()
        .map(|(job, carrier)| JobSave {
            good: goods.get(job.good).key.clone(),
            quantity: job.quantity,
            origin: name_of(&names, job.origin),
            destination: name_of(&names, job.destination),
            payment: job.payment,
            deadline: job.deadline,
            carrier: carrier.map(|carrier| name_of(&names, carrier.0)),
            introduction: job.introduction,
        })
        .collect();
    let officers = officers
        .iter()
        .map(|(name, officer, berth)| OfficerSave {
            name: name.to_string(),
            role: officer.role,
            loyalty: officer.loyalty,
            confined: officer.confined,
            berth: match *berth {
                Berth::Aboard(ship) => BerthSave::Aboard(name_of(&names, ship)),
                Berth::Captive(ship) => BerthSave::Captive(name_of(&names, ship)),
                Berth::ForHire {
                    port,
                    bounty,
                    until,
                } => BerthSave::ForHire {
                    port: name_of(&names, port),
                    bounty,
                    until,
                },
            },
        })
        .collect();
    SaveGame {
        version: SAVE_VERSION,
        clock: clock.elapsed,
        relations: pairs()
            .map(|pair| (pair, relations.between(pair[0], pair[1])))
            .collect(),
        ports,
        ships,
        jobs,
        // Only what names saved ships: a save naming a ship it hasn't got
        // can't be loaded.
        escorts: escorts
            .iter()
            .filter(|escort| {
                (saved(escort.ward) || escort.lost_at.is_some()) && escort.escort.is_none_or(saved)
            })
            .map(|escort| EscortSave {
                ward: afloat_save(&names, escort.ward),
                nation: escort.nation,
                origin: name_of(&names, escort.origin),
                destination: name_of(&names, escort.destination),
                fee: escort.fee,
                escort: escort.escort.map(|ship| name_of(&names, ship)),
                lost_at: escort.lost_at,
            })
            .collect(),
        officers,
        wrecks: wrecks
            .iter()
            .map(|(name, at, wreck)| WreckSave {
                name: name.to_string(),
                position: at.translation.to_array(),
                nation: wreck.nation,
                cargo: cargo_save(&goods, &wreck.cargo),
                purse: wreck.purse,
                until: wreck.until,
                salvors: wreck
                    .salvors
                    .iter()
                    .filter_map(|&ship| afloat_save(&names, ship))
                    .collect(),
            })
            .collect(),
        ledger: LedgerSave {
            next: ledger.next.0,
            losses: ledger
                .losses
                .iter()
                .map(|loss| LossSave {
                    ship: afloat_save(&names, loss.ship),
                    name: loss.name.clone(),
                    group: loss.group,
                    fate: loss.fate.map(|by| afloat_save(&names, by)),
                    at: loss.at,
                    nearby: loss
                        .nearby
                        .iter()
                        .map(|(ship, name)| (afloat_save(&names, *ship), name.clone()))
                        .collect(),
                })
                .collect(),
            answered: ledger
                .answered
                .iter()
                .filter_map(|answered| {
                    Some(AnsweredSave {
                        doer: afloat_save(&names, answered.doer)?,
                        victim: afloat_save(&names, answered.victim)?,
                        nation: answered.nation,
                        kind: answered.kind,
                        at: answered.at,
                    })
                })
                .collect(),
            news: ledger
                .news
                .iter()
                .map(|news| NewsSave {
                    id: news.id.0,
                    at: news.at,
                    place: place_save(&names, news.place),
                    event: happening_save(&goods, &names, &news.event),
                    known_at: news
                        .known_at
                        .iter()
                        .map(|&port| name_of(&names, port))
                        .collect(),
                    carriers: news
                        .carriers
                        .iter()
                        .filter_map(|&ship| afloat_save(&names, ship))
                        .collect(),
                    boats: news
                        .boats
                        .iter()
                        .map(|&(port, arrives)| (name_of(&names, port), arrives))
                        .collect(),
                    heard_by: news.heard_by.clone(),
                })
                .collect(),
        },
    }
}

/// Replaces the world's ships, contracts, stocks, and clock with the save's.
/// Checks everything the save names before changing anything.
pub fn restore(world: &mut World, save: &SaveGame) -> Result<(), LoadError> {
    let goods = world.resource::<Goods>().clone();
    let ports = world
        .query_filtered::<(&Name, Entity), With<Port>>()
        .iter(world)
        .map(|(name, entity)| (name.to_string(), entity))
        .collect();
    let mut names = Names {
        ports,
        ships: Vec::new(),
    };
    let ships = save
        .ships
        .iter()
        .map(|ship| ship_bundle(&goods, &names, ship))
        .collect::<Result<Vec<_>, _>>()?;
    let mut stocks = Vec::new();
    for port in &save.ports {
        let entity = names.port(&port.name)?;
        for (key, stock) in &port.stock {
            stocks.push((entity, good(&goods, key)?, *stock));
        }
    }
    let ship_names: Vec<&str> = save.ships.iter().map(|ship| ship.name.as_str()).collect();
    for ship in &save.ships {
        if let Captain::Prize { captor, orders } = &ship.captain {
            if !ship_names.contains(&captor.as_str()) {
                return Err(LoadError::Unknown(format!("ship {captor}")));
            }
            if let OrdersSave::SailTo(port) = orders {
                names.port(port)?;
            }
        }
    }
    for officer in &save.officers {
        match &officer.berth {
            BerthSave::Aboard(ship) | BerthSave::Captive(ship) => {
                if !ship_names.contains(&ship.as_str()) {
                    return Err(LoadError::Unknown(format!("ship {ship}")));
                }
            }
            BerthSave::ForHire { port, .. } => {
                names.port(port)?;
            }
        }
    }
    for job in &save.jobs {
        good(&goods, &job.good)?;
        names.port(&job.origin)?;
        names.port(&job.destination)?;
        if let Some(carrier) = &job.carrier
            && !ship_names.contains(&carrier.as_str())
        {
            return Err(LoadError::Unknown(format!("ship {carrier}")));
        }
    }
    let known_ship = |ship: &String| {
        if ship_names.contains(&ship.as_str()) {
            Ok(())
        } else {
            Err(LoadError::Unknown(format!("ship {ship}")))
        }
    };
    for escort in &save.escorts {
        escort.ward.iter().try_for_each(known_ship)?;
        escort.escort.iter().try_for_each(known_ship)?;
        names.port(&escort.origin)?;
        names.port(&escort.destination)?;
    }
    for wreck in &save.wrecks {
        cargo_load(&goods, &wreck.cargo)?;
    }
    // The news may name ships long gone, but its ports and goods must be
    // this game's.
    for news in &save.ledger.news {
        place_load(&names, &news.place)?;
        happening_load(&goods, &names, &news.event)?;
        for port in news
            .known_at
            .iter()
            .chain(news.boats.iter().map(|(port, _)| port))
        {
            names.port(port)?;
        }
    }
    for tale in save
        .ships
        .iter()
        .filter_map(|ship| ship.record.as_ref())
        .flat_map(|record| &record.knowledge)
    {
        tale_load(&goods, &names, tale)?;
    }

    // Everything checks out: replace the world's state.
    world.resource_mut::<WorldClock>().elapsed = save.clock;
    let mut relations = world.resource_mut::<Relations>();
    for &([a, b], value) in &save.relations {
        relations.set(a, b, value);
    }
    for saved in &save.ports {
        let port = names.port(&saved.name)?;
        if let Some(mut held) = world.get_mut::<Port>(port) {
            held.flag = saved.flag;
        }
        if let Some(mut favour) = world.get_mut::<Favour>(port) {
            for &(nation, value) in &saved.favour {
                favour.set(nation, value);
            }
        }
    }
    for (port, good, stock) in stocks {
        if let Some(entry) = world
            .get_mut::<Market>(port)
            .and_then(|market| market.into_inner().entry_mut(good))
        {
            entry.stock = stock;
        }
    }
    let old: Vec<Entity> = world
        .query_filtered::<Entity, Or<(
            With<ShipClass>,
            With<Job>,
            With<Escort>,
            With<Officer>,
            With<Wreck>,
        )>>()
        .iter(world)
        .collect();
    for entity in old {
        world.despawn(entity);
    }
    for (saved, (bundle, docked)) in save.ships.iter().zip(ships) {
        let mut ship = world.spawn(bundle);
        ship.insert(saved.crew);
        match saved.captain {
            Captain::Player => {
                ship.insert((Player, InSight::default()));
            }
            Captain::Patrol { next } => {
                ship.insert(Patrol::bound_for(next));
            }
            Captain::Raider(plan) => {
                ship.insert(Raider { plan });
            }
            Captain::Merchant(_) | Captain::Prize { .. } => {}
        }
        if let Some(nation) = saved.colours {
            ship.insert(Colours(nation));
        }

        if let Some(docked) = docked {
            ship.insert(docked);
        }
        names.ships.push((saved.name.clone(), ship.id()));
    }
    for wreck in &save.wrecks {
        let entity = world
            .spawn((
                Name::new(wreck.name.clone()),
                Wreck {
                    nation: wreck.nation,
                    cargo: cargo_load(&goods, &wreck.cargo)?,
                    purse: wreck.purse,
                    until: wreck.until,
                    salvors: Vec::new(),
                },
                Transform::from_translation(Vec3::from_array(wreck.position)),
            ))
            .id();
        names.ships.push((wreck.name.clone(), entity));
    }
    for wreck in &save.wrecks {
        let salvors = wreck
            .salvors
            .iter()
            .filter_map(|ship| find(&names.ships, ship))
            .collect();
        let entity = names.ship(&wreck.name)?;
        if let Some(mut saved) = world.get_mut::<Wreck>(entity) {
            saved.salvors = salvors;
        }
    }
    // Merchant plans, voyages, and what captains have heard name ports and
    // ships, so they go in once every ship exists.
    for ship in &save.ships {
        let entity = names.ship(&ship.name)?;
        if let Some(record) = &ship.record {
            let mut standing = Standing::default();
            for &(group, value) in &record.standing {
                standing.set_group(group, value);
            }
            let knowledge = record
                .knowledge
                .iter()
                .map(|tale| tale_load(&goods, &names, tale))
                .collect::<Result<_, LoadError>>()?;
            world.entity_mut(entity).insert((
                CaptainId(record.captain),
                standing,
                Service(record.service.clone()),
                Knowledge(knowledge),
            ));
        }
        if let Captain::Prize { captor, orders } = &ship.captain {
            let captor = names.ship(captor)?;
            let orders = match orders {
                OrdersSave::Follow => PrizeOrders::Follow,
                OrdersSave::Wait => PrizeOrders::Wait,
                OrdersSave::SailTo(port) => PrizeOrders::SailTo(names.port(port)?),
            };
            world.entity_mut(entity).insert((Prize { captor }, orders));
        }
        if let Captain::Merchant(plan) = &ship.captain {
            let plan = match plan {
                PlanSave::AtSea => Plan::AtSea,
                PlanSave::InPort { until } => Plan::InPort { until: *until },
                PlanSave::Loading {
                    bound_for,
                    sails_at,
                } => Plan::Loading {
                    bound_for: names.port(bound_for)?,
                    sails_at: *sails_at,
                },
                // A wreck gone since is nothing to dive on: she'll make port.
                PlanSave::Salvaging { wreck } => {
                    find(&names.ships, wreck).map_or(Plan::AtSea, |wreck| Plan::Salvaging { wreck })
                }
            };
            world.entity_mut(entity).insert(Merchant { plan });
        }
        if let Some(voyage) = &ship.voyage {
            let on_arrival = match &voyage.dock_at {
                Some(port) => Arrival::Dock(names.port(port)?),
                None => Arrival::HeaveTo,
            };
            world.write_message(SailOrder {
                ship: entity,
                destination: Vec3::from_array(voyage.destination),
                on_arrival,
            });
        }
    }
    for officer in &save.officers {
        let berth = match &officer.berth {
            BerthSave::Aboard(ship) => Berth::Aboard(names.ship(ship)?),
            BerthSave::Captive(ship) => Berth::Captive(names.ship(ship)?),
            BerthSave::ForHire {
                port,
                bounty,
                until,
            } => Berth::ForHire {
                port: names.port(port)?,
                bounty: *bounty,
                until: *until,
            },
        };
        world.spawn((
            Name::new(officer.name.clone()),
            Officer {
                role: officer.role,
                loyalty: officer.loyalty,
                confined: officer.confined,
            },
            berth,
        ));
    }
    for job in &save.jobs {
        let mut entity = world.spawn(Job {
            good: good(&goods, &job.good)?,
            quantity: job.quantity,
            origin: names.port(&job.origin)?,
            destination: names.port(&job.destination)?,
            payment: job.payment,
            deadline: job.deadline,
            introduction: job.introduction,
        });
        if let Some(carrier) = &job.carrier {
            entity.insert(Carrier(names.ship(carrier)?));
        }
    }
    for escort in &save.escorts {
        let escort = Escort {
            ward: afloat_load(&names, &escort.ward),
            nation: escort.nation,
            origin: names.port(&escort.origin)?,
            destination: names.port(&escort.destination)?,
            fee: escort.fee,
            escort: escort
                .escort
                .as_ref()
                .map(|ship| names.ship(ship))
                .transpose()?,
            lost_at: escort.lost_at,
        };
        world.spawn(escort);
    }
    let news = save
        .ledger
        .news
        .iter()
        .map(|news| {
            Ok(News {
                id: NewsId(news.id),
                at: news.at,
                place: place_load(&names, &news.place)?,
                event: happening_load(&goods, &names, &news.event)?,
                known_at: news
                    .known_at
                    .iter()
                    .map(|port| names.port(port))
                    .collect::<Result<_, _>>()?,
                carriers: news
                    .carriers
                    .iter()
                    .filter_map(|ship| find(&names.ships, ship))
                    .collect(),
                boats: news
                    .boats
                    .iter()
                    .map(|(port, arrives)| Ok((names.port(port)?, *arrives)))
                    .collect::<Result<_, LoadError>>()?,
                heard_by: news.heard_by.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoadError>>()?;
    let losses = save
        .ledger
        .losses
        .iter()
        .map(|loss| Loss {
            ship: afloat_load(&names, &loss.ship),
            name: loss.name.clone(),
            group: loss.group,
            fate: loss.fate.clone().map(|by| afloat_load(&names, &by)),
            at: loss.at,
            nearby: loss
                .nearby
                .iter()
                .map(|(ship, name)| (afloat_load(&names, ship), name.clone()))
                .collect(),
        })
        .collect();
    let answered = save
        .ledger
        .answered
        .iter()
        .map(|answered| {
            Ok(Answered {
                doer: names.ship(&answered.doer)?,
                victim: names.ship(&answered.victim)?,
                nation: answered.nation,
                kind: answered.kind,
                at: answered.at,
            })
        })
        .collect::<Result<_, LoadError>>()?;
    *world.resource_mut::<Ledger>() = Ledger {
        news,
        next: NewsId(save.ledger.next),
        losses,
        answered,
    };
    Ok(())
}

/// A saved ship's components and berth, checked against this game's data.
/// Her crew, captain, and berth go in after spawning: [`ShipClass::build`]
/// brings a fresh crew, and docking the player opens the port screen.
fn ship_bundle(
    goods: &Goods,
    names: &Names,
    ship: &ShipSave,
) -> Result<(impl Bundle, Option<Docked>), LoadError> {
    let class = ShipClass::named(&ship.class)
        .ok_or_else(|| LoadError::Unknown(format!("ship class {}", ship.class)))?;
    let mut hold = class.empty_hold();
    hold.owned = cargo_load(goods, &ship.owned)?;
    hold.consigned = cargo_load(goods, &ship.consigned)?;
    let docked = ship
        .docked
        .as_ref()
        .map(|port| names.port(port).map(|port| Docked { port }))
        .transpose()?;
    if let Captain::Merchant(PlanSave::Loading { bound_for, .. }) = &ship.captain {
        names.port(bound_for)?;
    }
    if let Some(Voyage {
        dock_at: Some(port),
        ..
    }) = &ship.voyage
    {
        names.port(port)?;
    }
    let bundle = (
        Name::new(ship.name.clone()),
        class.build(ship.crew.count, ship.guns, hold),
        Timbers(ship.timbers),
        Rigging(ship.rigging),
        Purse(ship.purse),
        Transform::from_translation(Vec3::from_array(ship.position))
            .with_rotation(heading_to_rotation(ship.heading)),
        Heading(ship.heading),
        Speed(ship.speed),
        Aground(ship.aground),
    );
    Ok((bundle, docked))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::DAY;
    use crate::new_game::{calm_seas, new_game};
    use crate::test_support::run_until;
    use crate::time_scale::MAX_STEP;

    /// A new game run for a day and a half at full time compression, so
    /// merchants are at sea and contracts are in hand.
    fn game_in_progress() -> (App, Entity) {
        let (mut app, player) = new_game();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        run_until(&mut app, 1.5 * DAY as f32, |_| false);
        (app, player)
    }

    /// The save with every list in name order, since query order isn't stable.
    fn sorted(mut save: SaveGame) -> SaveGame {
        save.ports.sort_by(|a, b| a.name.cmp(&b.name));
        save.ships.sort_by(|a, b| a.name.cmp(&b.name));
        save.jobs
            .sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
        save.officers
            .sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
        save.escorts
            .sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
        save.wrecks.sort_by(|a, b| a.name.cmp(&b.name));
        save
    }

    /// The entity with this name.
    fn named(app: &mut App, name: &str) -> Entity {
        app.world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, n)| n.as_str() == name)
            .unwrap()
            .0
    }

    #[test]
    fn a_save_survives_the_file_format() {
        let (mut app, _) = game_in_progress();
        let save = capture(app.world_mut());
        assert_eq!(SaveGame::from_ron(&save.to_ron()), Ok(save));
    }

    #[test]
    fn loading_a_save_restores_exactly_what_was_saved() {
        let (mut app, _) = game_in_progress();
        let save = sorted(capture(app.world_mut()));
        assert!(
            save.ships.iter().any(|ship| ship.voyage.is_some()),
            "a ship should be under way: {save:?}"
        );
        assert!(!save.ledger.news.is_empty(), "ships have sailed: {save:?}");

        let (mut fresh, _) = new_game();
        restore(fresh.world_mut(), &save).unwrap();
        // Let the reissued sail orders become routes without moving anyone.
        fresh.world_mut().resource_mut::<Time<Virtual>>().pause();
        fresh.update();
        assert_eq!(sorted(capture(fresh.world_mut())), save);
    }

    #[test]
    fn prizes_battle_damage_standing_service_relations_escorts_officers_and_morale_are_saved() {
        let (mut app, player) = game_in_progress();
        // Raiders may have taken some merchants by now: any will do.
        let (prize, prize_name) = app
            .world_mut()
            .query_filtered::<(Entity, &Name), With<Merchant>>()
            .iter(app.world())
            .map(|(entity, name)| (entity, name.to_string()))
            .next()
            .unwrap();
        app.world_mut()
            .entity_mut(prize)
            .remove::<Merchant>()
            .insert((Prize { captor: player }, Timbers(0.4), Rigging(0.7)));
        app.world_mut()
            .get_mut::<Standing>(player)
            .unwrap()
            .set(Nation::ENGLAND, -40.0);
        let flying_gang = Nation::PIRATES.groups()[0];
        app.world_mut().get_mut::<Service>(player).unwrap().0 = vec![Post {
            group: flying_gang,
            rank: 1,
        }];
        app.world_mut()
            .resource_mut::<Relations>()
            .set(Nation::ENGLAND, Nation::PIRATES, -70.0);
        app.world_mut().spawn((
            Name::new("Israel Hands"),
            Officer {
                role: Role::Gunner,
                loyalty: Loyalty::Turncoat,
                confined: true,
            },
            Berth::Aboard(player),
        ));
        app.world_mut().get_mut::<Crew>(player).unwrap().morale = 23.0;
        let (kingston, nassau) = (named(&mut app, "Kingston"), named(&mut app, "Nassau"));
        app.world_mut().spawn(Escort {
            ward: prize,
            nation: Nation::ENGLAND,
            origin: kingston,
            destination: nassau,
            fee: 75,
            escort: Some(player),
            lost_at: None,
        });
        let save = sorted(capture(app.world_mut()));
        assert!(save.escorts.contains(&EscortSave {
            ward: Some(prize_name.clone()),
            nation: Nation::ENGLAND,
            origin: "Kingston".into(),
            destination: "Nassau".into(),
            fee: 75,
            escort: Some("Player Sloop".into()),
            lost_at: None,
        }));
        let jamaica_merchants = Nation::ENGLAND.groups()[2];
        let record = save
            .ships
            .iter()
            .find_map(|ship| ship.record.as_ref())
            .expect("the player's record");
        assert!(record.standing.contains(&(jamaica_merchants, -40.0)));
        assert_eq!(record.service[0].group, flying_gang);
        assert!(
            save.relations
                .contains(&([Nation::ENGLAND, Nation::PIRATES], -70.0))
        );
        assert!(
            save.ships
                .iter()
                .any(|ship| matches!(ship.captain, Captain::Raider(_))),
            "the raiders"
        );
        assert!(
            save.officers
                .iter()
                .any(|officer| officer.name == "Israel Hands"
                    && officer.berth == BerthSave::Aboard("Player Sloop".into()))
        );
        assert!(
            save.officers
                .iter()
                .any(|officer| matches!(officer.berth, BerthSave::ForHire { .. })),
            "the taverns' officers"
        );
        let dove = save
            .ships
            .iter()
            .find(|ship| ship.name == prize_name)
            .unwrap();
        assert_eq!(
            dove.captain,
            Captain::Prize {
                captor: "Player Sloop".into(),
                orders: OrdersSave::Follow,
            }
        );
        assert_eq!((dove.timbers, dove.rigging), (0.4, 0.7));

        let (mut fresh, _) = new_game();
        restore(fresh.world_mut(), &save).unwrap();
        fresh.world_mut().resource_mut::<Time<Virtual>>().pause();
        fresh.update();
        assert_eq!(sorted(capture(fresh.world_mut())), save);
    }

    #[test]
    fn a_voyage_carries_on_after_loading() {
        let (mut app, player) = new_game();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        calm_seas(&mut app);
        let nassau = named(&mut app, "Nassau");
        let anchorage = app.world().get::<Transform>(nassau).unwrap().translation;
        app.world_mut()
            .write_message(crate::port::CastOff { ship: player });
        app.world_mut().write_message(SailOrder {
            ship: player,
            destination: anchorage,
            on_arrival: Arrival::Dock(nassau),
        });
        run_until(&mut app, 10.0 * crate::clock::HOUR as f32, |_| false);
        let save = capture(app.world_mut());

        let (mut fresh, _) = new_game();
        fresh
            .world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        restore(fresh.world_mut(), &save).unwrap();
        calm_seas(&mut fresh);
        let nassau = named(&mut fresh, "Nassau");
        let player = named(&mut fresh, "Player Sloop");
        let docked = run_until(&mut fresh, 40.0 * crate::clock::HOUR as f32, |world| {
            world.get::<Docked>(player) == Some(&Docked { port: nassau })
        });
        assert!(docked, "never reached Nassau after loading");
    }

    #[test]
    fn a_save_naming_something_unknown_is_refused_and_changes_nothing() {
        let (mut app, _) = game_in_progress();
        let before = sorted(capture(app.world_mut()));
        let mut save = before.clone();
        save.clock = 0.0;
        save.jobs.push(JobSave {
            good: "rum".into(),
            quantity: 1,
            origin: "Atlantis".into(),
            destination: "Nassau".into(),
            payment: 1,
            deadline: 0.0,
            carrier: None,
            introduction: None,
        });
        assert_eq!(
            restore(app.world_mut(), &save),
            Err(LoadError::Unknown("port Atlantis".into()))
        );
        assert_eq!(sorted(capture(app.world_mut())), before);
    }

    #[test]
    fn a_save_from_another_version_is_refused() {
        let (mut app, _) = new_game();
        let mut save = capture(app.world_mut());
        save.version = SAVE_VERSION + 1;
        assert_eq!(
            SaveGame::from_ron(&save.to_ron()),
            Err(LoadError::Version(SAVE_VERSION + 1))
        );
    }
}
