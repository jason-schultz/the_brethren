//! The report: serves the chronicles of runs of False Colours as web pages,
//! on this machine only. Every run, each in full, two compared, the trend
//! over runs, and crash reports.
//!
//! `cargo logbook` serves the game's own logs folder at
//! <http://127.0.0.1:7716>. `cargo logbook --summary` prints the newest
//! run's summary instead, and `--summary <run>` another's.
//! `--folder <dir>` reads the logs and crashes in `dir`.

mod chart;
mod fold;
mod headline;
mod pages;
mod server;
mod store;
mod summary;

use std::path::PathBuf;
use std::process::ExitCode;

use chronicle::{CRASHES, LOGS, user_file};
use tokio::net::TcpListener;

use crate::store::Store;

/// The port it serves on, out of the way of the usual ones.
const PORT: u16 = 7716;

/// What it was asked to do.
#[derive(Debug, PartialEq)]
struct Args {
    store: Option<PathBuf>,
    port: u16,
    /// Print a summary instead of serving: the newest run's, or this one's.
    summary: Option<Option<String>>,
}

fn args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        store: None,
        port: PORT,
        summary: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--folder" => parsed.store = Some(args.next().ok_or("--folder needs a folder")?.into()),
            "--port" => {
                let port = args.next().ok_or("--port needs a number")?;
                parsed.port = port.parse().map_err(|_| format!("not a port: {port}"))?;
            }
            "--summary" => parsed.summary = Some(None),
            id if parsed.summary == Some(None) && !id.starts_with("--") => {
                parsed.summary = Some(Some(id.to_string()));
            }
            other => return Err(format!("unexpected argument: {other}")),
        }
    }
    Ok(parsed)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}\nusage: cargo logbook [--folder DIR] [--port N] [--summary [RUN]]");
            return ExitCode::FAILURE;
        }
    };
    let store = match &args.store {
        Some(folder) => Store {
            logs: folder.join(LOGS),
            crashes: folder.join(CRASHES),
        },
        None => Store {
            logs: user_file(LOGS),
            crashes: user_file(CRASHES),
        },
    };
    if let Some(id) = args.summary {
        let Some(id) = id.or_else(|| store.runs().into_iter().next()) else {
            eprintln!("There are no runs in {}.", store.logs.display());
            return ExitCode::FAILURE;
        };
        return match server::summarised(&store, &id) {
            Ok((_, text, _)) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    let listener = match TcpListener::bind(("127.0.0.1", args.port)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("Couldn't serve on port {}: {error}", args.port);
            return ExitCode::FAILURE;
        }
    };
    println!(
        "Serving the runs in {} at http://127.0.0.1:{}",
        store.logs.display(),
        args.port
    );
    match axum::serve(listener, server::router(store)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &[&str]) -> Result<Args, String> {
        args(line.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn it_serves_unless_asked_for_a_summary() {
        assert_eq!(
            parse(&[]),
            Ok(Args {
                store: None,
                port: PORT,
                summary: None
            })
        );
        assert_eq!(parse(&["--summary"]).unwrap().summary, Some(None));
        assert_eq!(
            parse(&["--summary", "2023-01-01-soak"]).unwrap().summary,
            Some(Some("2023-01-01-soak".into()))
        );
        assert_eq!(parse(&["--port", "8000"]).unwrap().port, 8000);
        assert!(parse(&["--port", "lots"]).is_err());
        assert!(parse(&["2023-01-01-soak"]).is_err());
    }
}
