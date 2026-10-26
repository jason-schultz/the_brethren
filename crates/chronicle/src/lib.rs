//! The chronicle of a run of False Colours: what happened, one JSON record
//! a line. The game writes it and the report reads it back, so the format
//! is defined here, once.
//!
//! Records name ships, ports, nations, and groups by name, never by
//! entity: entities change from load to load.

mod files;

pub use files::{CRASHES, LOGS, RUNS_KEPT, RUNS_MAX_BYTES, Stamp, log_name, now, prune, user_file};

use std::fmt;

use serde::{Deserialize, Serialize};

/// The version of the record format. A log of another can't be read.
pub const FORMAT: u32 = 1;

/// One line of a log: a record, and when it was written.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Line {
    /// Game days since the game opened.
    pub day: f64,
    /// Real seconds since the run began.
    pub real: f64,
    #[serde(flatten)]
    pub record: Record,
}

impl Line {
    /// The line as it's written, without the newline.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("records are plain data, with string keys")
    }
}

/// What kind of run a log is of.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Run {
    /// Someone playing.
    Play,
    /// The world running on its own, headless, as fast as it can.
    Soak,
}

impl Run {
    pub fn name(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Soak => "soak",
        }
    }
}

/// Something that happened.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Record {
    /// The first line of every log.
    Header {
        format: u32,
        /// The game's version.
        version: String,
        run: Run,
        /// When the run began, seconds since the Unix epoch.
        started: u64,
        /// The seed its dice were cast from, if it was given one: the same
        /// seed plays the same world again.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u64>,
    },
    /// The run ended cleanly.
    End,
    /// The game panicked: where, and what it said.
    Crash { message: String },
    /// A warning or error the game logged.
    Log { level: String, message: String },
    /// A playtester's note.
    Note { text: String },
    /// Something that looks broken.
    Alarm(Alarm),
    /// The player used a control, by key or button.
    Control { control: String },
    /// The player gave an order.
    Order { order: String },
    /// An alert raised for the player.
    Alert { alert: String },
    /// The player came into this port, or with `None`, put to sea.
    Dock { port: Option<String> },
    /// How fast time runs, game seconds a real second, and whether it's
    /// paused.
    Clock { speed: f64, paused: bool },
    /// The player's purse changed `by`, to `purse`.
    Money { by: i64, purse: u32 },
    /// Something went in the ledger of news.
    News(News),
    /// A nation heard of a piece of news, `after_days` after it happened.
    Heard {
        news: u64,
        nation: String,
        after_days: f64,
    },
    /// The player's standing with a group moved `by`, to `now`, as it
    /// heard of a deed of hers.
    Standing {
        group: String,
        by: f32,
        now: f32,
        deed: String,
    },
    /// Two nations came to new terms.
    Terms {
        nations: [String; 2],
        terms: String,
        value: f32,
    },
    /// A port went over from one nation to another.
    WentOver {
        port: String,
        from: String,
        to: String,
    },
    /// A battle ended.
    Battle(Battle),
    /// A hunter went after a ship she'd sighted.
    Chase(Chase),
    /// A ship went down.
    Lost(Lost),
    /// How things stand, once a game day.
    Snapshot(Box<Snapshot>),
}

/// Something that looks broken, raised once when it starts.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Alarm {
    pub alarm: AlarmKind,
    /// The ship, port, or nations it's about.
    pub subject: String,
    pub detail: String,
}

/// The kinds of thing that look broken.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AlarmKind {
    /// A ship with a route hasn't moved in a day.
    Stuck,
    /// A ship's position isn't a number.
    NotFinite,
    /// A port has less than nothing of a good.
    NegativeStock,
    /// A port has gone a month without a good it needs.
    Starved,
    /// Two nations have been at war for two months.
    LongWar,
    /// The player's standing with a group is at its bound.
    Pinned,
}

impl AlarmKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Stuck => "Ship stuck",
            Self::NotFinite => "Position not a number",
            Self::NegativeStock => "Negative stock",
            Self::Starved => "Port starved",
            Self::LongWar => "Long war",
            Self::Pinned => "Standing pinned",
        }
    }
}

/// A piece of news, as it went in the ledger.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct News {
    /// Its id in the ledger, which [`Record::Heard`] refers to.
    pub id: u64,
    pub happening: Happening,
    /// The kind of deed, for a deed.
    pub deed: Option<String>,
    /// The ship that sailed, did the deed, was lost, or told the account.
    pub ship: String,
    /// The ship it was done to or blamed on, if any.
    pub other: Option<String>,
    /// The group a deed touched.
    pub group: Option<String>,
    /// Whether it's the player's doing or telling.
    pub player: bool,
}

/// What kind of news it is.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Happening {
    Sailed,
    Overdue,
    Deed,
    Wrecked,
    Account,
}

/// What a ship is in the world.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Player,
    Merchant,
    Patrol,
    Raider,
    /// A prize sailed by her captor's prize crew.
    Prize,
    Other,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::Player => "player",
            Self::Merchant => "merchant",
            Self::Patrol => "patrol",
            Self::Raider => "raider",
            Self::Prize => "prize",
            Self::Other => "other",
        })
    }
}

