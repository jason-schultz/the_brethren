//! A run's summary as plain text, short enough to paste into a
//! conversation: what's broken and what changed first, then the numbers.

use std::fmt::Write as _;

use chronicle::{Run, Stamp};

use crate::fold::{Matchup, Report, Series, Supply};
use crate::headline::{compare, headline, shown, shown_change};

/// The most lines any one list runs to.
const LIST: usize = 20;
/// The most price moves shown.
const PRICE_MOVES: usize = 10;

/// Writes `title` and up to [`LIST`] of `items`, if there are any.
fn list(out: &mut String, title: &str, items: impl ExactSizeIterator<Item = String>) {
    let total = items.len();
    if total == 0 {
        return;
    }
    let _ = writeln!(out, "\n{title} ({total}):");
    for item in items.take(LIST) {
        let _ = writeln!(out, "- {item}");
    }
    if total > LIST {
        let _ = writeln!(out, "- …and {} more", total - LIST);
    }
}

/// Real time, as a person would say it: "3 min 20 s".
pub fn duration(seconds: f64) -> String {
    let seconds = seconds.round() as u64;
    match seconds {
        s if s < 60 => format!("{s} s"),
        s if s < 3_600 => format!("{} min {} s", s / 60, s % 60),
        s => format!("{} h {:02} min", s / 3_600, s / 60 % 60),
    }
}

/// How a run ended, in a sentence.
pub fn ending(report: &Report) -> String {
    match (&report.crash, report.ended) {
        (Some(crash), _) => format!(
            "CRASHED: {}",
            crash.lines().last().unwrap_or_default().trim()
        ),
        (None, true) => "Ended cleanly.".to_string(),
        (None, false) => "Never ended: still running, or killed.".to_string(),
    }
}

fn matchup(name: &str, matchup: &Matchup) -> String {
    let [a, b] = &matchup.names;
    let draws = matchup.fought - matchup.wins[0] - matchup.wins[1];
    format!(
        "{name}: {} fought; {a} won {}, {b} won {}, {draws} broken off; men lost {} and {}",
        matchup.fought, matchup.wins[0], matchup.wins[1], matchup.men_lost[0], matchup.men_lost[1]
    )
}

/// The goods whose prices moved most over the run, as a fraction of where
/// they started, most first.
pub(crate) fn price_moves(prices: &std::collections::BTreeMap<String, Series>) -> Vec<String> {
    let mut moves: Vec<(f64, String)> = prices
        .iter()
        .filter_map(|(name, series)| {
            let (&(_, first), &(_, last)) = (series.first()?, series.last()?);
            (first > 0.0).then(|| {
                let by = (last - first) / first;
                (
                    by,
                    format!("{name}: {first:.0} → {last:.0} ({:+.0}%)", by * 100.0),
                )
            })
        })
        .collect();
    moves.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
    moves
        .into_iter()
        .take(PRICE_MOVES)
        .filter(|(by, _)| by.abs() >= 0.01)
        .map(|(_, text)| text)
        .collect()
}

