//! A run against the balance targets: each target's measure, taken from
//! the run's report, and whether it's on target. See `docs/BALANCE.md`.

use std::collections::BTreeMap;

use tuning::Target;

use crate::fold::Report;

/// A target, and how a run measured up to it.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict<'a> {
    pub target: &'a Target,
    /// `None` if the run can't say: too short, or no fights to judge.
    pub value: Option<f64>,
}

impl Verdict<'_> {
    /// Whether the run was off target, if it could say.
    pub fn off(&self) -> bool {
        self.value.is_some_and(|value| !self.target.met_by(value))
    }
}

/// How `report` measures up to each of `targets`.
pub fn verdicts<'a>(targets: &'a [Target], report: &Report) -> Vec<Verdict<'a>> {
    targets
        .iter()
        .map(|target| Verdict {
            target,
            value: measure(&target.id, report),
        })
        .collect()
}

/// The measure named `id` for `report`, if it knows the measure and the
/// run is long enough to tell.
pub fn measure(id: &str, report: &Report) -> Option<f64> {
    let per_100_days =
        |count: u32| (report.days >= 1.0).then(|| f64::from(count) * 100.0 / report.days);
    let ends = |end: &str| report.battles.ends.get(end).copied().unwrap_or(0);
    let lost = |causes: &[&str]| -> u32 {
        causes
            .iter()
            .map(|cause| report.battles.lost.get(*cause).copied().unwrap_or(0))
            .sum()
    };
    match id {
        "prizes_per_100_days" => per_100_days(ends("taken")),
        "broke_off_share" => {
            let fought: u32 = report.battles.ends.values().sum();
            (fought > 0).then(|| f64::from(ends("broke off")) / f64::from(fought))
        }
        "gone_down_per_100_days" => per_100_days(lost(&["sunk", "foundered", "reef", "grounding"])),
        "foundered_per_100_days" => per_100_days(lost(&["foundered"])),
        "most_fights_one_pair" => {
            let mut pairs: BTreeMap<[&str; 2], u32> = BTreeMap::new();
            for battle in &report.battles.list {
                let mut pair = battle.sides.each_ref().map(|side| side.ship.as_str());
                pair.sort();
                *pairs.entry(pair).or_default() += 1;
            }
            Some(f64::from(pairs.values().copied().max().unwrap_or(0)))
        }
        "ports_changed_per_100_days" => per_100_days(report.went_over),
        _ => None,
    }
}

/// A measure's value as it's shown: shares as percents.
pub fn shown(id: &str, value: f64) -> String {
    if id.ends_with("_share") {
        format!("{:.0}%", value * 100.0)
    } else {
        format!("{value:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fold::fold;
    use crate::fold::tests::log;
    use tuning::Hints;

    fn report(days: f64) -> Report {
        Report {
            days,
            ..fold(&log(Vec::new()))
        }
    }

    fn target(id: &str, min: Option<f64>, max: Option<f64>) -> Target {
        Target {
            id: id.into(),
            group: "Fights".into(),
            name: id.into(),
            unit: String::new(),
            min,
            max,
            hints: Hints {
                does: String::new(),
                play: String::new(),
                history: String::new(),
            },
        }
    }

    #[test]
    fn every_target_in_the_games_file_has_a_measure() {
        let source = include_str!("../../../assets/data/balance.ron");
        let targets: Vec<Target> = tuning::parse(source).unwrap();
        let mut report = report(100.0);
        report.battles.ends.insert("taken".into(), 1);
        for target in &targets {
            assert!(
                measure(&target.id, &report).is_some(),
                "nothing measures {}",
                target.id
            );
        }
    }

    #[test]
    fn a_run_is_judged_by_its_rates_and_shares() {
        let mut report = report(50.0);
        let battles = &mut report.battles;
        battles.ends.insert("taken".into(), 5);
        battles.ends.insert("broke off".into(), 15);
        battles.lost.insert("foundered".into(), 1);
        assert_eq!(measure("prizes_per_100_days", &report), Some(10.0));
        assert_eq!(measure("broke_off_share", &report), Some(0.75));
        assert_eq!(measure("gone_down_per_100_days", &report), Some(2.0));
        let targets = [
            target("broke_off_share", None, Some(0.5)),
            target("prizes_per_100_days", Some(5.0), None),
            target("no_such_measure", Some(1.0), None),
        ];
        let off: Vec<bool> = verdicts(&targets, &report)
            .iter()
            .map(Verdict::off)
            .collect();
        assert_eq!(off, [true, false, false], "an unknown measure can't be off");
        assert_eq!(shown("broke_off_share", 0.75), "75%");
    }
}
