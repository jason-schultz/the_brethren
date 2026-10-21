//! A run's headline numbers, and how they changed between runs: what the
//! comparison and trend pages show, and what the summary leads with.

use crate::fold::{Report, Series};

/// A headline number, by name.
#[derive(Debug, Clone, PartialEq)]
pub struct Measure {
    pub name: String,
    pub value: f64,
}

/// The last value of a series.
fn last(series: &Series) -> Option<f64> {
    series.last().map(|&(_, value)| value)
}

/// The greatest value of a series.
fn peak(series: &Series) -> Option<f64> {
    series.iter().map(|&(_, value)| value).reduce(f64::max)
}

/// A run's headline numbers, in the order they're shown. A number the run
/// has nothing for is left out.
pub fn headline(report: &Report) -> Vec<Measure> {
    let battles = &report.battles;
    let ended = |end: &str| f64::from(battles.ends.get(end).copied().unwrap_or(0));
    let mut measures: Vec<(String, Option<f64>)> = vec![
        ("Game days".into(), Some(report.days)),
        (
            "Real seconds a game day".into(),
            (report.days > 0.0).then(|| report.real_seconds / report.days),
        ),
        ("Alarms".into(), Some(report.alarms.len() as f64)),
        ("Warnings and errors".into(), Some(report.logs.len() as f64)),
        ("Battles".into(), Some(battles.list.len() as f64)),
        ("Ships sunk".into(), Some(ended("sunk"))),
        ("Ships taken".into(), Some(ended("taken"))),
        ("Ships struck and let go".into(), Some(ended("struck"))),
        ("Battles broken off".into(), Some(ended("broke off"))),
        (
            "Deeds".into(),
            Some(f64::from(report.word.deeds.values().sum::<u32>())),
        ),
        (
            "Days for word to reach a nation".into(),
            report.word.mean_days(),
        ),
        ("Purse at the end".into(), last(&report.timelines.purse)),
        (
            "Ships at the end".into(),
            Some(report.timelines.ships.values().filter_map(last).sum()),
        ),
        (
            "Open contracts at the end".into(),
            last(&report.timelines.open_jobs),
        ),
        ("Peak memory, MB".into(), peak(&report.timelines.memory_mb)),
        ("Peak entities".into(), peak(&report.timelines.entities)),
        (
            "Peak news in the ledger".into(),
            peak(&report.timelines.news),
        ),
    ];
    for (pair, series) in &report.timelines.relations {
        measures.push((format!("{pair} at the end"), last(series)));
    }
    measures
        .into_iter()
        .filter_map(|(name, value)| {
            Some(Measure {
                name,
                value: value?,
            })
        })
        .collect()
}

/// One headline number in two runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub name: String,
    pub before: Option<f64>,
    pub after: Option<f64>,
}

impl Change {
    /// Whether it's different enough to mention: by a hundredth or more of
    /// the larger, or there in only one run.
    pub fn is_notable(&self) -> bool {
        match (self.before, self.after) {
            (Some(before), Some(after)) => {
                (after - before).abs() > before.abs().max(after.abs()) * 0.01
            }
            _ => true,
        }
    }
}

/// Each headline number in `before` and `after`, in `after`'s order, then
/// any only `before` has.
pub fn compare(before: &[Measure], after: &[Measure]) -> Vec<Change> {
    let value = |measures: &[Measure], name: &str| {
        measures
            .iter()
            .find(|measure| measure.name == name)
            .map(|measure| measure.value)
    };
    let names = after
        .iter()
        .chain(
            before
                .iter()
                .filter(|old| value(after, &old.name).is_none()),
        )
        .map(|measure| measure.name.clone());
    names
        .map(|name| Change {
            before: value(before, &name),
            after: value(after, &name),
            name,
        })
        .collect()
}

/// A number for showing: whole if it is one, otherwise to two places.
pub fn shown(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

/// A change for showing: "+3", "−0.25", or "new".
pub fn shown_change(change: &Change) -> String {
    match (change.before, change.after) {
        (Some(before), Some(after)) => {
            let by = after - before;
            if by >= 0.0 {
                format!("+{}", shown(by))
            } else {
                format!("−{}", shown(-by))
            }
        }
        (None, Some(_)) => "new".to_string(),
        (_, None) => "gone".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fold::fold;
    use crate::fold::tests::{line, log};
    use chronicle::Record;

    fn measure(name: &str, value: f64) -> Measure {
        Measure {
            name: name.into(),
            value,
        }
    }

    #[test]
    fn a_run_without_a_player_has_no_purse_in_its_headline() {
        let report = fold(&log(vec![line(10.0, 5.0, Record::End)]));
        let headline = headline(&report);
        assert_eq!(headline[0], measure("Game days", 10.0));
        assert_eq!(headline[1], measure("Real seconds a game day", 0.5));
        assert!(!headline.iter().any(|m| m.name == "Purse at the end"));
    }

    #[test]
    fn changes_name_what_moved_and_what_came_and_went() {
        let before = [
            measure("Battles", 10.0),
            measure("Alarms", 2.0),
            measure("Old", 1.0),
        ];
        let after = [
            measure("Battles", 10.05),
            measure("Alarms", 5.0),
            measure("New", 1.0),
        ];
        let changes = compare(&before, &after);
        let names: Vec<&str> = changes.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Battles", "Alarms", "New", "Old"]);
        let notable: Vec<&str> = changes
            .iter()
            .filter(|c| c.is_notable())
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(notable, ["Alarms", "New", "Old"]);
        assert_eq!(shown_change(&changes[1]), "+3");
        assert_eq!(shown_change(&changes[2]), "new");
        assert_eq!(shown_change(&changes[3]), "gone");
    }

    #[test]
    fn numbers_are_shown_whole_when_they_are() {
        assert_eq!(shown(12.0), "12");
        assert_eq!(shown(0.256), "0.26");
        assert_eq!(
            shown_change(&Change {
                name: String::new(),
                before: Some(1.0),
                after: Some(0.5)
            }),
            "−0.50"
        );
    }
}
