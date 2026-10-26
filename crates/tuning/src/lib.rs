//! False Colours' knobs and balance targets, and what each one is for.
//!
//! *Knobs* change how the game plays: how fast a holed ship fills, how far
//! a lookout sees. The game reads their values at build time. *Targets*
//! say what a good soak looks like: how often ships founder, how many
//! fights end in a prize. They change nothing in the game; the logbook
//! measures each soak against them. Every entry carries hints for whoever
//! tunes it: what it does, how it changes play, and the history behind it.
//!
//! Both live as RON in the game's `assets/data` folder, and the logbook's
//! Tuning page edits them. See `docs/BALANCE.md`.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The knobs' file, in `assets/data`.
pub const KNOBS_FILE: &str = "tuning.ron";
/// The targets' file, in `assets/data`.
pub const TARGETS_FILE: &str = "balance.ron";

/// What an entry is for, for whoever tunes it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Hints {
    /// What it does, plainly.
    pub does: String,
    /// How raising or lowering it changes play.
    pub play: String,
    /// The history behind it, and where the game departs from it.
    pub history: String,
}

/// A value the game plays by.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Knob {
    /// How the game finds it: "leaks.pumps_per_hour".
    pub id: String,
    /// The heading it's listed under.
    pub group: String,
    pub name: String,
    /// What it's measured in: "km", "timbers an hour".
    pub unit: String,
    pub value: f64,
    /// The range it makes sense in.
    pub min: f64,
    pub max: f64,
    /// The value history would give, where the game departs from it.
    pub historical: Option<f64>,
    pub hints: Hints,
}

/// What a good soak looks like, for one measure of it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Target {
    /// The measure it's of, as the logbook names it: "foundered_per_100_days".
    pub id: String,
    pub group: String,
    pub name: String,
    pub unit: String,
    /// The least and most a good soak has, either open.
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub hints: Hints,
}

impl Target {
    /// Whether a soak measuring `value` is on target.
    pub fn met_by(&self, value: f64) -> bool {
        self.min.is_none_or(|min| value >= min) && self.max.is_none_or(|max| value <= max)
    }

    /// The range, in words: "1 to 5", "at least 2", "at most 0.4", "any".
    pub fn range(&self) -> String {
        match (self.min, self.max) {
            (Some(min), Some(max)) => format!("{min} to {max}"),
            (Some(min), None) => format!("at least {min}"),
            (None, Some(max)) => format!("at most {max}"),
            (None, None) => "any".to_string(),
        }
    }
}

/// Something that can be wrong with a tuning file.
pub trait Entry {
    fn id(&self) -> &str;
    /// Why its values make no sense, if they don't.
    fn fault(&self) -> Option<String>;
}

impl Entry for Knob {
    fn id(&self) -> &str {
        &self.id
    }

    fn fault(&self) -> Option<String> {
        (!(self.min <= self.value && self.value <= self.max)).then(|| {
            format!(
                "{}: {} is outside {} to {}",
                self.id, self.value, self.min, self.max
            )
        })
    }
}

impl Entry for Target {
    fn id(&self) -> &str {
        &self.id
    }

    fn fault(&self) -> Option<String> {
        match (self.min, self.max) {
            (Some(min), Some(max)) if min > max => Some(format!(
                "{}: the least, {min}, is over the most, {max}",
                self.id
            )),
            _ => None,
        }
    }
}

/// The entries in a tuning file, each with a unique id and values that
/// make sense.
pub fn parse<T: DeserializeOwned + Entry>(source: &str) -> Result<Vec<T>, String> {
    let entries: Vec<T> = ron::from_str(source).map_err(|error| error.to_string())?;
    check(&entries)?;
    Ok(entries)
}

/// Why `entries` won't do, if they won't.
pub fn check<T: Entry>(entries: &[T]) -> Result<(), String> {
    for (index, entry) in entries.iter().enumerate() {
        if entries[..index]
            .iter()
            .any(|other| other.id() == entry.id())
        {
            return Err(format!("{} is listed twice", entry.id()));
        }
        if let Some(fault) = entry.fault() {
            return Err(fault);
        }
    }
    Ok(())
}

/// A tuning file's text, for `entries`.
pub fn write<T: Serialize>(entries: &[T]) -> Result<String, String> {
    let config = ron::ser::PrettyConfig::new().struct_names(false);
    let body = ron::ser::to_string_pretty(entries, config).map_err(|error| error.to_string())?;
    Ok(format!(
        "// Edited by hand or from the logbook's Tuning page (`cargo logbook`).\n\
         // See docs/BALANCE.md.\n{body}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hints() -> Hints {
        Hints {
            does: "does".into(),
            play: "play".into(),
            history: "history".into(),
        }
    }

    fn knob(id: &str, value: f64) -> Knob {
        Knob {
            id: id.into(),
            group: "Leaks".into(),
            name: "Seepage".into(),
            unit: "timbers an hour".into(),
            value,
            min: 0.0,
            max: 1.0,
            historical: None,
            hints: hints(),
        }
    }

    #[test]
    fn a_file_written_reads_back_the_same() {
        let knobs = vec![knob("leaks.seep", 0.01), knob("leaks.pumps", 0.09)];
        let text = write(&knobs).unwrap();
        assert_eq!(parse::<Knob>(&text).unwrap(), knobs);
    }

    #[test]
    fn a_value_out_of_its_range_or_an_id_twice_is_refused() {
        assert!(check(&[knob("a", 2.0)]).unwrap_err().contains("outside"));
        assert!(
            check(&[knob("a", 0.5), knob("a", 0.5)])
                .unwrap_err()
                .contains("twice")
        );
    }

    #[test]
    fn a_target_is_met_inside_its_range_and_either_end_may_be_open() {
        let mut target = Target {
            id: "foundered_per_100_days".into(),
            group: "Losses".into(),
            name: "Ships foundered".into(),
            unit: "per 100 days".into(),
            min: Some(1.0),
            max: Some(5.0),
            hints: hints(),
        };
        assert!(target.met_by(1.0) && target.met_by(5.0) && !target.met_by(6.0));
        assert_eq!(target.range(), "1 to 5");
        target.max = None;
        assert!(target.met_by(60.0) && !target.met_by(0.5));
        assert_eq!(target.range(), "at least 1");
        target.max = Some(0.5);
        assert!(check(&[target]).unwrap_err().contains("over the most"));
    }
}
