//! Where the game keeps the player's files: saves and settings.

use std::env;
use std::path::PathBuf;

/// `<name>` in the platform's folder for application data, or in the
/// working directory if there's none:
/// - macOS: `~/Library/Application Support/The Brethren`
/// - Windows: `%APPDATA%\The Brethren`
/// - Linux: `$XDG_DATA_HOME/the-brethren`, or `~/.local/share/the-brethren`
pub fn user_file(name: &str) -> PathBuf {
    folder().unwrap_or_default().join(name)
}

#[cfg(target_os = "macos")]
fn folder() -> Option<PathBuf> {
    let home = PathBuf::from(env::var_os("HOME")?);
    Some(home.join("Library/Application Support/The Brethren"))
}

#[cfg(target_os = "windows")]
fn folder() -> Option<PathBuf> {
    Some(PathBuf::from(env::var_os("APPDATA")?).join("The Brethren"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn folder() -> Option<PathBuf> {
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(env::var_os("HOME")?).join(".local/share")))?;
    Some(data.join("the-brethren"))
}
