//! The pages, as HTML: every run, one run, two compared, the trend over
//! runs, and crash reports. They only lay out what the report worked out.

use std::collections::BTreeMap;

use chronicle::{Battle, Run, Side};
use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::chart::chart;
use crate::fold::{Matchup, Report, Series};
use crate::headline::{Measure, compare, headline, shown, shown_change};
use crate::summary::{balance, begun, duration, ending, per_port, price_moves, shortest_first};

/// The most battles and orders listed on a run's page.
const LISTED: usize = 100;
/// The headline numbers the trend page charts.
const TRENDED: [&str; 6] = [
    "Battles",
    "Alarms",
    "Ships at the end",
    "Days for word to reach a nation",
    "Real seconds a game day",
    "Peak memory, MB",
];

const STYLE: &str = r#"
:root { --ink: #222; --faded: #666; --paper: #fbf8f1; --rule: #ddd4c0; --bad: #a93226; --link: #1f5f99; }
@media (prefers-color-scheme: dark) {
  :root { --ink: #e8e2d4; --faded: #a59c8a; --paper: #1d1b18; --rule: #3d3830; --bad: #e57368; --link: #7fb3e0; }
}
body { font: 15px/1.45 system-ui, sans-serif; color: var(--ink); background: var(--paper); margin: 0 auto; max-width: 980px; padding: 16px; }
a { color: var(--link); }
h1 { font-size: 1.5em; margin-bottom: 0.2em; }
h2 { font-size: 1.15em; border-bottom: 1px solid var(--rule); padding-bottom: 2px; margin-top: 1.8em; }
nav a { margin-right: 1em; }
table { border-collapse: collapse; margin: 0.5em 0; }
th, td { padding: 3px 10px 3px 0; text-align: left; vertical-align: top; border-bottom: 1px solid var(--rule); }
td.n, th.n { text-align: right; }
.faded { color: var(--faded); }
.bad { color: var(--bad); font-weight: 600; }
.notable td { font-weight: 600; }
figure.chart { margin: 1em 0; }
figure.chart svg { width: 100%; height: auto; }
figcaption { font-weight: 600; }
.grid { stroke: var(--rule); }
.axis { fill: var(--faded); font-size: 11px; }
ul.legend { list-style: none; padding: 0; display: flex; flex-wrap: wrap; gap: 4px 16px; font-size: 13px; }
.swatch { display: inline-block; width: 12px; height: 12px; margin-right: 5px; vertical-align: -1px; }
pre { white-space: pre-wrap; background: var(--rule); padding: 8px; }
.wide { overflow-x: auto; }
"#;

fn page(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · False Colours logbook" }
                style { (PreEscaped(STYLE)) }
            }
            body {
                nav { a href="/" { "Runs" } a href="/trend?kind=soak" { "Soak trend" } a href="/trend?kind=play" { "Play trend" } a href="/crashes" { "Crashes" } }
                (body)
            }
        }
    }
}

pub fn not_found(what: &str) -> Markup {
    page("Not found", html! { h1 { "Not found" } p { (what) } })
}

fn table(headers: &[&str], rows: impl IntoIterator<Item = Vec<String>>) -> Markup {
    html! {
        div.wide { table {
            tr { @for header in headers { th { (header) } } }
            @for row in rows { tr { @for cell in row { td { (cell) } } } }
        } }
    }
}

fn counts(title: &str, map: &BTreeMap<String, u32>) -> Markup {
    if map.is_empty() {
        return html! {};
    }
    html! {
        h3 { (title) }
        (table(&["", "Count"], map.iter().map(|(name, n)| vec![name.clone(), n.to_string()])))
    }
}

fn runs_line(report: &Report) -> String {
    format!(
        "{}: {:.1} game days in {}.",
        begun(report),
        report.days,
        duration(report.real_seconds)
    )
}

