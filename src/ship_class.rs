//! Ship classes: everything that makes a ship of a given type, in one place,
//! defined as data (`assets/data/ships.ron`).

use std::sync::LazyLock;

use bevy::prelude::*;
use serde::Deserialize;

use crate::cargo::Hold;
use crate::combat::{Battery, Timbers};
use crate::crew::{Complement, Crew};
use crate::officers::Afterguard;
use crate::sailing::{Hull, Rigging};

const SHIPS: &str = include_str!("../assets/data/ships.ron");

/// The class a new captain starts in.
const FIRST: &str = "Sloop";
/// The class a captain who has lost her ship is put adrift in.
const BOAT: &str = "Longboat";

/// The classes built into the game. A static rather than a resource, like
/// the factions: classes are plain `Copy` values carried by every ship,
/// borrowing their names from the data, which never changes once the game
/// is running.
static CLASSES: LazyLock<Vec<ShipClass>> =
    LazyLock::new(|| ShipClass::parse(SHIPS).expect("the embedded ships are validated by tests"));

/// How she's rigged: what she looks like, and what her card calls her.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rig {
    /// A ship's boat under a lugsail.
    Lugger,
    /// One mast, gaff-rigged fore-and-aft.
    Sloop,
    /// Two masts, both fore-and-aft.
    Schooner,
    /// Two masts: square on the fore, fore-and-aft on the main.
    Brigantine,
    /// Two masts, both square.
    Brig,
    /// Three masts, all square.
    Ship,
}

/// Also a component: every ship carries her class.
#[derive(Component, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct ShipClass {
    /// What she's called, and how saves and the factions' fleets name her.
    pub name: &'static str,
    pub rig: Rig,
    pub hull: Hull,
    pub complement: Complement,
    /// Cargo space in tons burthen.
    pub hold_tons: f64,
    /// Guns she's pierced for, both sides together.
    pub guns: u32,
    /// How much shot she takes, as a multiple of a sloop: each hit does a
    /// sloop's damage to her timbers and rigging divided by this.
    pub stoutness: f32,
    /// What a new, sound one costs, in pieces of eight.
    pub price: u32,
    /// Length on deck, meters: how big she's drawn.
    pub length_m: f32,
}

