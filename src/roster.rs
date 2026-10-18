//! Rosters: the named NPC ships a service keeps at sea, and replacing the
//! ones it loses.
//!
//! A ship sunk or taken is replaced some time later under her old name. A
//! name is only reused once no ship bears it, so names stay unique, which
//! saves rely on.

use bevy::prelude::*;

/// A service's roster of ship names, and when its next replacement is due.
#[derive(Debug, Clone, PartialEq)]
pub struct Roster {
    pub names: &'static [&'static str],
    /// How long after a loss her replacement is launched, seconds.
    pub replacement: f64,
    next_launch: Option<f64>,
}

impl Roster {
    pub const fn new(names: &'static [&'static str], replacement: f64) -> Self {
        Self {
            names,
            replacement,
            next_launch: None,
        }
    }

    /// The first name on the roster that no ship in `afloat` bears, with
    /// its place on the roster, if her replacement is due at `now`.
    /// Starts the clock on a newly missed ship.
    pub fn due<'a>(
        &mut self,
        afloat: impl Iterator<Item = &'a Name> + Clone,
        now: f64,
    ) -> Option<(usize, &'static str)> {
        let Some((index, &name)) = self
            .names
            .iter()
            .enumerate()
            .find(|(_, name)| !afloat.clone().any(|ship| ship.as_str() == **name))
        else {
            self.next_launch = None;
            return None;
        };
        if now < *self.next_launch.get_or_insert(now + self.replacement) {
            return None;
        }
        self.next_launch = None;
        Some((index, name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMES: &[&str] = &["Swallow", "Dove"];

    #[test]
    fn a_lost_ship_is_replaced_after_the_delay_under_her_name() {
        let mut roster = Roster::new(NAMES, 10.0);
        let swallow = [Name::new("Swallow")];
        assert_eq!(roster.due(swallow.iter(), 0.0), None, "the clock starts");
        assert_eq!(roster.due(swallow.iter(), 9.0), None);
        assert_eq!(roster.due(swallow.iter(), 10.0), Some((1, "Dove")));
        assert_eq!(roster.due(swallow.iter(), 10.0), None, "the clock restarts");
    }

    #[test]
    fn a_full_roster_needs_nothing() {
        let mut roster = Roster::new(NAMES, 10.0);
        let all = [Name::new("Swallow"), Name::new("Dove")];
        assert_eq!(roster.due(all.iter(), 100.0), None);
    }
}