/// Every run, newest first, picked out by kind if `kind` is given.
pub fn index(runs: &[(String, Report)], kind: Option<Run>) -> Markup {
    page(
        "Runs",
        html! {
            h1 { "Runs" }
            p {
                "Show: " a href="/" { "all" } " · " a href="/?kind=play" { "play" } " · " a href="/?kind=soak" { "soak" }
            }
            @if runs.is_empty() {
                p.faded { "No runs yet. Play with the chronicle (`cargo run --features dev`) or soak (`cargo soak`)." }
            } @else {
                form action="/compare" {
                    p { "Pick a run in each of A and B to compare them. " button type="submit" { "Compare" } }
                    div.wide { table {
                        tr { th { "A" } th { "B" } th { "Run" } th { "Kind" } th { "Version" } th.n { "Days" } th.n { "Real" } th.n { "Alarms" } th.n { "Battles" } th { "Ending" } }
                        @for (id, report) in runs.iter().filter(|(_, report)| kind.is_none_or(|kind| report.run == kind)) {
                            tr {
                                td { input type="radio" name="a" value=(id); }
                                td { input type="radio" name="b" value=(id); }
                                td { a href=(format!("/runs/{id}")) { (id) } }
                                td { (report.run.name()) }
                                td { (report.version) }
                                td.n { (format!("{:.1}", report.days)) }
                                td.n { (duration(report.real_seconds)) }
                                td.n class=[(!report.alarms.is_empty()).then_some("bad")] { (report.alarms.len()) }
                                td.n { (report.battles.list.len()) }
                                td class=[report.crash.is_some().then_some("bad")] { (ending(report)) }
                            }
                        }
                    } }
                }
            }
        },
    )
}

fn side(side: &Side) -> String {
    let nation = side.nation.as_deref().unwrap_or("no colours");
    format!(
        "{} ({} {}, {nation}, {} guns): crew {}→{}, timbers {:.0}%",
        side.ship,
        side.role,
        side.class,
        side.guns,
        side.crew[0],
        side.crew[1],
        side.timbers[1] * 100.0
    )
}

fn battle_row(battle: &Battle) -> Vec<String> {
    let loser = battle
        .loser
        .map_or("–".to_string(), |loser| battle.sides[loser].ship.clone());
    vec![
        format!("{:.1}", battle.began),
        side(&battle.sides[0]),
        side(&battle.sides[1]),
        battle.end.name().to_string(),
        loser,
    ]
}

fn matchups(title: &str, matchups: &BTreeMap<String, Matchup>) -> Markup {
    if matchups.is_empty() {
        return html! {};
    }
    html! {
        h3 { (title) }
        (table(
            &["Matchup", "Fought", "First won", "Second won", "Broken off", "Men lost"],
            matchups.iter().map(|(name, m)| vec![
                name.clone(),
                m.fought.to_string(),
                format!("{} {}", m.names[0], m.wins[0]),
                format!("{} {}", m.names[1], m.wins[1]),
                (m.fought - m.wins[0] - m.wins[1]).to_string(),
                format!("{} / {}", m.men_lost[0], m.men_lost[1]),
            ]),
        ))
    }
}

fn series_of(map: &BTreeMap<String, Series>) -> Vec<(String, &Series)> {
    map.iter()
        .map(|(name, series)| (name.clone(), series))
        .collect()
}

fn headline_table(measures: &[Measure]) -> Markup {
    table(
        &["", "Value"],
        measures
            .iter()
            .map(|measure| vec![measure.name.clone(), shown(measure.value)]),
    )
}

