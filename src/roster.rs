//! Rosters: the named NPC ships a service keeps at sea, and replacing the
//! ones it loses.
//!
//! A ship sunk or taken is replaced some time later under her old name. A
//! name is only reused once no ship bears it, so names stay unique, which
//! saves rely on. The ships come from each nation's fleet in the factions
//! data.

use bevy::prelude::*;

use crate::factions::{Nation, nations};
use crate::ship_class::ShipClass;

/// A ship on a roster: her name, the nation she sails for, and her class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Listed {
    pub name: &'static str,
    pub nation: Nation,
    pub class: ShipClass,
}

/// A service's roster of ships, and when its next replacement is due.
#[derive(Debug, Clone, PartialEq)]
pub struct Roster {
    pub ships: Vec<Listed>,
    /// How long after a loss her replacement is launched, seconds.
    pub replacement: f64,
    next_launch: Option<f64>,
}

impl Roster {
    pub fn new(ships: Vec<Listed>, replacement: f64) -> Self {
        Self {
            ships,
            replacement,
            next_launch: None,
        }
    }

    /// The ships `fleet` picks from every nation's, in the data's order.
    pub fn of_fleets(
        fleet: impl Fn(Nation) -> &'static [(String, String)],
        replacement: f64,
    ) -> Self {
        let ships = nations()
            .flat_map(|nation| {
                fleet(nation).iter().map(move |(name, class)| Listed {
                    name: name.as_str(),
                    nation,
                    class: ShipClass::named(class)
                        .expect("the factions data is checked for known classes"),
                })
            })
            .collect();
        Self::new(ships, replacement)
    }

    /// The first ship on the roster that no ship in `afloat` is named for,
    /// with her place on the roster, if her replacement is due at `now`.
    /// Starts the clock on a newly missed ship.
    pub fn due<'a>(
        &mut self,
        afloat: impl Iterator<Item = &'a Name> + Clone,
        now: f64,
    ) -> Option<(usize, Listed)> {
        let Some((index, &listed)) = self
            .ships
            .iter()
            .enumerate()
            .find(|(_, listed)| !afloat.clone().any(|ship| ship.as_str() == listed.name))
        else {
            self.next_launch = None;
            return None;
        };
        if now < *self.next_launch.get_or_insert(now + self.replacement) {
            return None;
        }
        self.next_launch = None;
        Some((index, listed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(name: &'static str) -> Listed {
        Listed {
            name,
            nation: Nation::ENGLAND,
            class: ShipClass::sloop(),
        }
    }

    fn roster() -> Roster {
        Roster::new(vec![listed("Swallow"), listed("Dove")], 10.0)
    }

    #[test]
    fn a_lost_ship_is_replaced_after_the_delay_under_her_name() {
        let mut roster = roster();
        let swallow = [Name::new("Swallow")];
        assert_eq!(roster.due(swallow.iter(), 0.0), None, "the clock starts");
        assert_eq!(roster.due(swallow.iter(), 9.0), None);
        assert_eq!(roster.due(swallow.iter(), 10.0), Some((1, listed("Dove"))));
        assert_eq!(roster.due(swallow.iter(), 10.0), None, "the clock restarts");
    }

    #[test]
    fn a_full_roster_needs_nothing() {
        let mut roster = roster();
        let all = [Name::new("Swallow"), Name::new("Dove")];
        assert_eq!(roster.due(all.iter(), 100.0), None);
    }

    #[test]
    fn fleets_come_from_the_factions_data() {
        let navies = Roster::of_fleets(|nation| &nation.fleet().navy, 0.0);
        let sails_for = |name| {
            navies
                .ships
                .iter()
                .find(|listed| listed.name == name)
                .map(|listed| listed.nation)
        };
        assert_eq!(sails_for("HMS Swift"), Some(Nation::ENGLAND));
        assert_eq!(sails_for("Ranger"), Some(Nation::PIRATES));
        let classes: Vec<&str> = navies
            .ships
            .iter()
            .map(|listed| listed.class.name)
            .collect();
        assert!(
            classes.iter().any(|&class| class != "Sloop"),
            "not every navy sails sloops: {classes:?}"
        );
    }
}
