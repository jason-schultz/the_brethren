//! Where the game keeps its files, what a log is called, and how many logs
//! are kept.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fmt, fs, io};

use crate::Run;

/// The folder of logs, among the player's files...
pub const LOGS: &str = "logs";
/// ...and of crash reports, which are never pruned.
pub const CRASHES: &str = "crashes";
/// The most runs kept in the logs folder...
pub const RUNS_KEPT: usize = 20;
/// ...and the most room they take, bytes.
pub const RUNS_MAX_BYTES: u64 = 200 * 1024 * 1024;

/// `<name>` in the platform's folder for application data, or in the
/// working directory if there's none:
/// - macOS: `~/Library/Application Support/False Colours`
/// - Windows: `%APPDATA%\False Colours`
/// - Linux: `$XDG_DATA_HOME/false-colours`, or `~/.local/share/false-colours`
pub fn user_file(name: &str) -> PathBuf {
    static FOLDER: OnceLock<Option<PathBuf>> = OnceLock::new();
    let folder = FOLDER.get_or_init(|| {
        let data = data_folder()?;
        let [name, old] = FOLDER_NAMES;
        Some(moved_in(&data.join(name), &data.join(old)))
    });
    folder.clone().unwrap_or_default().join(name)
}

/// What the game's folder is called, and what it was called when the game
/// was The Brethren.
#[cfg(any(target_os = "macos", target_os = "windows"))]
const FOLDER_NAMES: [&str; 2] = ["False Colours", "The Brethren"];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const FOLDER_NAMES: [&str; 2] = ["false-colours", "the-brethren"];

/// `folder`, with the `old` one moved there first if only it exists, so a
/// player's saves, keys, and logs come along under the new name.
fn moved_in(folder: &Path, old: &Path) -> PathBuf {
    if !folder.exists()
        && old.exists()
        && let Err(error) = fs::rename(old, folder)
    {
        eprintln!(
            "Couldn't move {} to {}: {error}",
            old.display(),
            folder.display()
        );
    }
    folder.to_path_buf()
}

/// The platform's folder for application data.
#[cfg(target_os = "macos")]
fn data_folder() -> Option<PathBuf> {
    Some(PathBuf::from(env::var_os("HOME")?).join("Library/Application Support"))
}

#[cfg(target_os = "windows")]
fn data_folder() -> Option<PathBuf> {
    Some(PathBuf::from(env::var_os("APPDATA")?))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn data_folder() -> Option<PathBuf> {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(env::var_os("HOME")?).join(".local/share")))
}

/// Seconds since the Unix epoch, now.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// A moment on the calendar, UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Stamp {
    /// `secs` after the Unix epoch.
    pub fn at(secs: u64) -> Self {
        // Howard Hinnant's civil-from-days: the proleptic Gregorian
        // calendar in eras of 400 years, with years starting in March.
        let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let day_of_era = z.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let march_month = (5 * day_of_year + 2) / 153;
        let month = if march_month < 10 {
            march_month + 3
        } else {
            march_month - 9
        };
        Self {
            year: year_of_era + era * 400 + i64::from(month <= 2),
            month: month as u32,
            day: (day_of_year - (153 * march_month + 2) / 5 + 1) as u32,
            hour: (rest / 3_600) as u32,
            minute: (rest / 60 % 60) as u32,
            second: (rest % 60) as u32,
        }
    }
}

impl fmt::Display for Stamp {
    /// "2026-10-20 14:32 UTC".
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}-{:02}-{:02} {:02}:{:02} UTC",
            self.year, self.month, self.day, self.hour, self.minute
        )
    }
}

/// The name of the log of a run begun at `started`: names sort in the
/// order the runs began.
pub fn log_name(started: u64, run: Run) -> String {
    let at = Stamp::at(started);
    format!(
        "{}-{:02}-{:02}-{:02}{:02}{:02}-{}.jsonl",
        at.year,
        at.month,
        at.day,
        at.hour,
        at.minute,
        at.second,
        run.name()
    )
}

