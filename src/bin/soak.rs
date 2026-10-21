//! The world running on its own, headless and as fast as it will go, with
//! the full chronicle written to the logs folder: `cargo soak`, or
//! `cargo soak --days 200`. It opens as a new game does; the player's
//! sloop lies at Port Royal throughout, and everyone else goes about their
//! business. `cargo logbook` shows what came of it.

use std::process::ExitCode;
use std::time::Instant;

use bevy::ecs::schedule::SingleThreadedExecutor;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use false_colours::chronicle::{ChroniclePlugin, Run, log_layer};
use false_colours::clock::{DAY, WorldClock};
use false_colours::new_game::NewGamePlugin;
use false_colours::stepped_app;
use false_colours::time_scale::MAX_STEP;

/// Game days a soak runs, unless told otherwise.
const DAYS: f64 = 100.0;
/// How often it says how far it's got, game days.
const PROGRESS_DAYS: u64 = 10;

fn main() -> ExitCode {
    let days = match days(std::env::args().skip(1)) {
        Ok(days) => days,
        Err(error) => {
            eprintln!("{error}\nusage: cargo soak [--days N]");
            return ExitCode::FAILURE;
        }
    };
    let mut app = stepped_app();
    app.add_plugins((
        LogPlugin {
            custom_layer: log_layer,
            ..default()
        },
        NewGamePlugin,
        ChroniclePlugin { run: Run::Soak },
    ));
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
        "{days} days in {:.0} s. See them with `cargo logbook`.",
        began.elapsed().as_secs_f64()
    );
    ExitCode::SUCCESS
}

/// The days asked for: `--days N`, or [`DAYS`].
fn days(mut args: impl Iterator<Item = String>) -> Result<f64, String> {
    match (args.next().as_deref(), args.next(), args.next()) {
        (None, _, _) => Ok(DAYS),
        (Some("--days"), Some(days), None) => match days.parse::<f64>() {
            Ok(days) if days > 0.0 => Ok(days),
            _ => Err(format!("not a number of days: {days}")),
        },
        _ => Err("unexpected arguments".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(args: &[&str]) -> impl Iterator<Item = String> {
        args.iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn a_soak_runs_the_days_asked_for() {
        assert_eq!(days(args(&[])), Ok(DAYS));
        assert_eq!(days(args(&["--days", "250"])), Ok(250.0));
        assert!(days(args(&["--days", "none"])).is_err());
        assert!(days(args(&["--days", "-3"])).is_err());
        assert!(days(args(&["--days"])).is_err());
        assert!(days(args(&["--weeks", "3"])).is_err());
    }
}
