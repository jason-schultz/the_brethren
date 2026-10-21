//! Folding a log into a report, in one pass over its lines: the numbers
//! every page and the summary are drawn from.

use std::collections::BTreeMap;

use chronicle::{Alarm, Battle, Happening, Line, Log, Record, Run, Snapshot};
use serde::{Deserialize, Serialize};

/// The version of the report, to know a cached one is out of date.
pub const REPORT_FORMAT: u32 = 1;

/// Values over game days.
pub type Series = Vec<(f64, f64)>;

/// Something that happened on a game day.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Dated<T> {
    pub day: f64,
    pub what: T,
}

/// What a run came to.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Report {
    pub run: Run,
    pub version: String,
    /// When it began, seconds since the Unix epoch.
    pub started: u64,
    /// The game days it covered, and the real seconds they took.
    pub days: f64,
    pub real_seconds: f64,
    /// Whether it ended cleanly...
    pub ended: bool,
    /// ...or crashed, saying this.
    pub crash: Option<String>,
    /// Lines that couldn't be read, by number, and why.
    pub skipped: Vec<(usize, String)>,
    pub alarms: Vec<Dated<Alarm>>,
    pub notes: Vec<Dated<String>>,
    /// Warnings and errors the game logged.
    pub logs: Vec<Dated<String>>,
    /// The player's orders.
    pub orders: Vec<Dated<String>>,
    /// Nations coming to new terms, and ports going over.
    pub events: Vec<Dated<String>>,
    pub timelines: Timelines,
    pub battles: Battles,
    pub standing: Standing,
    pub word: Word,
    pub economy: Economy,
    pub session: Session,
}

/// How things went day by day, from the snapshots.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Timelines {
    pub purse: Series,
    /// The player's standing, by group.
    pub standing: BTreeMap<String, Series>,
    /// By pair of nations: "England and Spain".
    pub relations: BTreeMap<String, Series>,
    /// Ships in the world, by role.
    pub ships: BTreeMap<String, Series>,
    pub memory_mb: Series,
    pub entities: Series,
    /// Pieces of news in the ledger.
    pub news: Series,
    pub open_jobs: Series,
}

/// Every battle, and how each matchup went.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Battles {
    pub list: Vec<Battle>,
    /// How many ended each way, by the end's name.
    pub ends: BTreeMap<String, u32>,
    /// By the two classes, "Brig v Sloop".
    pub by_class: BTreeMap<String, Matchup>,
    /// By the two roles, "merchant v raider".
    pub by_role: BTreeMap<String, Matchup>,
}

/// The battles between two kinds of ship, named in order.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Matchup {
    pub names: [String; 2],
    pub fought: u32,
    /// Battles each side won.
    pub wins: [u32; 2],
    /// Men each side lost, all battles together.
    pub men_lost: [u32; 2],
}

/// What moved the player's standing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Standing {
    /// The total change, by kind of deed...
    pub by_deed: BTreeMap<String, f64>,
    /// ...and by group.
    pub by_group: BTreeMap<String, f64>,
}

/// News, and how fast word of deeds travelled.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Word {
    /// Pieces of news, by kind.
    pub news: BTreeMap<String, u32>,
    /// Deeds, by kind...
    pub deeds: BTreeMap<String, u32>,
    /// ...and the player's.
    pub player_deeds: BTreeMap<String, u32>,
    /// Times a nation heard of something, and the days it took in all...
    pub heard: u32,
    pub heard_after_days: f64,
    /// ...and at most.
    pub slowest_days: f64,
}

impl Word {
    /// The mean days word took to reach a nation, if it reached any.
    pub fn mean_days(&self) -> Option<f64> {
        (self.heard > 0).then(|| self.heard_after_days / f64::from(self.heard))
    }
}

/// Money and markets.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Economy {
    /// What came into the player's purse, and what went out.
    pub earned: i64,
    pub spent: i64,
    /// Prices by port and good, "Kingston: Sugar".
    pub prices: BTreeMap<String, Series>,
}