#[derive(Debug, PartialEq)]
pub enum ShipsError {
    Parse(String),
    Duplicate(&'static str),
    Missing(&'static str),
    /// Fewer berths than hands to fight her, or fewer of those than she
    /// needs to sail; or no stoutness at all.
    Unworkable(&'static str),
}

impl ShipClass {
    /// Every class in `source`, checked: names unique, a class for a new
    /// captain and one for a boat, and every class workable.
    pub fn parse(source: &'static str) -> Result<Vec<Self>, ShipsError> {
        let classes: Vec<Self> =
            ron::from_str(source).map_err(|e| ShipsError::Parse(e.to_string()))?;
        for (i, class) in classes.iter().enumerate() {
            if classes[..i].iter().any(|other| other.name == class.name) {
                return Err(ShipsError::Duplicate(class.name));
            }
            let Complement { min, full, berths } = class.complement;
            if !(min <= full && full <= berths && class.stoutness > 0.0) {
                return Err(ShipsError::Unworkable(class.name));
            }
        }
        for name in [FIRST, BOAT] {
            if !classes.iter().any(|class| class.name == name) {
                return Err(ShipsError::Missing(name));
            }
        }
        Ok(classes)
    }

    /// Every ship class in the game, in the data's order.
    pub fn all() -> &'static [Self] {
        &CLASSES
    }

    /// The class called `name`.
    pub fn named(name: &str) -> Option<Self> {
        Self::all().iter().copied().find(|class| class.name == name)
    }

    /// The sloop a new captain starts in.
    pub fn sloop() -> Self {
        Self::named(FIRST).expect("parsing checks for the first class")
    }

    /// A ship's boat: what a captain who has lost her ship is left with.
    pub fn boat() -> Self {
        Self::named(BOAT).expect("parsing checks for the boat")
    }

    /// The classes a shipyard sells: all but the boat.
    pub fn for_sale() -> impl Iterator<Item = Self> {
        Self::all()
            .iter()
            .copied()
            .filter(|class| class.name != BOAT)
    }

    /// This class's hold, empty.
    pub fn empty_hold(&self) -> Hold {
        Hold::new(self.hold_tons)
    }

    /// What makes a ship of this class.
    fn hull_parts(self) -> impl Bundle {
        (self, self.hull, self.complement)
    }

    /// Everything a new ship of this class brings from the yard: sound, and
    /// with every gun she's pierced for. Inserted over an old ship, it
    /// changes her class and leaves her crew, hold, and afterguard as they
    /// were.
    pub fn new_hull(self) -> impl Bundle {
        (
            self.hull_parts(),
            Battery::new(self.guns),
            Timbers::default(),
            Rigging::default(),
        )
    }

    /// A new ship of this class with `crew` men aboard, `guns` mounted (at
    /// most [`ShipClass::guns`]), carrying `hold`.
    pub fn build(self, crew: u32, guns: u32, hold: Hold) -> impl Bundle {
        (
            self.hull_parts(),
            Crew::new(crew),
            Battery::new(guns.min(self.guns)),
            hold,
            Afterguard::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_ships_parse() {
        let classes = ShipClass::parse(SHIPS).unwrap();
        assert!(classes.len() > 2);
        assert_eq!(ShipClass::sloop().name, "Sloop");
        assert_eq!(ShipClass::boat().guns, 0);
        assert!(ShipClass::for_sale().all(|class| class != ShipClass::boat()));
    }

    #[test]
    fn ships_that_dont_add_up_are_refused() {
        let duplicate = r#"[
            (name: "Sloop", rig: Sloop, hull: (speed_mps: 5.5, headwind_factor: 0.75, tailwind_factor: 1.2, turn_degrees: 12.0, acceleration: 0.5), complement: (min: 8, full: 40, berths: 75), hold_tons: 40.0, guns: 8, stoutness: 1.0, price: 2000, length_m: 20.0),
            (name: "Sloop", rig: Sloop, hull: (speed_mps: 5.5, headwind_factor: 0.75, tailwind_factor: 1.2, turn_degrees: 12.0, acceleration: 0.5), complement: (min: 8, full: 40, berths: 75), hold_tons: 40.0, guns: 8, stoutness: 1.0, price: 2000, length_m: 20.0),
        ]"#;
        assert_eq!(
            ShipClass::parse(duplicate).unwrap_err(),
            ShipsError::Duplicate("Sloop")
        );
        let crowded = r#"[
            (name: "Sloop", rig: Sloop, hull: (speed_mps: 5.5, headwind_factor: 0.75, tailwind_factor: 1.2, turn_degrees: 12.0, acceleration: 0.5), complement: (min: 8, full: 40, berths: 30), hold_tons: 40.0, guns: 8, stoutness: 1.0, price: 2000, length_m: 20.0),
        ]"#;
        assert_eq!(
            ShipClass::parse(crowded).unwrap_err(),
            ShipsError::Unworkable("Sloop")
        );
        let no_boat = r#"[
            (name: "Sloop", rig: Sloop, hull: (speed_mps: 5.5, headwind_factor: 0.75, tailwind_factor: 1.2, turn_degrees: 12.0, acceleration: 0.5), complement: (min: 8, full: 40, berths: 75), hold_tons: 40.0, guns: 8, stoutness: 1.0, price: 2000, length_m: 20.0),
        ]"#;
        assert_eq!(
            ShipClass::parse(no_boat).unwrap_err(),
            ShipsError::Missing(BOAT)
        );
    }

    #[test]
    fn turn_rates_are_read_in_degrees() {
        let sloop = ShipClass::sloop();
        assert!((sloop.hull.max_turn_rate - 12.0_f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn every_class_is_a_trade_off() {
        let fastest = ShipClass::for_sale()
            .max_by(|a, b| a.hull.speed_mps.total_cmp(&b.hull.speed_mps))
            .unwrap();
        let roomiest = ShipClass::for_sale()
            .max_by(|a, b| a.hold_tons.total_cmp(&b.hold_tons))
            .unwrap();
        let heaviest = ShipClass::for_sale()
            .max_by_key(|class| class.guns)
            .unwrap();
        assert_ne!(fastest, roomiest, "the fastest isn't the best carrier");
        assert_ne!(fastest, heaviest, "nor the most heavily armed");
        assert_ne!(roomiest, heaviest);
    }
}
