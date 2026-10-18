//! The economy: port markets, trade, and contracts.
//!
//! One model drives it all. Each port's [`market::Market`] holds stock of
//! the goods it produces and needs. Stock drifts every day, prices follow
//! stock, and a port running short of something it needs posts a
//! [`jobs::Job`] for a delivery from a port that produces it.

pub mod jobs;
pub mod market;

use std::fmt;

use bevy::prelude::*;

use crate::schedule::{FrameSet, SimSet};

/// Why a trade or contract can't go ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// This port doesn't deal in that here.
    NotTraded,
    /// The port hasn't got that many to sell.
    OutOfStock,
    CantAfford,
    /// No room aboard, in the hold or in the berths.
    NoRoom,
    /// The ship hasn't got that many to sell.
    NotEnough,
    /// The ship isn't docked where this happens.
    NotHere,
    /// She already has an officer in that post.
    PostFilled,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::NotTraded => "not traded here",
            Self::OutOfStock => "none to be had",
            Self::CantAfford => "can't afford it",
            Self::NoRoom => "no room aboard",
            Self::NotEnough => "not enough aboard",
            Self::NotHere => "not at this port",
            Self::PostFilled => "the post is filled",
        })
    }
}

pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<market::Trade>()
            .add_message::<market::Supplied>()
            .add_message::<jobs::AcceptJob>()
            .add_systems(
                FixedUpdate,
                (
                    market::drift_stock,
                    jobs::warn_of_deadlines,
                    jobs::deliver_jobs,
                    jobs::expire_jobs,
                    jobs::post_jobs,
                )
                    .chain()
                    .in_set(SimSet::Economy),
            )
            // Trading and signing contracts are orders, so they work while paused.
            .add_systems(
                Update,
                (market::trade, jobs::accept_jobs).in_set(FrameSet::Orders),
            );
    }
}