/// A battle, from the first shot to the end.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Battle {
    /// The game day it began.
    pub began: f64,
    /// The ship that started it first.
    pub sides: [Side; 2],
    pub end: BattleEnd,
    /// The side that lost, unless they broke off.
    pub loser: Option<usize>,
}

/// How a battle ended, from least to most.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum BattleEnd {
    /// They drew apart, or one got into port.
    BrokeOff,
    /// One struck her colours, and wasn't taken.
    Struck,
    /// One was boarded and taken.
    Taken,
    /// One went down.
    Sunk,
}

impl BattleEnd {
    pub fn name(self) -> &'static str {
        match self {
            Self::BrokeOff => "broke off",
            Self::Struck => "struck",
            Self::Taken => "taken",
            Self::Sunk => "sunk",
        }
    }
}

/// One ship in a battle, at its start and its end.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Side {
    pub ship: String,
    pub class: String,
    pub role: Role,
    pub nation: Option<String>,
    pub guns: u32,
    /// Hands at the start and the end.
    pub crew: [u32; 2],
    /// Timbers at the start and the end, from 1 (sound) to 0.
    pub timbers: [f32; 2],
    /// Rigging at the start and the end, from 1 (sound) to 0.
    pub rigging: [f32; 2],
    /// Where she was when it began.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub began_at: Option<Place>,
}

/// Where a ship was: meters east and north of the map's origin, and the
/// nearest port, kilometers off.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Place {
    pub at: [f32; 2],
    pub port: String,
    pub km: f32,
}

/// A hunter going after a ship she'd sighted, `km` off.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Chase {
    pub hunter: Spotted,
    pub quarry: Spotted,
    pub km: f32,
}

/// A ship going down: how, and what was left of her crew and hull.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Lost {
    pub ship: Spotted,
    /// "sunk", "taken", "grounding", "reef", or "foundered".
    pub cause: String,
    pub crew: u32,
    pub timbers: f32,
}

/// A ship in a chase or lost, and where she was.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Spotted {
    pub ship: String,
    pub role: Role,
    pub nation: Option<String>,
    pub place: Place,
}

/// How things stand.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    /// The player's ship, if she has one.
    pub player: Option<Captain>,
    /// Every pair of nations.
    pub relations: Vec<Pair>,
    /// Ships at sea or in port, by nation and role.
    pub fleets: Vec<Fleet>,
    pub ports: Vec<PortState>,
    /// Contracts posted and not yet taken.
    pub open_jobs: u32,
    /// Pieces of news in the ledger.
    pub news: u32,
    /// Entities in the world, of every kind.
    pub entities: u32,
    /// The game's memory, kilobytes, where the platform says.
    pub memory_kb: Option<u64>,
}

/// The player, and how she stands.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Captain {
    pub ship: String,
    pub class: String,
    pub purse: u32,
    pub timbers: f32,
    pub rigging: f32,
    pub crew: u32,
    pub morale: f32,
    /// Tons of cargo aboard, hers and consigned.
    pub cargo_tons: f64,
    /// The port she's in, if any.
    pub docked: Option<String>,
    pub colours: Option<String>,
    /// With every group of every nation.
    pub standing: Vec<GroupStanding>,
    /// The nations that want her.
    pub wanted: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GroupStanding {
    pub group: String,
    pub value: f32,
}

/// How two nations stand, from -100 to 100.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Pair {
    pub nations: [String; 2],
    pub value: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Fleet {
    pub nation: Option<String>,
    pub role: Role,
    pub ships: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PortState {
    pub port: String,
    pub flag: String,
    pub goods: Vec<GoodState>,
}

/// A good in a port's market.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GoodState {
    pub good: String,
    pub stock: f64,
    pub target: f64,
    /// Pieces of eight a unit, at this stock.
    pub price: f64,
    /// Whether the port uses it up rather than making it.
    pub needed: bool,
    /// Stock it makes a day, or uses up if less than nothing.
    #[serde(default)]
    pub per_day: f64,
    /// Stock convoys from home land a day, at most.
    #[serde(default)]
    pub convoys: f64,
}

/// A log, read back.
#[derive(Debug, Clone, PartialEq)]
pub struct Log {
    /// The run, from the header.
    pub run: Run,
    pub version: String,
    /// When it began, seconds since the Unix epoch.
    pub started: u64,
    /// The seed its dice were cast from, if it was given one.
    pub seed: Option<u64>,
    /// Every line after the header.
    pub lines: Vec<Line>,
    /// Lines that couldn't be read: a crash's cut-off last line, or a
    /// record from a newer game. Their line numbers, from 1, and why.
    pub skipped: Vec<(usize, String)>,
}

/// Why a log couldn't be read at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// The first line isn't a header.
    NoHeader,
    /// It's in another version of the format.
    Format(u32),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NoHeader => f.write_str("the log has no header"),
            Self::Format(found) => {
                write!(f, "the log is in format {found}, and this reads {FORMAT}")
            }
        }
    }
}