/// How a play session's real time went, and what was used in it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Session {
    /// Real seconds spent paused, and at each speed: "paused", "60×".
    pub by_speed: BTreeMap<String, f64>,
    /// Real seconds in port and at sea.
    pub by_place: BTreeMap<String, f64>,
    pub quickloads: u32,
    /// Controls used, by name, and alerts raised, by name.
    pub controls: BTreeMap<String, u32>,
    pub alerts: BTreeMap<String, u32>,
    /// What the clock and the player were doing at the latest line.
    state: SessionState,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SessionState {
    real: f64,
    speed: f64,
    paused: bool,
    in_port: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            real: 0.0,
            speed: 1.0,
            paused: false,
            in_port: false,
        }
    }
}

impl Session {
    /// Counts the real time since the last line as it was spent.
    fn advance(&mut self, real: f64) {
        let state = &mut self.state;
        let spent = (real - state.real).max(0.0);
        state.real = real;
        let speed = if state.paused {
            "paused".to_string()
        } else {
            format!("{}×", state.speed)
        };
        *self.by_speed.entry(speed).or_default() += spent;
        let place = if state.in_port { "in port" } else { "at sea" };
        *self.by_place.entry(place.to_string()).or_default() += spent;
    }
}

/// Counts one more of `key`.
fn count(map: &mut BTreeMap<String, u32>, key: &str) {
    *map.entry(key.to_string()).or_default() += 1;
}

/// Adds the point (`day`, `value`) to `key`'s series.
fn plot(map: &mut BTreeMap<String, Series>, key: String, day: f64, value: f64) {
    map.entry(key).or_default().push((day, value));
}

/// Folds a log into its report.
pub fn fold(log: &Log) -> Report {
    let mut report = Report {
        run: log.run,
        version: log.version.clone(),
        started: log.started,
        days: 0.0,
        real_seconds: 0.0,
        ended: false,
        crash: None,
        skipped: log.skipped.clone(),
        alarms: Vec::new(),
        notes: Vec::new(),
        logs: Vec::new(),
        orders: Vec::new(),
        events: Vec::new(),
        timelines: Timelines::default(),
        battles: Battles::default(),
        standing: Standing::default(),
        word: Word::default(),
        economy: Economy::default(),
        session: Session::default(),
    };
    for line in &log.lines {
        report.add(line);
    }
    report
}

impl Report {
    fn add(
        &mut self,
        &Line {
            day,
            real,
            ref record,
        }: &Line,
    ) {
        self.days = self.days.max(day);
        self.real_seconds = self.real_seconds.max(real);
        self.session.advance(real);
        let dated = |what: String| Dated { day, what };
        match record {
            Record::Header { .. } => {}
            Record::End => self.ended = true,
            Record::Crash { message } => self.crash = Some(message.clone()),
            Record::Log { level, message } => self.logs.push(dated(format!("{level}: {message}"))),
            Record::Note { text } => self.notes.push(dated(text.clone())),
            Record::Alarm(alarm) => self.alarms.push(Dated {
                day,
                what: alarm.clone(),
            }),
            Record::Control { control } => {
                count(&mut self.session.controls, control);
                if control == "Quickload" {
                    self.session.quickloads += 1;
                }
            }
            Record::Order { order } => self.orders.push(dated(order.clone())),
            Record::Alert { alert } => count(&mut self.session.alerts, alert),
            Record::Dock { port } => self.session.state.in_port = port.is_some(),
            Record::Clock { speed, paused } => {
                self.session.state.speed = *speed;
                self.session.state.paused = *paused;
            }
            Record::Money { by, purse } => {
                if *by > 0 {
                    self.economy.earned += by;
                } else {
                    self.economy.spent -= by;
                }
                self.timelines.purse.push((day, f64::from(*purse)));
            }
            Record::News(news) => {
                count(&mut self.word.news, happening_name(news.happening));
                if let Some(deed) = &news.deed {
                    count(&mut self.word.deeds, deed);
                    if news.player {
                        count(&mut self.word.player_deeds, deed);
                    }
                }
            }
            Record::Heard { after_days, .. } => {
                self.word.heard += 1;
                self.word.heard_after_days += after_days;
                self.word.slowest_days = self.word.slowest_days.max(*after_days);
            }
            Record::Standing {
                group, by, deed, ..
            } => {
                *self.standing.by_deed.entry(deed.clone()).or_default() += f64::from(*by);
                *self.standing.by_group.entry(group.clone()).or_default() += f64::from(*by);
            }
            Record::Terms {
                nations: [a, b],
                terms,
                ..
            } => self
                .events
                .push(dated(format!("{a} and {b} are now {terms}"))),
            Record::WentOver { port, from, to } => self
                .events
                .push(dated(format!("{port} went over from {from} to {to}"))),
            Record::Battle(battle) => self.battles.add(battle),
            Record::Snapshot(snapshot) => self.timelines.add(day, snapshot, &mut self.economy),
        }
    }
}