/// Deletes the oldest runs in `folder` beyond the newest `keep`, and then
/// more, oldest first, until the rest fit in `max_bytes`. A run is its log
/// and every file beside it named for it. Returns what was deleted.
pub fn prune(folder: &Path, keep: usize, max_bytes: u64) -> io::Result<Vec<PathBuf>> {
    let mut runs: BTreeMap<String, (bool, u64, Vec<PathBuf>)> = BTreeMap::new();
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((stem, extension)) = name.split_once('.') else {
            continue;
        };
        let run = runs.entry(stem.to_string()).or_default();
        run.0 |= extension == "jsonl";
        run.1 += fs::metadata(&path)?.len();
        run.2.push(path);
    }
    runs.retain(|_, (is_run, _, _)| *is_run);
    let sizes: Vec<u64> = runs.values().map(|(_, size, _)| *size).collect();
    let doomed = oldest_to_prune(&sizes, keep, max_bytes);
    let mut deleted = Vec::new();
    for (_, _, paths) in runs.into_values().take(doomed) {
        for path in paths {
            fs::remove_file(&path)?;
            deleted.push(path);
        }
    }
    Ok(deleted)
}

/// How many of the oldest runs to delete, of runs of `sizes` oldest first,
/// to keep at most `keep` and fit in `max_bytes`. The newest always stays.
fn oldest_to_prune(sizes: &[u64], keep: usize, max_bytes: u64) -> usize {
    let mut doomed = sizes.len().saturating_sub(keep.max(1));
    let mut total: u64 = sizes[doomed..].iter().sum();
    while total > max_bytes && doomed + 1 < sizes.len() {
        total -= sizes[doomed];
        doomed += 1;
    }
    doomed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_follow_the_calendar_through_leap_days() {
        let at = |year, month, day, hour, minute, second| Stamp {
            year,
            month,
            day,
            hour,
            minute,
            second,
        };
        assert_eq!(Stamp::at(0), at(1970, 1, 1, 0, 0, 0));
        assert_eq!(Stamp::at(951_782_400), at(2000, 2, 29, 0, 0, 0));
        assert_eq!(Stamp::at(1_700_000_000), at(2023, 11, 14, 22, 13, 20));
        assert_eq!(Stamp::at(1_704_067_199), at(2023, 12, 31, 23, 59, 59));
    }

    #[test]
    fn log_names_sort_in_the_order_runs_began() {
        assert_eq!(
            log_name(1_700_000_000, Run::Play),
            "2023-11-14-221320-play.jsonl"
        );
        let mut names = [1_700_000_000, 1_699_999_999, 1_000_000_000]
            .map(|started| log_name(started, Run::Soak));
        names.sort();
        assert_eq!(names[0], log_name(1_000_000_000, Run::Soak));
        assert_eq!(names[2], log_name(1_700_000_000, Run::Soak));
    }

    #[test]
    fn the_oldest_runs_go_first_and_the_newest_always_stays() {
        assert_eq!(oldest_to_prune(&[10, 10, 10], 5, 100), 0);
        assert_eq!(oldest_to_prune(&[10, 10, 10], 2, 100), 1);
        assert_eq!(oldest_to_prune(&[50, 10, 30, 30], 5, 60), 2);
        assert_eq!(oldest_to_prune(&[10, 500], 5, 100), 1);
        assert_eq!(oldest_to_prune(&[], 5, 100), 0);
    }

    #[test]
    fn the_old_folder_moves_to_the_new_name_once_and_never_over_it() {
        let root = env::temp_dir().join(format!("chronicle-move-{}", std::process::id()));
        let (old, new) = (root.join("The Brethren"), root.join("False Colours"));
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("quicksave.ron"), "save").unwrap();
        assert_eq!(moved_in(&new, &old), new);
        assert_eq!(
            fs::read_to_string(new.join("quicksave.ron")).unwrap(),
            "save"
        );
        assert!(!old.exists());

        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("quicksave.ron"), "older").unwrap();
        moved_in(&new, &old);
        assert_eq!(
            fs::read_to_string(new.join("quicksave.ron")).unwrap(),
            "save"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn pruning_deletes_a_run_with_its_side_files() {
        let folder = env::temp_dir().join(format!("chronicle-prune-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        for name in [
            "a-soak.jsonl",
            "a-soak.report.json",
            "b-soak.jsonl",
            "c-play.jsonl",
            "notes.txt",
        ] {
            fs::write(folder.join(name), "x").unwrap();
        }
        let mut deleted: Vec<String> = prune(&folder, 2, u64::MAX)
            .unwrap()
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        deleted.sort();
        assert_eq!(deleted, ["a-soak.jsonl", "a-soak.report.json"]);
        assert!(folder.join("notes.txt").exists());
        fs::remove_dir_all(&folder).unwrap();
    }
}