impl std::error::Error for ReadError {}

/// Reads a log. A line that can't be read is skipped and noted, so a log
/// cut off by a crash still reads.
pub fn read(text: &str) -> Result<Log, ReadError> {
    let mut lines = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty());
    let header = lines
        .next()
        .and_then(|(_, line)| serde_json::from_str::<Line>(line).ok());
    let Some(Line {
        record:
            Record::Header {
                format,
                version,
                run,
                started,
                seed,
            },
        ..
    }) = header
    else {
        return Err(ReadError::NoHeader);
    };
    if format != FORMAT {
        return Err(ReadError::Format(format));
    }
    let mut log = Log {
        run,
        version,
        started,
        seed,
        lines: Vec::new(),
        skipped: Vec::new(),
    };
    for (index, line) in lines {
        match serde_json::from_str::<Line>(line) {
            Ok(line) => log.lines.push(line),
            Err(error) => log.skipped.push((index + 1, error.to_string())),
        }
    }
    Ok(log)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(format: u32) -> Line {
        Line {
            day: 0.0,
            real: 0.0,
            record: Record::Header {
                format,
                version: "0.1.0".into(),
                run: Run::Soak,
                started: 1_700_000_000,
                seed: Some(7),
            },
        }
    }

    fn note(day: f64, text: &str) -> Line {
        Line {
            day,
            real: day * 10.0,
            record: Record::Note { text: text.into() },
        }
    }

    fn log_of(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|line| line.to_json() + "\n")
            .collect::<String>()
    }

    #[test]
    fn a_log_reads_back_as_it_was_written() {
        let side = Side {
            ship: "Ranger".into(),
            class: "Sloop".into(),
            role: Role::Raider,
            nation: Some("pirates".into()),
            guns: 8,
            crew: [30, 24],
            timbers: [1.0, 0.8],
            rigging: [1.0, 0.6],
            began_at: Some(Place {
                at: [-1_200.0, 3_400.0],
                port: "Kingston".into(),
                km: 3.6,
            }),
        };
        let battle = Line {
            day: 2.5,
            real: 30.0,
            record: Record::Battle(Battle {
                began: 2.4,
                sides: [side.clone(), side],
                end: BattleEnd::Taken,
                loser: Some(1),
            }),
        };
        let spotted = |ship: &str, role| Spotted {
            ship: ship.into(),
            role,
            nation: None,
            place: Place {
                at: [0.0, 0.0],
                port: "Port Royal".into(),
                km: 1.5,
            },
        };
        let chase = Line {
            day: 2.3,
            real: 29.0,
            record: Record::Chase(Chase {
                hunter: spotted("Ranger", Role::Raider),
                quarry: spotted("Dove", Role::Merchant),
                km: 6.2,
            }),
        };
        let lost = Line {
            day: 2.6,
            real: 31.0,
            record: Record::Lost(Lost {
                ship: spotted("Dove", Role::Merchant),
                cause: "foundered".into(),
                crew: 2,
                timbers: 0.0,
            }),
        };
        let lines = [
            header(FORMAT),
            note(1.0, "sailing is slow"),
            chase,
            battle,
            lost,
        ];
        let log = read(&log_of(&lines)).unwrap();
        assert_eq!(log.run, Run::Soak);
        assert_eq!(log.seed, Some(7));
        assert_eq!(log.lines, lines[1..]);
        assert!(log.skipped.is_empty());
    }

    #[test]
    fn a_log_from_before_seeds_reads_without_one() {
        let text = format!(
            "{{\"day\":0.0,\"real\":0.0,\"kind\":\"header\",\"format\":{FORMAT},\
             \"version\":\"0.1.0\",\"run\":\"play\",\"started\":1}}\n"
        );
        assert_eq!(read(&text).unwrap().seed, None);
    }

    #[test]
    fn a_cut_off_or_unknown_line_is_skipped_and_the_rest_read() {
        let text = log_of(&[header(FORMAT), note(1.0, "one")])
            + "{\"day\":2.0,\"real\":1.0,\"kind\":\"from_the_future\"}\n"
            + &note(3.0, "three").to_json()
            + "\n{\"day\":4.0,\"re";
        let log = read(&text).unwrap();
        assert_eq!(log.lines, [note(1.0, "one"), note(3.0, "three")]);
        assert_eq!(
            log.skipped
                .iter()
                .map(|(line, _)| *line)
                .collect::<Vec<_>>(),
            [3, 5]
        );
    }

    #[test]
    fn a_log_without_a_header_or_in_another_format_is_refused() {
        assert_eq!(read(&log_of(&[note(1.0, "one")])), Err(ReadError::NoHeader));
        assert_eq!(read(""), Err(ReadError::NoHeader));
        assert_eq!(
            read(&log_of(&[header(FORMAT + 1)])),
            Err(ReadError::Format(FORMAT + 1))
        );
    }
}