/// One run, in full, with its summary to copy.
pub fn run_page(
    id: &str,
    report: &Report,
    summary: &str,
    previous: Option<&str>,
    crash_report: bool,
) -> Markup {
    let timelines = &report.timelines;
    let mut by_port: BTreeMap<&str, Vec<(String, &Series)>> = BTreeMap::new();
    for (name, series) in &report.economy.prices {
        let (port, good) = name.split_once(": ").unwrap_or((name, name));
        by_port
            .entry(port)
            .or_default()
            .push((good.to_string(), series));
    }
    let world = [
        ("Entities".to_string(), &timelines.entities),
        ("News in the ledger".to_string(), &timelines.news),
        ("Open contracts".to_string(), &timelines.open_jobs),
    ];
    page(
        id,
        html! {
            h1 { (id) }
            p { (runs_line(report)) " " span class=[report.crash.is_some().then_some("bad")] { (ending(report)) } }
            p {
                a href=(format!("/runs/{id}/summary.txt")) { "Summary as text" }
                @if let Some(previous) = previous { " · " a href=(format!("/compare?a={previous}&b={id}")) { "Compare with the run before" } }
                @if crash_report { " · " a.bad href=(format!("/crashes/{id}")) { "Crash report" } }
            }
            p {
                button onclick="navigator.clipboard.writeText(document.getElementById('summary').textContent).then(() => this.textContent = 'Copied')" { "Copy summary for Claude" }
            }
            details { summary { "The summary" } pre id="summary" { (summary) } }

            @if !report.skipped.is_empty() {
                h2.bad { "Lines that couldn't be read" }
                (table(&["Line", "Why"], report.skipped.iter().map(|(line, why)| vec![line.to_string(), why.clone()])))
            }
            h2 { "Alarms" }
            @if report.alarms.is_empty() { p.faded { "None." } } @else {
                (table(&["Day", "Alarm", "About", "Detail"], report.alarms.iter().map(|a| vec![
                    format!("{:.1}", a.day), a.what.alarm.name().to_string(), a.what.subject.clone(), a.what.detail.clone(),
                ])))
            }
            @if !report.notes.is_empty() {
                h2 { "Notes" }
                (table(&["Day", "Note"], report.notes.iter().map(|n| vec![format!("{:.1}", n.day), n.what.clone()])))
            }
            @if !report.logs.is_empty() {
                h2 { "Warnings and errors" }
                (table(&["Day", "Message"], report.logs.iter().map(|l| vec![format!("{:.1}", l.day), l.what.clone()])))
            }

            h2 { "Headline" }
            (headline_table(&headline(report)))

            h2 { "Over time" }
            (chart("The player's purse", "day", &[("purse".to_string(), &timelines.purse)]))
            (chart("The player's standing", "day", &series_of(&timelines.standing)))
            (chart("Relations between nations", "day", &series_of(&timelines.relations)))
            (chart("Ships, by role", "day", &series_of(&timelines.ships)))
            (chart("Memory, MB", "day", &[("memory".to_string(), &timelines.memory_mb)]))
            (chart("The world", "day", &world.iter().map(|(name, series)| (name.clone(), *series)).collect::<Vec<_>>()))

            h2 { "Battles" }
            (counts("How they ended", &report.battles.ends))
            (matchups("By class", &report.battles.by_class))
            (matchups("By role", &report.battles.by_role))
            @if !report.battles.list.is_empty() {
                h3 { "The latest" }
                (table(&["Day", "Attacker", "Attacked", "End", "Lost"], report.battles.list.iter().rev().take(LISTED).map(battle_row)))
            }

            h2 { "Deeds and word" }
            @if !report.standing.by_deed.is_empty() {
                h3 { "The player's standing, by deed" }
                (table(&["Deed", "Moved"], report.standing.by_deed.iter().map(|(d, by)| vec![d.clone(), format!("{by:+.1}")])))
                h3 { "The player's standing, by group" }
                (table(&["Group", "Moved"], report.standing.by_group.iter().map(|(g, by)| vec![g.clone(), format!("{by:+.1}")])))
            }
            (counts("News", &report.word.news))
            (counts("Deeds, by anyone", &report.word.deeds))
            (counts("Deeds, by the player", &report.word.player_deeds))
            @if let Some(mean) = report.word.mean_days() {
                p { "Word reached a nation " (report.word.heard) " times, after " (format!("{mean:.1}")) " days on average and " (format!("{:.1}", report.word.slowest_days)) " at most." }
            }
            @if !report.events.is_empty() {
                h3 { "Nations and ports" }
                (table(&["Day", "What"], report.events.iter().map(|e| vec![format!("{:.1}", e.day), e.what.clone()])))
            }

            h2 { "Economy" }
            p { "The player earned " (report.economy.earned) " and spent " (report.economy.spent) " pieces of eight." }
            @if !report.economy.supply.is_empty() {
                h3 { "Supply and demand, by the ports, tons or heads a day" }
                (table(
                    &["Good", "Balance", "Made", "Convoys, at most", "Used"],
                    shortest_first(&report.economy.supply).into_iter().map(|(good, supply)| vec![
                        good.clone(),
                        balance(supply),
                        format!("{:.1} {}", supply.total_made(), per_port(&supply.made)),
                        format!("{:.1}", supply.convoys),
                        format!("{:.1} {}", supply.total_used(), per_port(&supply.used)),
                    ]),
                ))
            }
            @let moves = price_moves(&report.economy.prices);
            @if !moves.is_empty() {
                h3 { "Prices that moved most" }
                ul { @for line in moves { li { (line) } } }
            }
            @for (port, goods) in &by_port {
                (chart(&format!("Prices at {port}"), "day", goods))
            }

            @if report.run == Run::Play {
                h2 { "The session" }
                (table(&["Time", "Real"], report.session.by_speed.iter().chain(&report.session.by_place).map(|(what, s)| vec![what.clone(), duration(*s)])))
                p { "Quickloads: " (report.session.quickloads) }
                (counts("Controls used", &report.session.controls))
                (counts("Alerts", &report.session.alerts))
                @if !report.orders.is_empty() {
                    h3 { "Orders, latest first" }
                    (table(&["Day", "Order"], report.orders.iter().rev().take(LISTED).map(|o| vec![format!("{:.1}", o.day), o.what.clone()])))
                }
            }
        },
    )
}

