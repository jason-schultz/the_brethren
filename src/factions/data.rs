//! The nations, their groups, and what deeds are worth, as data
//! (`assets/data/factions.ron`).

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use super::DeedKind;
use crate::service::RANKS;
use crate::ship_class::ShipClass;

const FACTIONS: &str = include_str!("../../assets/data/factions.ron");

/// The factions built into the game. A static rather than a resource:
/// nations and groups name themselves in nearly every message the game
/// writes, and only a static lets their `Display` do it. The data never
/// changes once the game is running.
static DATA: LazyLock<Factions> = LazyLock::new(|| {
    Factions::parse(FACTIONS).expect("the embedded factions are validated by tests")
});

/// The factions built into the game.
pub fn factions() -> &'static Factions {
    &DATA
}

/// Every nation, how they stand with each other at the start, and what
/// deeds are worth.
#[derive(Deserialize, Debug)]
pub struct Factions {
    pub nations: Vec<NationData>,
    /// How pairs of nations stand at the start, by key.
    relations: Vec<(String, String, f32)>,
    pub reputation: Reputation,
}

/// One nation, as the data has it.
#[derive(Deserialize, Debug)]
pub struct NationData {
    /// How maps and saves name it.
    pub key: String,
    pub name: String,
    /// Its colours in a word: "English".
    pub adjective: String,
    /// What it's shown in, as sRGB.
    pub colour: (f32, f32, f32),
    /// Whether it has a king's law: pardons, and a navy that patrols
    /// rather than raids.
    pub lawful: bool,
    /// The day of its first Act of Grace, if it proclaims them.
    pub grace: Option<f64>,
    /// The player's standing with each of its groups at the start.
    pub standing: f32,
    /// The chances an officer hired in its taverns is loyal, ambitious, or
    /// a turncoat.
    pub officers: (f32, f32, f32),
    pub navy: GroupData,
    pub governor: GroupData,
    pub merchants: GroupData,
    pub fleet: Fleet,
}

/// One of a nation's groups, as the data has it.
#[derive(Deserialize, Debug)]
pub struct GroupData {
    pub name: String,
    /// The title of each rank in it, lowest first: one for each of the
    /// service's ranks.
    pub ranks: Vec<String>,
    /// Why it says a new captain should join it.
    pub pitch: String,
}

/// A nation's ships, each by name and class.
#[derive(Deserialize, Debug)]
pub struct Fleet {
    pub merchants: Vec<(String, String)>,
    /// Patrols for a lawful nation, raiders for one that isn't.
    pub navy: Vec<(String, String)>,
}

/// How deeds move standing and relations.
#[derive(Deserialize, Debug)]
pub struct Reputation {
    /// The rest of a nation takes this share of what a deed does to one of
    /// its groups.
    pub kin_share: f32,
    /// A nation's navy takes this share of a blow struck against its
    /// enemies, as credit, or against its allies, as offence.
    pub neighbour_share: f32,
    pub deeds: HashMap<DeedKind, Worth>,
}

/// What a deed is worth.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Worth {
    /// Standing with the group it touched.
    pub standing: f32,
    /// Relations between the doer's colours and the nation it touched.
    pub diplomacy: f32,
    /// Whether it's a blow struck against the nation, which its enemies
    /// applaud and its allies resent.
    pub blow: bool,
}

/// Why the factions data won't do.
#[derive(Debug, PartialEq)]
pub enum FactionsError {
    Parse(String),
    /// Two nations share a key.
    DuplicateKey(String),
    /// Relations name a nation there isn't.
    UnknownNation(String),
    /// A deed has no worth given.
    Unpriced(DeedKind),
    /// Too many nations to tell apart.
    TooMany,
    /// A group hasn't a title for every rank.
    Ranks(String),
    /// A fleet's ship is of a class the ships data doesn't have.
    UnknownClass(String),
}

