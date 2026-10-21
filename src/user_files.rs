//! Where the game keeps the player's files: saves, settings, and logs. The
//! folder is worked out in the `chronicle` crate, which the report tool
//! shares, so both look in the same place.

pub use chronicle::user_file;