fn happening_name(happening: Happening) -> &'static str {
    match happening {
        Happening::Sailed => "sailings",
        Happening::Overdue => "ships overdue",
        Happening::Deed => "deeds",
        Happening::Wrecked => "wrecks",
        Happening::Account => "accounts of losses",
    }
}

impl Battles {
    fn add(&mut self, battle: &Battle) {
        count(&mut self.ends, battle.end.name());
        let [a, b] = &battle.sides;
        let men_lost = battle
            .sides
            .clone()
            .map(|side| side.crew[0].saturating_sub(side.crew[1]));
        for (map, names) in [
            (&mut self.by_class, [&a.class, &b.class]),
            (
                &mut self.by_role,
                [&a.role.to_string(), &b.role.to_string()],
            ),
        ] {
            // Each matchup is kept one way round, whoever started it.
            let flipped = names[0] > names[1];
            let [first, second] = ordered(names, flipped).map(String::clone);
            let matchup = map
                .entry(format!("{first} v {second}"))
                .or_insert_with(|| Matchup {
                    names: [first, second],
                    ..Matchup::default()
                });
            matchup.fought += 1;
            for (total, lost) in matchup.men_lost.iter_mut().zip(ordered(men_lost, flipped)) {
                *total += lost;
            }
            if let Some(loser) = battle.loser {
                let winner = if flipped { loser } else { 1 - loser };
                matchup.wins[winner] += 1;
            }
        }
        self.list.push(battle.clone());
    }
}

/// `pair`, the other way round if `flipped`.
fn ordered<T>([a, b]: [T; 2], flipped: bool) -> [T; 2] {
    if flipped { [b, a] } else { [a, b] }
}