/// `amounts` by port, most first: "Kingston 1.5, Nassau 0.5".
pub(crate) fn per_port(amounts: &std::collections::BTreeMap<String, f64>) -> String {
    let mut amounts: Vec<(&String, &f64)> = amounts.iter().collect();
    amounts.sort_by(|a, b| b.1.total_cmp(a.1));
    amounts
        .iter()
        .map(|(port, amount)| format!("{port} {amount:.1}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether the world makes enough of a good: "short 1.8 a day".
pub(crate) fn balance(supply: &Supply) -> String {
    match supply.balance() {
        by if by < -0.05 => format!("short {:.1} a day", -by),
        by if by > 0.05 => format!("over {by:.1} a day"),
        _ => "even".to_string(),
    }
}

/// Every good and its supply, the shortest first.
pub(crate) fn shortest_first(
    supply: &std::collections::BTreeMap<String, Supply>,
) -> Vec<(&String, &Supply)> {
    let mut goods: Vec<(&String, &Supply)> = supply.iter().collect();
    goods.sort_by(|a, b| a.1.balance().total_cmp(&b.1.balance()));
    goods
}

/// Every good's supply and demand, the shortest first: "Munitions: short
/// 1.8 a day; made 0.5 (Kingston 0.5); convoys up to 0.8; used 3.1 (…)".
pub(crate) fn supply_lines(supply: &std::collections::BTreeMap<String, Supply>) -> Vec<String> {
    shortest_first(supply)
        .into_iter()
        .map(|(good, supply)| {
            let mut parts = vec![balance(supply)];
            if !supply.made.is_empty() {
                let made = per_port(&supply.made);
                parts.push(format!("made {:.1} ({made})", supply.total_made()));
            }
            if supply.convoys > 0.0 {
                parts.push(format!("convoys up to {:.1}", supply.convoys));
            }
            if !supply.used.is_empty() {
                let used = per_port(&supply.used);
                parts.push(format!("used {:.1} ({used})", supply.total_used()));
            }
            format!("{good}: {}", parts.join("; "))
        })
        .collect()
}

/// What a run was and when it began: "False Colours 0.1.0, soak run begun
/// 2026-10-20 18:30 UTC, seed 42".
pub fn begun(report: &Report) -> String {
    let seed = report
        .seed
        .map_or_else(String::new, |seed| format!(", seed {seed}"));
    format!(
        "False Colours {}, {} run begun {}{seed}",
        report.version,
        report.run.name(),
        Stamp::at(report.started),
    )
}

/// The summary of run `id`, against `against` if given: another run, and
/// its id.
pub fn summary(id: &str, report: &Report, against: Option<(&str, &Report)>) -> String {
    let mut out = format!(
        "Run {id}: {}. {:.1} game days in {} real. {}\n",
        begun(report),
        report.days,
        duration(report.real_seconds),
        ending(report),
    );
    list(
        &mut out,
        "Lines that couldn't be read",
        report
            .skipped
            .iter()
            .map(|(line, why)| format!("line {line}: {why}")),
    );
    list(
        &mut out,
        "Alarms",
        report.alarms.iter().map(|alarm| {
            let what = &alarm.what;
            format!(
                "day {:.1}: {}, {}: {}",
                alarm.day,
                what.alarm.name(),
                what.subject,
                what.detail
            )
        }),
    );
    list(
        &mut out,
        "Playtester's notes",
        report
            .notes
            .iter()
            .map(|note| format!("day {:.1}: {}", note.day, note.what)),
    );
    list(
        &mut out,
        "Warnings and errors",
        report
            .logs
            .iter()
            .map(|log| format!("day {:.1}: {}", log.day, log.what)),
    );
    let ours = headline(report);
    if let Some((other, before)) = against {
        let changes: Vec<String> = compare(&headline(before), &ours)
            .iter()
            .filter(|change| change.is_notable())
            .map(|change| {
                let value = |value: Option<f64>| value.map_or("–".to_string(), shown);
                format!(
                    "{}: {} → {} ({})",
                    change.name,
                    value(change.before),
                    value(change.after),
                    shown_change(change)
                )
            })
            .collect();
        let title = format!("Changed since run {other}");
        if changes.is_empty() {
            let _ = writeln!(out, "\n{title}: nothing of note.");
        }
        list(&mut out, &title, changes.into_iter());
    }
    list(
        &mut out,
        "Headline",
        ours.iter()
            .map(|measure| format!("{}: {}", measure.name, shown(measure.value))),
    );
    let battles = &report.battles;
    list(
        &mut out,
        "Battles by class",
        battles.by_class.iter().map(|(name, m)| matchup(name, m)),
    );
    list(
        &mut out,
        "Battles by role",
        battles.by_role.iter().map(|(name, m)| matchup(name, m)),
    );
    let standing = &report.standing;
    list(
        &mut out,
        "The player's standing, by deed",
        standing
            .by_deed
            .iter()
            .map(|(deed, by)| format!("{deed}: {by:+.1}")),
    );
    list(
        &mut out,
        "The player's standing, by group",
        standing
            .by_group
            .iter()
            .map(|(group, by)| format!("{group}: {by:+.1}")),
    );
    let word = &report.word;
    list(
        &mut out,
        "News",
        word.news.iter().map(|(kind, n)| format!("{kind}: {n}")),
    );
    list(
        &mut out,
        "Deeds, by anyone",
        word.deeds.iter().map(|(kind, n)| format!("{kind}: {n}")),
    );
    list(
        &mut out,
        "Deeds, by the player",
        word.player_deeds
            .iter()
            .map(|(kind, n)| format!("{kind}: {n}")),
    );
    if let Some(mean) = word.mean_days() {
        let _ = writeln!(
            out,
            "\nWord reached a nation {} times, after {mean:.1} days on average and {:.1} at most.",
            word.heard, word.slowest_days
        );
    }
    list(
        &mut out,
        "Nations and ports",
        report
            .events
            .iter()
            .map(|event| format!("day {:.1}: {}", event.day, event.what)),
    );
    let economy = &report.economy;
    if economy.earned != 0 || economy.spent != 0 {
        let _ = writeln!(
            out,
            "\nThe player earned {} and spent {} pieces of eight.",
            economy.earned, economy.spent
        );
    }
    list(
        &mut out,
        "Supply and demand, by the ports, the shortest first",
        supply_lines(&economy.supply).into_iter(),
    );
    list(
        &mut out,
        "Prices that moved most",
        price_moves(&economy.prices).into_iter(),
    );
    if report.run == Run::Play {
        let session = &report.session;
        let spent = |map: &std::collections::BTreeMap<String, f64>| {
            map.iter()
                .map(|(what, seconds)| format!("{what}: {}", duration(*seconds)))
                .collect::<Vec<_>>()
        };
        list(
            &mut out,
            "Real time, by speed",
            spent(&session.by_speed).into_iter(),
        );
        list(
            &mut out,
            "Real time, by place",
            spent(&session.by_place).into_iter(),
        );
        let _ = writeln!(out, "\nQuickloads: {}", session.quickloads);
        let mut controls: Vec<(&String, &u32)> = session.controls.iter().collect();
        controls.sort_by(|a, b| b.1.cmp(a.1));
        list(
            &mut out,
            "Controls used",
            controls.iter().map(|(name, n)| format!("{name}: {n}")),
        );
        list(
            &mut out,
            "Alerts",
            session
                .alerts
                .iter()
                .map(|(name, n)| format!("{name}: {n}")),
        );
        list(
            &mut out,
            "Orders, latest first",
            report
                .orders
                .iter()
                .rev()
                .map(|order| format!("day {:.1}: {}", order.day, order.what)),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fold::fold;
    use crate::fold::tests::{battle, line, log, side};
    use chronicle::{Alarm, AlarmKind, BattleEnd, GoodState, PortState, Record, Role, Snapshot};

    fn sample() -> Report {
        let mut lines = vec![
            line(
                2.0,
                10.0,
                Record::Alarm(Alarm {
                    alarm: AlarmKind::Starved,
                    subject: "Nassau: Provisions".into(),
                    detail: "none for 30 days".into(),
                }),
            ),
            line(
                3.0,
                20.0,
                Record::Note {
                    text: "the brig is too strong".into(),
                },
            ),
            line(
                4.0,
                30.0,
                battle(
                    [
                        side("Sloop", Role::Raider, [30, 20]),
                        side("Brig", Role::Patrol, [80, 75]),
                    ],
                    BattleEnd::Taken,
                    Some(0),
                ),
            ),
        ];
        lines.extend((0..30).map(|n| {
            line(
                5.0,
                40.0,
                Record::Order {
                    order: format!("Buy {n} Sugar"),
                },
            )
        }));
        lines.push(line(6.0, 200.0, Record::End));
        fold(&log(lines))
    }

    #[test]
    fn the_summary_leads_with_what_looks_broken_and_what_the_tester_said() {
        let text = summary("run-b", &sample(), None);
        let first = text.lines().next().unwrap();
        assert!(first.starts_with("Run run-b: False Colours 0.1.0, play run begun 2023-11-14"));
        assert!(first.ends_with("6.0 game days in 3 min 20 s real. Ended cleanly."));
        let alarm = text.find("Port starved, Nassau: Provisions").unwrap();
        let note = text.find("the brig is too strong").unwrap();
        let headline = text.find("\nHeadline").unwrap();
        assert!(alarm < note && note < headline);
        assert!(text.contains(
            "Brig v Sloop: 1 fought; Brig won 1, Sloop won 0, 0 broken off; men lost 5 and 10"
        ));
    }

    #[test]
    fn long_lists_are_cut_short_newest_orders_first() {
        let text = summary("run-b", &sample(), None);
        assert!(text.contains("Orders, latest first (30):\n- day 5.0: Buy 29 Sugar"));
        assert!(text.contains("- …and 10 more"));
    }

    #[test]
    fn against_another_run_the_summary_says_what_changed() {
        let before = fold(&log(vec![line(6.0, 100.0, Record::End)]));
        let text = summary("run-b", &sample(), Some(("run-a", &before)));
        assert!(text.contains("Changed since run run-a"));
        assert!(text.contains("Battles: 0 → 1 (+1)"));
        assert!(text.contains("Alarms: 0 → 1 (+1)"));
        assert!(
            !text.contains("Game days: 6 → 6"),
            "unchanged isn't mentioned"
        );
    }

    #[test]
    fn a_crash_is_said_first() {
        let report = fold(&log(vec![line(
            1.0,
            1.0,
            Record::Crash {
                message: "panicked at src/word.rs:10:5:\nno such ship".into(),
            },
        )]));
        assert!(text_first_line(&summary("x", &report, None)).ends_with("CRASHED: no such ship"));
        assert_eq!(duration(3_725.0), "1 h 02 min");
    }

    #[test]
    fn supply_and_demand_shows_what_each_port_makes_and_uses_shortest_first() {
        let good = |good: &str, per_day: f64, convoys| GoodState {
            good: good.into(),
            stock: 0.0,
            target: 10.0,
            price: 1.0,
            needed: per_day < 0.0,
            per_day,
            convoys,
        };
        let port = |name: &str, goods| PortState {
            port: name.into(),
            flag: "England".into(),
            goods,
        };
        let snapshot = Snapshot {
            ports: vec![
                port(
                    "Kingston",
                    vec![good("Munitions", 0.5, 0.0), good("Sugar", 3.0, 0.0)],
                ),
                port("Port Royal", vec![good("Munitions", -1.0, 0.5)]),
                port(
                    "Nassau",
                    vec![good("Munitions", -2.0, 0.0), good("Sugar", -1.0, 0.0)],
                ),
            ],
            ..Snapshot::default()
        };
        let report = fold(&log(vec![line(
            1.0,
            1.0,
            Record::Snapshot(Box::new(snapshot)),
        )]));
        assert_eq!(
            supply_lines(&report.economy.supply),
            [
                "Munitions: short 2.0 a day; made 0.5 (Kingston 0.5); convoys up to 0.5; \
                 used 3.0 (Nassau 2.0, Port Royal 1.0)",
                "Sugar: over 2.0 a day; made 3.0 (Kingston 3.0); used 1.0 (Nassau 1.0)",
            ]
        );
        assert!(summary("x", &report, None).contains("\nSupply and demand"));
    }

    fn text_first_line(text: &str) -> &str {
        text.lines().next().unwrap()
    }
}
