//! The knobs the game plays by, from `assets/data/tuning.ron`, built in.
//!
//! Each is a value someone tuning the game may want to change, with hints
//! on what it does in the file itself (see the `tuning` crate). Editing
//! the file, by hand or from the logbook's Tuning page, takes effect on
//! the next build: `cargo soak` and `cargo run` rebuild for it.

use std::sync::LazyLock;

use tuning::{Knob, parse};

use crate::clock::HOUR;

const KNOBS: &str = include_str!("../assets/data/tuning.ron");

/// The knobs built into the game. A static rather than a resource, like
/// the ship classes: they're fixed for a build.
pub static TUNING: LazyLock<Tuning> =
    LazyLock::new(|| Tuning::parse(KNOBS).expect("the embedded knobs are validated by tests"));

/// The knobs, in the units the game works in.
#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
    /// How far a lookout sees, meters.
    pub sighting_range: f32,
    pub timbers_per_hit: f32,
    pub rigging_per_hit: f32,
    pub men_per_hit: f64,
    pub strike_timbers: f32,
    pub strike_rigging: f32,
    pub strike_crew: f32,
    /// How long a steady crew keeps up a chase, seconds.
    pub stern_chase: f64,
    /// The shortest and longest chase, seconds.
    pub patience: [f64; 2],
    /// How long a lost quarry is remembered, seconds.
    pub lost_sight: f64,
    pub chase_morale_per_day: f32,
    pub seep_per_hour: f32,
    pub leak_per_hour: f32,
    pub pumps_per_hour: f32,
}

impl Tuning {
    /// The knobs in `source`, every one the game needs.
    pub fn parse(source: &str) -> Result<Self, String> {
        let knobs: Vec<Knob> = parse(source)?;
        let knob = |id: &str| {
            knobs
                .iter()
                .find(|knob| knob.id == id)
                .map(|knob| knob.value)
                .ok_or_else(|| format!("the knob {id} is missing"))
        };
        let hours = |id: &str| knob(id).map(|value| value * HOUR);
        Ok(Self {
            sighting_range: (knob("sight.range_km")? * 1_000.0) as f32,
            timbers_per_hit: knob("gunnery.timbers_per_hit")? as f32,
            rigging_per_hit: knob("gunnery.rigging_per_hit")? as f32,
            men_per_hit: knob("gunnery.men_per_hit")?,
            strike_timbers: knob("striking.timbers")? as f32,
            strike_rigging: knob("striking.rigging")? as f32,
            strike_crew: knob("striking.crew")? as f32,
            stern_chase: hours("chase.stern_chase_hours")?,
            patience: [
                hours("chase.patience_min_hours")?,
                hours("chase.patience_max_hours")?,
            ],
            lost_sight: hours("chase.lost_sight_hours")?,
            chase_morale_per_day: knob("chase.morale_per_day")? as f32,
            seep_per_hour: knob("leaks.seep_per_hour")? as f32,
            leak_per_hour: knob("leaks.leak_per_hour")? as f32,
            pumps_per_hour: knob("leaks.pumps_per_hour")? as f32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_knobs_parse_and_make_sense() {
        let tuning = Tuning::parse(KNOBS).unwrap();
        assert_eq!(tuning.sighting_range, 8_000.0);
        assert!(
            tuning.patience[0] <= tuning.stern_chase && tuning.stern_chase <= tuning.patience[1]
        );
    }

    #[test]
    fn a_missing_knob_is_named() {
        let knobs: Vec<Knob> = parse(KNOBS).unwrap();
        let without = tuning::write(&knobs[1..]).unwrap();
        assert!(
            Tuning::parse(&without)
                .unwrap_err()
                .contains("sight.range_km")
        );
    }
}