impl Factions {
    pub fn parse(source: &str) -> Result<Self, FactionsError> {
        let factions: Self =
            ron::from_str(source).map_err(|e| FactionsError::Parse(e.to_string()))?;
        if factions.nations.len() > u8::MAX as usize {
            return Err(FactionsError::TooMany);
        }
        for (i, nation) in factions.nations.iter().enumerate() {
            if factions.nations[..i]
                .iter()
                .any(|other| other.key == nation.key)
            {
                return Err(FactionsError::DuplicateKey(nation.key.clone()));
            }
        }
        for nation in &factions.nations {
            for group in [&nation.navy, &nation.governor, &nation.merchants] {
                if group.ranks.len() != RANKS.len() {
                    return Err(FactionsError::Ranks(group.name.clone()));
                }
            }
        }
        for nation in &factions.nations {
            let mut fleet = nation.fleet.merchants.iter().chain(&nation.fleet.navy);
            if let Some((_, class)) = fleet.find(|(_, class)| ShipClass::named(class).is_none()) {
                return Err(FactionsError::UnknownClass(class.clone()));
            }
        }
        for (a, b, _) in &factions.relations {
            for key in [a, b] {
                factions
                    .index(key)
                    .ok_or_else(|| FactionsError::UnknownNation(key.clone()))?;
            }
        }
        if let Some(kind) = DeedKind::ALL
            .into_iter()
            .find(|kind| !factions.reputation.deeds.contains_key(kind))
        {
            return Err(FactionsError::Unpriced(kind));
        }
        Ok(factions)
    }

    /// The place of the nation keyed `key`.
    pub fn index(&self, key: &str) -> Option<usize> {
        self.nations.iter().position(|nation| nation.key == key)
    }

    /// How the nations at places `a` and `b` stand at the start.
    pub fn starting_relations(&self, a: usize, b: usize) -> f32 {
        self.relations
            .iter()
            .find(|(x, y, _)| {
                let (x, y) = (self.index(x), self.index(y));
                (x, y) == (Some(a), Some(b)) || (x, y) == (Some(b), Some(a))
            })
            .map_or(0.0, |&(_, _, value)| value)
    }

    /// What a deed of `kind` is worth.
    pub fn worth(&self, kind: DeedKind) -> Worth {
        self.reputation.deeds[&kind]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_factions_parse() {
        let factions = Factions::parse(FACTIONS).unwrap();
        assert_eq!(factions.nations.len(), 3);
        assert_eq!(factions.starting_relations(0, 1), -20.0);
        assert_eq!(factions.starting_relations(2, 1), -60.0, "either way round");
    }

    #[test]
    fn factions_that_dont_add_up_are_refused() {
        let duplicate = FACTIONS.replace("key: \"spain\"", "key: \"england\"");
        assert_eq!(
            Factions::parse(&duplicate).unwrap_err(),
            FactionsError::DuplicateKey("england".into())
        );
        let stranger = FACTIONS.replace("(\"spain\", \"pirates\"", "(\"spain\", \"france\"");
        assert_eq!(
            Factions::parse(&stranger).unwrap_err(),
            FactionsError::UnknownNation("france".into())
        );
        let unpriced = FACTIONS.replace(
            "Escorted: (standing: 5.0, diplomacy: 1.0, blow: false),",
            "",
        );
        assert_eq!(
            Factions::parse(&unpriced).unwrap_err(),
            FactionsError::Unpriced(DeedKind::Escorted)
        );
        let untitled = FACTIONS.replace("\"Captain of Note\", ", "");
        assert_eq!(
            Factions::parse(&untitled).unwrap_err(),
            FactionsError::Ranks("the Flying Gang".into())
        );
        let unbuilt = FACTIONS.replace("(\"Dove\", \"Schooner\")", "(\"Dove\", \"Galleon\")");
        assert_eq!(
            Factions::parse(&unbuilt).unwrap_err(),
            FactionsError::UnknownClass("Galleon".into())
        );
    }
}
