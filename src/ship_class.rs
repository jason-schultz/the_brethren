//! Ship classes: everything that makes a ship of a given type, in one place.
//! These will move to data assets once there is more than one class.

use bevy::prelude::*;

use crate::cargo::Hold;
use crate::combat::Battery;
use crate::crew::{Complement, Crew};
use crate::officers::Afterguard;
use crate::sailing::Hull;

/// Also a component: every ship carries her class.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ShipClass {
    pub name: &'static str,
    pub hull: Hull,
    pub complement: Complement,
    /// Cargo space in tons burthen.
    pub hold_tons: f64,
    /// Guns she's pierced for, both sides together.
    pub guns: u32,
    /// What a new, sound one costs, in pieces of eight.
    pub price: u32,
    /// Length on deck, meters: how big she's drawn.
    pub length_m: f32,
}

impl ShipClass {
    /// A trading sloop of about 60 tons. Merchants sail her with a dozen
    /// men; pirates cram in six or seven times that to board their prizes.
    pub const SLOOP: Self = Self {
        name: "Sloop",
        hull: Hull::SLOOP,
        complement: Complement {
            min: 8,
            full: 40,
            berths: 75,
        },
        hold_tons: 40.0,
        guns: 8,
        price: 2_000,
        length_m: 20.0,
    };

    /// A ship's boat: what a captain who has lost his ship is left with.
    /// No guns, a few tons of stores, and room for a boarding party.
    pub const LONGBOAT: Self = Self {
        name: "Longboat",
        hull: Hull::LONGBOAT,
        complement: Complement {
            min: 2,
            full: 8,
            berths: 24,
        },
        hold_tons: 4.0,
        guns: 0,
        price: 150,
        length_m: 8.0,
    };

    /// Every ship class in the game.
    pub const ALL: [Self; 2] = [Self::SLOOP, Self::LONGBOAT];

    /// The class called `name`.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.name == name)
    }

    /// This class's hold, empty.
    pub fn empty_hold(&self) -> Hold {
        Hold::new(self.hold_tons)
    }

    /// A new ship of this class with `crew` men aboard, `guns` mounted (at
    /// most [`ShipClass::guns`]), carrying `hold`.
    pub fn build(self, crew: u32, guns: u32, hold: Hold) -> impl Bundle {
        (
            self,
            self.hull,
            self.complement,
            Crew::new(crew),
            Battery::new(guns.min(self.guns)),
            hold,
            Afterguard::default(),
        )
    }
}