impl Timelines {
    fn add(&mut self, day: f64, snapshot: &Snapshot, economy: &mut Economy) {
        if let Some(player) = &snapshot.player {
            self.purse.push((day, f64::from(player.purse)));
            for standing in &player.standing {
                plot(
                    &mut self.standing,
                    standing.group.clone(),
                    day,
                    f64::from(standing.value),
                );
            }
        }
        for pair in &snapshot.relations {
            let [a, b] = &pair.nations;
            plot(
                &mut self.relations,
                format!("{a} and {b}"),
                day,
                f64::from(pair.value),
            );
        }
        let mut ships: BTreeMap<String, f64> = BTreeMap::new();
        for fleet in &snapshot.fleets {
            *ships.entry(fleet.role.to_string()).or_default() += f64::from(fleet.ships);
        }
        for (role, count) in ships {
            plot(&mut self.ships, role, day, count);
        }
        if let Some(kb) = snapshot.memory_kb {
            self.memory_mb.push((day, kb as f64 / 1024.0));
        }
        self.entities.push((day, f64::from(snapshot.entities)));
        self.news.push((day, f64::from(snapshot.news)));
        self.open_jobs.push((day, f64::from(snapshot.open_jobs)));
        for port in &snapshot.ports {
            for good in &port.goods {
                plot(
                    &mut economy.prices,
                    format!("{}: {}", port.port, good.good),
                    day,
                    good.price,
                );
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use chronicle::{BattleEnd, Role, Side};

    pub fn line(day: f64, real: f64, record: Record) -> Line {
        Line { day, real, record }
    }

    pub fn log(lines: Vec<Line>) -> Log {
        Log {
            run: Run::Play,
            version: "0.1.0".into(),
            started: 1_700_000_000,
            lines,
            skipped: Vec::new(),
        }
    }

    pub fn side(class: &str, role: Role, crew: [u32; 2]) -> Side {
        Side {
            ship: format!("the {class}"),
            class: class.into(),
            role,
            nation: None,
            guns: 8,
            crew,
            timbers: [1.0, 0.5],
            rigging: [1.0, 0.5],
        }
    }

    pub fn battle(sides: [Side; 2], end: BattleEnd, loser: Option<usize>) -> Record {
        Record::Battle(Battle {
            began: 1.0,
            sides,
            end,
            loser,
        })
    }

    #[test]
    fn matchups_are_kept_one_way_round_whoever_started_the_fight() {
        let report = fold(&log(vec![
            line(
                1.0,
                1.0,
                battle(
                    [
                        side("Sloop", Role::Raider, [30, 25]),
                        side("Brig", Role::Patrol, [80, 78]),
                    ],
                    BattleEnd::Taken,
                    Some(0),
                ),
            ),
            line(
                2.0,
                2.0,
                battle(
                    [
                        side("Brig", Role::Patrol, [80, 70]),
                        side("Sloop", Role::Raider, [30, 30]),
                    ],
                    BattleEnd::BrokeOff,
                    None,
                ),
            ),
        ]));
        let matchup = &report.battles.by_class["Brig v Sloop"];
        assert_eq!(matchup.names, ["Brig", "Sloop"]);
        assert_eq!(matchup.fought, 2);
        assert_eq!(matchup.wins, [1, 0], "the brig took the sloop");
        assert_eq!(matchup.men_lost, [12, 5]);
        assert_eq!(report.battles.by_role["patrol v raider"].wins, [1, 0]);
        assert_eq!(report.battles.ends["taken"], 1);
        assert_eq!(report.battles.ends["broke off"], 1);
    }

    #[test]
    fn real_time_is_counted_by_speed_and_place_as_it_was_spent() {
        let clock = |speed, paused| Record::Clock { speed, paused };
        let report = fold(&log(vec![
            line(0.0, 0.0, clock(1.0, false)),
            line(
                0.0,
                0.0,
                Record::Dock {
                    port: Some("Port Royal".into()),
                },
            ),
            line(0.1, 30.0, Record::Dock { port: None }),
            line(0.2, 40.0, clock(600.0, false)),
            line(1.0, 100.0, clock(600.0, true)),
            line(1.0, 120.0, Record::End),
        ]));
        let session = &report.session;
        assert_eq!(session.by_speed["1×"], 40.0);
        assert_eq!(session.by_speed["600×"], 60.0);
        assert_eq!(session.by_speed["paused"], 20.0);
        assert_eq!(session.by_place["in port"], 30.0);
        assert_eq!(session.by_place["at sea"], 90.0);
        assert!(report.ended);
        assert_eq!(report.real_seconds, 120.0);
    }

    #[test]
    fn the_players_standing_is_totalled_by_deed_and_group_and_money_by_way() {
        let standing = |by: f32, deed: &str| Record::Standing {
            group: "the Royal Navy".into(),
            by,
            now: 0.0,
            deed: deed.into(),
        };
        let report = fold(&log(vec![
            line(1.0, 1.0, standing(-20.0, "Captured")),
            line(2.0, 2.0, standing(-5.0, "Captured")),
            line(3.0, 3.0, standing(3.0, "Delivered")),
            line(
                3.0,
                3.0,
                Record::Money {
                    by: 300,
                    purse: 800,
                },
            ),
            line(
                4.0,
                4.0,
                Record::Money {
                    by: -120,
                    purse: 680,
                },
            ),
            line(
                5.0,
                5.0,
                Record::Heard {
                    news: 1,
                    nation: "England".into(),
                    after_days: 2.0,
                },
            ),
            line(
                5.0,
                5.0,
                Record::Heard {
                    news: 2,
                    nation: "Spain".into(),
                    after_days: 4.0,
                },
            ),
        ]));
        assert_eq!(report.standing.by_deed["Captured"], -25.0);
        assert_eq!(report.standing.by_group["the Royal Navy"], -22.0);
        assert_eq!((report.economy.earned, report.economy.spent), (300, 120));
        assert_eq!(report.timelines.purse, [(3.0, 800.0), (4.0, 680.0)]);
        assert_eq!(report.word.mean_days(), Some(3.0));
        assert_eq!(report.word.slowest_days, 4.0);
    }
}
