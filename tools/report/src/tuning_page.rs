//! The Tuning page: the game's knobs and balance targets, each with its
//! hints, edited in place, and each target against the newest soak.

use maud::{Markup, html};
use tuning::{Hints, Knob, Target};

use crate::balance::{Verdict, shown};
use crate::pages::page;

/// What the page shows: the files' entries, or why one couldn't be read,
/// the newest soak's verdicts on the targets, and a word on what was just
/// done.
pub struct Tuning<'a> {
    pub knobs: Result<Vec<Knob>, String>,
    pub targets: &'a Result<Vec<Target>, String>,
    /// The soak the targets were measured on, and its verdicts.
    pub measured: Option<(String, Vec<Verdict<'a>>)>,
    /// "Saved x." or why it wasn't.
    pub notice: Option<Result<String, String>>,
}

fn hints(hints: &Hints) -> Markup {
    html! {
        dl {
            dt { "What it does" } dd { (hints.does) }
            dt { "In play" } dd { (hints.play) }
            dt { "History" } dd { (hints.history) }
        }
    }
}

/// The entries in groups, in the order the file first lists each group.
fn grouped<'a, T>(
    entries: &'a [T],
    group: impl Fn(&'a T) -> &'a str,
) -> Vec<(&'a str, Vec<&'a T>)> {
    let mut groups: Vec<(&str, Vec<&T>)> = Vec::new();
    for entry in entries {
        let name = group(entry);
        match groups.iter_mut().find(|(other, _)| *other == name) {
            Some((_, members)) => members.push(entry),
            None => groups.push((name, vec![entry])),
        }
    }
    groups
}

fn number(value: Option<f64>) -> String {
    value.map_or(String::new(), |value| value.to_string())
}

fn target_entry(target: &Target, verdict: Option<&Verdict>) -> Markup {
    let measured = verdict.and_then(|verdict| Some((verdict.value?, verdict.off())));
    html! {
        div.entry id=(target.id) {
            h4 { (target.name) " " span.faded { "(" (target.unit) ")" } }
            form method="post" action="/tuning/target" {
                input type="hidden" name="id" value=(target.id);
                "Good between "
                input type="number" step="any" name="min" value=(number(target.min)) placeholder="no least";
                " and "
                input type="number" step="any" name="max" value=(number(target.max)) placeholder="no most";
                " "
                button { "Save" }
            }
            @match measured {
                Some((value, true)) => p.bad { "Newest soak: " (shown(&target.id, value)) ", off target (wants " (target.range()) ")." },
                Some((value, false)) => p.good { "Newest soak: " (shown(&target.id, value)) ", on target." },
                None => p.faded { "Not measured yet: run a soak (cargo soak --days 100)." },
            }
            (hints(&target.hints))
        }
    }
}

fn knob_entry(knob: &Knob) -> Markup {
    html! {
        div.entry id=(knob.id) {
            h4 { (knob.name) " " span.faded { "(" (knob.unit) ")" } }
            form method="post" action="/tuning/knob" {
                input type="hidden" name="id" value=(knob.id);
                input type="number" step="any" name="value" value=(knob.value) min=(knob.min) max=(knob.max);
                " "
                button { "Save" }
                span.faded {
                    " from " (knob.min) " to " (knob.max)
                    @if let Some(historical) = knob.historical { "; historically " (historical) }
                }
            }
            (hints(&knob.hints))
        }
    }
}

pub fn tuning_page(tuning: &Tuning) -> Markup {
    let verdicts = tuning.measured.as_ref().map(|(_, verdicts)| verdicts);
    let verdict =
        |id: &str| verdicts.and_then(|verdicts| verdicts.iter().find(|v| v.target.id == id));
    page(
        "Tuning",
        html! {
            h1 { "Tuning" }
            p {
                "Knobs change how the game plays; they take effect on the next "
                code { "cargo soak" } " or " code { "cargo run" } ", which rebuild for them. "
                "Targets change nothing in the game: they say what a good soak looks like, and each soak's summary lists the ones it missed. "
                "Both are saved to the game's " code { "assets/data" } " folder. See " code { "docs/BALANCE.md" } "."
            }
            @match &tuning.notice {
                Some(Ok(saved)) => p.notice.good { (saved) },
                Some(Err(error)) => p.notice.bad { (error) },
                None => {}
            }
            h2 { "Balance targets" }
            @if let Some((soak, _)) = &tuning.measured {
                p.faded { "Measured on soak " a href={ "/runs/" (soak) } { (soak) } "." }
            }
            @match tuning.targets {
                Ok(targets) => {
                    @for (group, members) in grouped(targets, |target| target.group.as_str()) {
                        h3 { (group) }
                        @for target in members { (target_entry(target, verdict(&target.id))) }
                    }
                }
                Err(error) => p.bad { (error) },
            }
            h2 { "Knobs" }
            @match &tuning.knobs {
                Ok(knobs) => {
                    @for (group, members) in grouped(knobs, |knob| knob.group.as_str()) {
                        h3 { (group) }
                        @for knob in members { (knob_entry(knob)) }
                    }
                }
                Err(error) => p.bad { (error) },
            }
        },
    )
}
