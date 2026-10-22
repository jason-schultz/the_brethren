//! The world running on its own, headless and as fast as it will go, with
//! the full chronicle written to the logs folder: `cargo soak`, or
//! `cargo soak --days 200 --seed 42`. It opens as a new game does; the
//! player's sloop lies at Port Royal throughout, and everyone else goes
//! about their business. `cargo logbook` shows what came of it.
//!
//! Each soak casts the world's dice from a seed of its own, so no two run
//! alike; the log keeps the seed, and `--seed` plays a run again.

use std::process::ExitCode;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bevy::ecs::schedule::SingleThreadedExecutor;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use false_colours::chronicle::{ChroniclePlugin, Run, log_layer};
use false_colours::clock::{DAY, WorldClock};
use false_colours::combat::rounds::Dice;
use false_colours::new_game::NewGamePlugin;
use false_colours::stepped_app;
use false_colours::time_scale::MAX_STEP;

/// Game days a soak runs, unless told otherwise.
const DAYS: f64 = 100.0;
/// How often it says how far it's got, game days.
const PROGRESS_DAYS: u64 = 10;

fn main() -> ExitCode {
    let Args { days, seed } = match args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}\nusage: cargo soak [--days N] [--seed N]");
            return ExitCode::FAILURE;
        }
    };
    let seed = seed.unwrap_or_else(fresh_seed);
    let mut app = stepped_app();
    app.add_plugins((
        LogPlugin {
            custom_layer: log_layer,
            ..default()
        },
        NewGamePlugin,
        ChroniclePlugin {
            run: Run::Soak,
            seed: Some(seed),
        },
    ));
    app.insert_resource(Dice::seeded(seed));
    // A handful of ships is too little work to share among threads: waking
    // them costs more than it saves.
    for (_, schedule) in app.world_mut().resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
    // Every step as long as full time compression makes it.
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .set_timestep_seconds(MAX_STEP);
    let began = Instant::now();
    let mut reported = 0;
    loop {
        app.update();
        let day = app.world().resource::<WorldClock>().elapsed / DAY;
        if day >= days {
            break;
        }
        if day as u64 >= reported + PROGRESS_DAYS {
            reported = day as u64;
            println!("day {reported}, {:.0} s", began.elapsed().as_secs_f64());
        }
    }
    app.world_mut().write_message(AppExit::Success);
    app.update();
    println!(
        "{days} days, seed {seed}, in {:.0} s. See them with `cargo logbook`.",
        began.elapsed().as_secs_f64()
    );
    ExitCode::SUCCESS
}

/// What a soak was asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Args {
    days: f64,
    /// `None` for a fresh one.
    seed: Option<u64>,
}

/// `--days N` (or [`DAYS`]) and `--seed N`, in either order.
fn args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut asked = Args {
        days: DAYS,
        seed: None,
    };
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} wants a value"))?;
        match flag.as_str() {
            "--days" => match value.parse::<f64>() {
                Ok(days) if days > 0.0 => asked.days = days,
                _ => return Err(format!("not a number of days: {value}")),
            },
            "--seed" => {
                let seed = value.parse().map_err(|_| format!("not a seed: {value}"))?;
                asked.seed = Some(seed);
            }
            _ => return Err(format!("unexpected argument: {flag}")),
        }
    }
    Ok(asked)
}

/// A seed no soak has likely used: the time now, to the nanosecond.
fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn given(args: &[&str]) -> impl Iterator<Item = String> {
        args.iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn a_soak_runs_the_days_and_seed_asked_for() {
        let asked = |days, seed| Ok(Args { days, seed });
        assert_eq!(args(given(&[])), asked(DAYS, None));
        assert_eq!(args(given(&["--days", "250"])), asked(250.0, None));
        assert_eq!(
            args(given(&["--seed", "42", "--days", "30"])),
            asked(30.0, Some(42))
        );
        assert!(args(given(&["--days", "none"])).is_err());
        assert!(args(given(&["--days", "-3"])).is_err());
        assert!(args(given(&["--days"])).is_err());
        assert!(args(given(&["--seed", "-1"])).is_err());
        assert!(args(given(&["--weeks", "3"])).is_err());
    }
}
