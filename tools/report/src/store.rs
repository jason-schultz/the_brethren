//! The runs and crash reports on disk, and each run's report, folded once
//! and kept beside its log until the log changes.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tuning::Entry;

use crate::fold::{REPORT_FORMAT, Report, fold};

/// Where the logs and crash reports are.
#[derive(Debug, Clone)]
pub struct Store {
    pub logs: PathBuf,
    pub crashes: PathBuf,
    /// The game's data folder, with its knobs and balance targets.
    pub data: PathBuf,
}

/// A report as it's cached, with what it was folded from.
#[derive(Serialize, Deserialize)]
struct Cached {
    format: u32,
    /// The log's size, bytes, and when it was last written.
    size: u64,
    modified: u64,
    report: Report,
}

/// The names of the files in `folder` ending `extension`, without it,
/// newest first: names begin with when they were written.
fn stems(folder: &Path, extension: &str) -> Vec<String> {
    let mut stems: Vec<String> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()?
                .strip_suffix(extension)
                .map(str::to_string)
        })
        .collect();
    stems.sort_by(|a, b| b.cmp(a));
    stems
}

/// A log's size and when it was last written, seconds since the epoch.
fn stamp(path: &Path) -> Option<(u64, u64)> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((metadata.len(), modified.as_secs()))
}

impl Store {
    /// The entries of one of the game's tuning files: its knobs, or its
    /// balance targets.
    pub fn tuning<T: DeserializeOwned + Entry>(&self, file: &str) -> Result<Vec<T>, String> {
        let path = self.data.join(file);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("couldn't read {}: {error}", path.display()))?;
        tuning::parse(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Writes `entries` to one of the game's tuning files, if they make
    /// sense.
    pub fn save_tuning<T: Serialize + Entry>(
        &self,
        file: &str,
        entries: &[T],
    ) -> Result<(), String> {
        tuning::check(entries)?;
        let path = self.data.join(file);
        fs::write(&path, tuning::write(entries)?)
            .map_err(|error| format!("couldn't write {}: {error}", path.display()))
    }

    /// The runs in the logs folder, newest first.
    pub fn runs(&self) -> Vec<String> {
        stems(&self.logs, ".jsonl")
    }

    /// The crash reports, newest first, by the run each ended.
    pub fn crashes(&self) -> Vec<String> {
        stems(&self.crashes, ".txt")
    }

    /// Run `id`'s report, from the cache if its log hasn't changed. Only a
    /// run in the folder can be asked for, so no id reaches another file.
    pub fn report(&self, id: &str) -> Result<Report, String> {
        if !self.runs().iter().any(|run| run == id) {
            return Err(format!("there's no run {id}"));
        }
        let log = self.logs.join(format!("{id}.jsonl"));
        let cache = self.logs.join(format!("{id}.report.json"));
        let (size, modified) = stamp(&log).ok_or_else(|| format!("can't read run {id}"))?;
        let cached = fs::read_to_string(&cache)
            .ok()
            .and_then(|text| serde_json::from_str::<Cached>(&text).ok())
            .filter(|cached| {
                (cached.format, cached.size, cached.modified) == (REPORT_FORMAT, size, modified)
            });
        if let Some(cached) = cached {
            return Ok(cached.report);
        }
        let text = fs::read_to_string(&log).map_err(|error| error.to_string())?;
        let report = fold(&chronicle::read(&text).map_err(|error| error.to_string())?);
        let cached = Cached {
            format: REPORT_FORMAT,
            size,
            modified,
            report,
        };
        // A run still being written changes under us; the next look folds
        // it again. A cache that can't be written only costs time.
        if let Ok(json) = serde_json::to_string(&cached) {
            let _ = fs::write(cache, json);
        }
        Ok(cached.report)
    }

    /// The run of the same kind before `id`, if there's one.
    pub fn previous(&self, id: &str, report: &Report) -> Option<String> {
        self.runs()
            .into_iter()
            .filter(|run| run.as_str() < id)
            .find(|run| run.ends_with(&format!("-{}", report.run.name())))
    }

    /// Crash report `id`, with the same care as [`Self::report`].
    pub fn crash(&self, id: &str) -> Option<String> {
        if !self.crashes().iter().any(|crash| crash == id) {
            return None;
        }
        fs::read_to_string(self.crashes.join(format!("{id}.txt"))).ok()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use chronicle::{FORMAT, Line, Record, Run};

    /// A store in a fresh folder of its own, with a log for each of
    /// `runs`, days long: (id, days).
    pub fn store(name: &str, runs: &[(&str, f64)]) -> Store {
        let folder = std::env::temp_dir().join(format!("report-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let store = Store {
            logs: folder.join("logs"),
            crashes: folder.join("crashes"),
            data: folder.join("data"),
        };
        fs::create_dir_all(&store.logs).unwrap();
        fs::create_dir_all(&store.crashes).unwrap();
        for &(id, days) in runs {
            let run = if id.ends_with("-play") {
                Run::Play
            } else {
                Run::Soak
            };
            let lines = [
                Record::Header {
                    format: FORMAT,
                    version: "0.1.0".into(),
                    run,
                    started: 1_700_000_000,
                    seed: None,
                },
                Record::End,
            ]
            .map(|record| {
                Line {
                    day: days,
                    real: 1.0,
                    record,
                }
                .to_json()
            });
            fs::write(store.logs.join(format!("{id}.jsonl")), lines.join("\n")).unwrap();
        }
        store
    }

    #[test]
    fn runs_are_listed_newest_first_and_only_they_can_be_read() {
        let store = store(
            "listing",
            &[("2023-01-01-soak", 3.0), ("2023-02-01-play", 5.0)],
        );
        fs::write(store.logs.join("notes.txt"), "not a run").unwrap();
        assert_eq!(store.runs(), ["2023-02-01-play", "2023-01-01-soak"]);
        assert_eq!(store.report("2023-01-01-soak").unwrap().days, 3.0);
        assert!(store.report("../logs/2023-01-01-soak").is_err());
        assert!(store.report("notes").is_err());
    }

    #[test]
    fn a_report_is_kept_until_its_log_changes() {
        let store = store("cache", &[("2023-01-01-soak", 3.0)]);
        store.report("2023-01-01-soak").unwrap();
        assert!(store.logs.join("2023-01-01-soak.report.json").exists());
        let log = store.logs.join("2023-01-01-soak.jsonl");
        let longer = fs::read_to_string(&log).unwrap()
            + "\n"
            + &Line {
                day: 9.0,
                real: 2.0,
                record: Record::End,
            }
            .to_json();
        fs::write(&log, longer).unwrap();
        assert_eq!(store.report("2023-01-01-soak").unwrap().days, 9.0);
    }

    #[test]
    fn the_previous_run_is_the_last_one_of_the_same_kind() {
        let store = store(
            "previous",
            &[
                ("2023-01-01-soak", 1.0),
                ("2023-01-02-play", 1.0),
                ("2023-01-03-soak", 1.0),
            ],
        );
        let report = store.report("2023-01-03-soak").unwrap();
        assert_eq!(
            store.previous("2023-01-03-soak", &report).as_deref(),
            Some("2023-01-01-soak")
        );
        let first = store.report("2023-01-01-soak").unwrap();
        assert_eq!(store.previous("2023-01-01-soak", &first), None);
    }
}
