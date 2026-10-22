//! The chronicle of a run: what happened, written down as it happens, for
//! playtests, soaks, and crash reports. The records themselves are defined
//! in the `chronicle` crate, which the report tool reads them back with.
//!
//! Every build keeps the latest few hundred records in memory, and a panic
//! writes them into a crash report beside the backtrace (see [`crash`]).
//! Those are the cheap records: the warnings and errors the game logs, and
//! the player's controls, orders, and alerts, where she docks, and how fast
//! time runs. With the `chronicle` feature a build also writes the whole run
//! to a log in the logs folder, and notes much more: news, battles,
//! standing, money, a snapshot a day, a playtester's notes, and alarms when
//! something looks broken.
//!
//! The chronicle only reads the world. Systems note what happened in the
//! [`Chronicle`], and at the end of each frame it's written out.

#[cfg(feature = "chronicle")]
mod alarms;
#[cfg(feature = "chronicle")]
mod battles;
mod crash;
mod session;
#[cfg(feature = "chronicle")]
mod world;

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use bevy::prelude::*;
use chronicle::{FORMAT, Line, Record};

use crate::clock::{DAY, WorldClock};

pub use chronicle::Run;
pub use crash::log_layer;

/// How many of the latest records a crash report shows.
const RECENT: usize = 300;

/// Records noted this frame, written out at the end of it.
#[derive(Resource, Debug, Default)]
pub struct Chronicle(pub Vec<Record>);

impl Chronicle {
    pub fn note(&mut self, record: Record) {
        self.0.push(record);
    }
}

/// A playtester's note, typed in the game.
#[cfg(feature = "chronicle")]
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct Note(pub String);

/// The systems that note records, which run before they're written out.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Noting;

pub struct ChroniclePlugin {
    pub run: Run,
    /// The seed the world's dice were cast from, if it was given one.
    pub seed: Option<u64>,
}

impl Plugin for ChroniclePlugin {
    fn build(&self, app: &mut App) {
        begin(self.run, self.seed);
        crash::install();
        noting(app, self.run);
        app.add_systems(Last, write_out.after(Noting));
    }
}

/// Everything that notes records, without writing them anywhere: what
/// tests run.
pub(crate) fn noting(app: &mut App, run: Run) {
    app.init_resource::<Chronicle>();
    session::plugin(app, run);
    #[cfg(feature = "chronicle")]
    {
        app.add_message::<Note>();
        world::plugin(app);
        battles::plugin(app);
        alarms::plugin(app);
    }
}

/// Where records go once written out. It's kept globally rather than as a
/// resource because the panic hook and the log layer need it too, and
/// they have no world to look in.
static SINK: LazyLock<Mutex<Sink>> = LazyLock::new(|| {
    let started = chronicle::now();
    Mutex::new(Sink {
        began: Instant::now(),
        started,
        name: chronicle::log_name(started, Run::Play, 0),
        run: Run::Play,
        day: 0.0,
        recent: VecDeque::new(),
        file: None,
    })
});

struct Sink {
    began: Instant,
    /// When the run began, seconds since the Unix epoch.
    started: u64,
    /// The name of the run's log, which names its other files too.
    name: String,
    run: Run,
    /// The game day of the latest record.
    day: f64,
    /// The latest records, oldest first, as written.
    recent: VecDeque<String>,
    /// The log of the whole run, with the `chronicle` feature.
    file: Option<BufWriter<File>>,
}

impl Sink {
    fn write(&mut self, day: f64, record: Record) {
        self.day = day;
        let line = Line {
            day,
            real: self.began.elapsed().as_secs_f64(),
            record,
        }
        .to_json();
        if let Some(file) = &mut self.file
            && let Err(error) = writeln!(file, "{line}")
        {
            self.file = None;
            warn!("couldn't write the log, and stopped: {error}");
        }
        self.recent.push_back(line);
        if self.recent.len() > RECENT {
            self.recent.pop_front();
        }
    }

    fn flush(&mut self) {
        if let Some(file) = &mut self.file
            && let Err(error) = file.flush()
        {
            self.file = None;
            warn!("couldn't write the log, and stopped: {error}");
        }
    }

