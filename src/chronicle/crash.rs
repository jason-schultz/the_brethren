//! Crash reports, and the warnings and errors the game logs.
//!
//! A panic writes a report to the crashes folder: what panicked and where,
//! the backtrace, and the latest records of the run, with a copy of the
//! quicksave beside it. Crash reports are kept in every build, and never
//! pruned.

use std::backtrace::Backtrace;
use std::collections::HashMap;
use std::fmt::{self, Write as _};
use std::panic::{self, PanicHookInfo};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError, TryLockError};
use std::time::Duration;
use std::{fs, io, thread};

use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Subscriber};
use bevy::log::tracing_subscriber::Layer;
use bevy::log::tracing_subscriber::layer::Context;
use bevy::log::{BoxedLayer, Level};
use bevy::prelude::*;
use chronicle::{CRASHES, Record, Stamp, user_file};

use super::{Chronicle, SINK, Sink};
use crate::save::quicksave::save_path;

/// How often the same warning is noted before the rest are let go.
const REPEATS: u32 = 5;
/// How long a panic waits for the sink, if another thread is writing.
const SINK_WAIT: Duration = Duration::from_millis(500);

/// Warnings and errors logged since the chronicle last looked, as (level,
/// message). The log layer has no world, so they wait here.
static LOGGED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Has every panic write a crash report before it's reported as usual.
pub(super) fn install() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        report(info);
        previous(info);
    }));
}

fn report(info: &PanicHookInfo) {
    let message = info.to_string();
    let backtrace = Backtrace::force_capture();
    let Some(mut sink) = wait_for_sink() else {
        eprintln!("False Colours crashed, and couldn't write a crash report.");
        return;
    };
    let day = sink.day;
    sink.write(
        day,
        Record::Crash {
            message: message.clone(),
        },
    );
    sink.flush();
    let text = crash_report(&sink, &message, &backtrace);
    match write_report(&sink.stem(), &text) {
        Ok(path) => eprintln!("False Colours crashed: see {}", path.display()),
        Err(error) => eprintln!("False Colours crashed, and couldn't write a report: {error}"),
    }
}

/// The sink, unless this thread panicked while writing to it, or another
/// is too long about it.
fn wait_for_sink() -> Option<std::sync::MutexGuard<'static, Sink>> {
    let started = std::time::Instant::now();
    loop {
        match SINK.try_lock() {
            Ok(sink) => return Some(sink),
            Err(TryLockError::Poisoned(poisoned)) => return Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) if started.elapsed() < SINK_WAIT => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(TryLockError::WouldBlock) => return None,
        }
    }
}

fn crash_report(sink: &Sink, message: &str, backtrace: &impl fmt::Display) -> String {
    let mut text = format!(
        "False Colours {} crashed on game day {:.2}, {} run begun {}.\n\n{message}\n\n",
        env!("CARGO_PKG_VERSION"),
        sink.day,
        sink.run.name(),
        Stamp::at(sink.started),
    );
    let _ = write!(text, "Backtrace:\n{backtrace}\n\n");
    let _ = writeln!(
        text,
        "The last {} records, oldest first:",
        sink.recent.len()
    );
    for line in &sink.recent {
        let _ = writeln!(text, "{line}");
    }
    text
}

/// Writes the report, and a copy of the quicksave if there is one, to the
/// crashes folder, named for the run. Returns the report's path.
fn write_report(stem: &str, text: &str) -> io::Result<PathBuf> {
    let folder = user_file(CRASHES);
    fs::create_dir_all(&folder)?;
    let path = folder.join(format!("{stem}.txt"));
    fs::write(&path, text)?;
    let save = save_path();
    if save.exists() {
        fs::copy(save, folder.join(format!("{stem}-quicksave.ron")))?;
    }
    Ok(path)
}

/// The layer that hands the game's warnings and errors to the chronicle.
/// Give it to Bevy's `LogPlugin` as its `custom_layer`.
pub fn log_layer(_: &mut App) -> Option<BoxedLayer> {
    Some(Box::new(Warnings))
}

struct Warnings;

impl<S: Subscriber> Layer<S> for Warnings {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let level = *event.metadata().level();
        // More verbose levels are greater: this lets through warnings and
        // errors.
        if level > Level::WARN {
            return;
        }
        let mut message = Message::default();
        event.record(&mut message);
        LOGGED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((level.to_string(), message.0));
    }
}

/// An event's message, and any other fields after it.
#[derive(Default)]
struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.0, "{value:?}");
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }
}

/// Notes what's been logged. A warning repeated every frame is noted only
/// the first few times.
pub(super) fn note_logs(mut chronicle: ResMut<Chronicle>, mut seen: Local<HashMap<String, u32>>) {
    let logged = std::mem::take(&mut *LOGGED.lock().unwrap_or_else(PoisonError::into_inner));
    for (level, message) in logged {
        let times = seen.entry(message.clone()).or_default();
        *times += 1;
        if let Some(message) = repeated(message, *times) {
            chronicle.note(Record::Log { level, message });
        }
    }
}

/// The message to note the `times`th time it's logged, if it's to be noted.
fn repeated(message: String, times: u32) -> Option<String> {
    match times {
        times if times < REPEATS => Some(message),
        REPEATS => Some(format!("{message} (repeated; no more of these are noted)")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::Instant;

    use chronicle::Run;

    #[test]
    fn a_warning_repeated_every_frame_is_noted_only_the_first_few_times() {
        let noted: Vec<String> = (1..=REPEATS + 3)
            .filter_map(|times| repeated("no wind".to_string(), times))
            .collect();
        assert_eq!(noted.len() as u32, REPEATS);
        assert!(noted.last().unwrap().contains("no more"));
    }

    #[test]
    fn a_crash_report_says_what_panicked_and_what_came_before() {
        let mut sink = Sink {
            began: Instant::now(),
            started: 1_700_000_000,
            run: Run::Play,
            day: 0.0,
            recent: VecDeque::new(),
            file: None,
        };
        sink.write(
            3.5,
            Record::Order {
                order: "Buy 10 sugar".into(),
            },
        );
        let text = crash_report(&sink, "panicked at src/x.rs:1:1:\nno ship", &"frames");
        assert!(text.contains("crashed on game day 3.50, play run begun 2023-11-14 22:13 UTC"));
        assert!(text.contains("no ship"));
        assert!(text.contains("Backtrace:\nframes"));
        assert!(text.contains("Buy 10 sugar"));
    }
}