/// Two runs side by side: `before`'s and `after`'s headline numbers, and
/// their days on the same charts.
pub fn compare_page(before: (&str, &Report), after: (&str, &Report)) -> Markup {
    let changes = compare(&headline(before.1), &headline(after.1));
    let both = |pick: fn(&Report) -> &BTreeMap<String, Series>| -> Vec<(String, &Series)> {
        [("A", before.1), ("B", after.1)]
            .into_iter()
            .flat_map(|(label, report)| {
                pick(report)
                    .iter()
                    .map(move |(name, series)| (format!("{label}: {name}"), series))
            })
            .collect()
    };
    let one = |pick: fn(&Report) -> &Series| -> Vec<(String, &Series)> {
        vec![
            ("A".to_string(), pick(before.1)),
            ("B".to_string(), pick(after.1)),
        ]
    };
    page(
        "Compare",
        html! {
            h1 { "A against B" }
            p { "A: " a href=(format!("/runs/{}", before.0)) { (before.0) } ". " (runs_line(before.1)) " " (ending(before.1)) }
            p { "B: " a href=(format!("/runs/{}", after.0)) { (after.0) } ". " (runs_line(after.1)) " " (ending(after.1)) }
            h2 { "Headline" }
            div.wide { table {
                tr { th { "" } th.n { "A" } th.n { "B" } th.n { "Change" } }
                @for change in &changes {
                    tr class=[change.is_notable().then_some("notable")] {
                        td { (change.name) }
                        td.n { (change.before.map_or("–".to_string(), shown)) }
                        td.n { (change.after.map_or("–".to_string(), shown)) }
                        td.n { (shown_change(change)) }
                    }
                }
            } }
            h2 { "Over time" }
            (chart("Relations between nations", "day", &both(|r| &r.timelines.relations)))
            (chart("Ships, by role", "day", &both(|r| &r.timelines.ships)))
            (chart("The player's purse", "day", &one(|r| &r.timelines.purse)))
            (chart("Memory, MB", "day", &one(|r| &r.timelines.memory_mb)))
            (chart("Entities", "day", &one(|r| &r.timelines.entities)))
        },
    )
}

/// The headline numbers of `runs` of one kind, oldest first.
pub fn trend_page(kind: Run, runs: &[(String, Report)]) -> Markup {
    let headlines: Vec<Vec<Measure>> = runs.iter().map(|(_, report)| headline(report)).collect();
    let value = |run: usize, name: &str| {
        headlines[run]
            .iter()
            .find(|measure| measure.name == name)
            .map(|measure| measure.value)
    };
    let mut names: Vec<String> = Vec::new();
    for measure in headlines.iter().flatten() {
        if !names.contains(&measure.name) {
            names.push(measure.name.clone());
        }
    }
    let trended: Vec<(String, Series)> = TRENDED
        .iter()
        .map(|&name| {
            let series = (0..runs.len())
                .filter_map(|run| Some(((run + 1) as f64, value(run, name)?)))
                .collect();
            (name.to_string(), series)
        })
        .collect();
    page(
        "Trend",
        html! {
            h1 { "The trend over " (kind.name()) " runs" }
            @if runs.is_empty() { p.faded { "No " (kind.name()) " runs yet." } } @else {
                p { "The last " (runs.len()) ", oldest first, numbered 1 on." }
                @for (name, series) in &trended {
                    (chart(name, "run", &[(name.clone(), series)]))
                }
                h2 { "Every headline number" }
                div.wide { table {
                    tr {
                        th { "" }
                        @for (index, (id, _)) in runs.iter().enumerate() { th.n { a href=(format!("/runs/{id}")) title=(id) { (index + 1) } } }
                    }
                    @for name in &names {
                        tr {
                            td { (name) }
                            @for run in 0..runs.len() { td.n { (value(run, name).map_or("–".to_string(), shown)) } }
                        }
                    }
                } }
            }
        },
    )
}

pub fn crashes_page(crashes: &[String]) -> Markup {
    page(
        "Crashes",
        html! {
            h1 { "Crash reports" }
            @if crashes.is_empty() { p.faded { "None. Long may it last." } } @else {
                ul { @for id in crashes { li { a href=(format!("/crashes/{id}")) { (id) } " · " a href=(format!("/runs/{id}")) { "its run" } } } }
            }
        },
    )
}

pub fn crash_page(id: &str, text: &str) -> Markup {
    page(
        id,
        html! {
            h1.bad { "Crash: " (id) }
            p { a href=(format!("/runs/{id}")) { "Its run" } }
            pre { (text) }
        },
    )
}