    /// The name of the run's files, without an extension.
    fn stem(&self) -> &str {
        self.name.trim_end_matches(".jsonl")
    }
}

fn sink() -> MutexGuard<'static, Sink> {
    // A panic while writing leaves the sink as it was: still worth writing to.
    SINK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts the run: opens its log, with the `chronicle` feature, and heads
/// it.
fn begin(run: Run, seed: Option<u64>) {
    let mut sink = sink();
    sink.run = run;
    sink.began = Instant::now();
    sink.started = chronicle::now();
    sink.name = chronicle::log_name(sink.started, run, 0);
    #[cfg(feature = "chronicle")]
    if let Some((name, file)) = world::open_log(sink.started, run) {
        sink.name = name;
        sink.file = Some(file);
    }
    let header = Record::Header {
        format: FORMAT,
        version: env!("CARGO_PKG_VERSION").to_string(),
        run,
        started: sink.started,
        seed,
    };
    sink.write(0.0, header);
    sink.flush();
}

/// Writes out the frame's records, and ends the run if the app is exiting.
/// Every frame's are flushed, so a crash loses none.
fn write_out(
    mut chronicle: ResMut<Chronicle>,
    clock: Res<WorldClock>,
    mut exits: MessageReader<AppExit>,
) {
    let ended = exits.read().next().is_some();
    if chronicle.0.is_empty() && !ended {
        return;
    }
    let day = clock.elapsed / DAY;
    let mut sink = sink();
    for record in chronicle.0.drain(..) {
        sink.write(day, record);
    }
    if ended {
        sink.write(day, Record::End);
    }
    sink.flush();
}

/// A ship's markers, which say what she is.
#[cfg(feature = "chronicle")]
type Roles = (
    Has<crate::player::Player>,
    Has<crate::merchants::Merchant>,
    Has<crate::patrols::Patrol>,
    Has<crate::raiders::Raider>,
    Has<crate::prizes::Prize>,
);

/// What a ship is, by her markers. A prize is a prize first, whatever she
/// was before.
#[cfg(feature = "chronicle")]
fn role(
    (player, merchant, patrol, raider, prize): (bool, bool, bool, bool, bool),
) -> chronicle::Role {
    use chronicle::Role;
    if player {
        Role::Player
    } else if prize {
        Role::Prize
    } else if merchant {
        Role::Merchant
    } else if patrol {
        Role::Patrol
    } else if raider {
        Role::Raider
    } else {
        Role::Other
    }
}

/// A run condition: whether a game day has begun since it was last asked,
/// and the first time it's asked.
#[cfg(feature = "chronicle")]
fn new_day(clock: Res<WorldClock>, mut last: Local<Option<u64>>) -> bool {
    let day = (clock.elapsed / DAY).floor() as u64;
    let new = *last != Some(day);
    *last = Some(day);
    new
}

/// The name of an entity, for a record.
fn name_of(names: &Query<&Name>, entity: Entity) -> String {
    names
        .get(entity)
        .map_or_else(|_| "an unknown ship".to_string(), |name| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sim_app;

    /// A headless app noting records, as a run of `run` would.
    pub(super) fn noting_app(run: Run) -> App {
        let mut app = sim_app();
        noting(&mut app, run);
        app
    }

    /// Takes the records noted so far.
    pub(super) fn noted(app: &mut App) -> Vec<Record> {
        std::mem::take(&mut app.world_mut().resource_mut::<Chronicle>().0)
    }

    #[test]
    fn the_sink_keeps_only_the_latest_records() {
        let mut sink = Sink {
            began: Instant::now(),
            started: 0,
            name: chronicle::log_name(0, Run::Soak, 0),
            run: Run::Soak,
            day: 0.0,
            recent: VecDeque::new(),
            file: None,
        };
        for n in 0..RECENT + 5 {
            sink.write(
                n as f64,
                Record::Note {
                    text: n.to_string(),
                },
            );
        }
        assert_eq!(sink.recent.len(), RECENT);
        assert!(sink.recent[0].contains("\"text\":\"5\""));
        assert_eq!(sink.day, (RECENT + 4) as f64);
        assert_eq!(sink.stem(), "1970-01-01-000000-soak");
    }
}
