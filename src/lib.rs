//! The Brethren: an Age-of-Sail sandbox.
//!
//! [`SimulationPlugin`] is the headless world (safe for tests). [`GamePlugin`]
//! adds input and presentation on top and requires Bevy's `DefaultPlugins`.

pub mod alert;
pub mod allegiance;
pub mod camera;
pub mod cargo;
pub mod clock;
pub mod combat;
pub mod controls;
pub mod crew;
pub mod economy;
pub mod factions;
pub mod fleet;
pub mod geometry;
pub mod goods;
pub mod hunting;
pub mod loyalty;
pub mod map;
pub mod math;
pub mod merchants;
pub mod navigation;
pub mod new_game;
pub mod officers;
pub mod patrols;
pub mod player;
pub mod port;
pub mod prizes;
pub mod raiders;
pub mod roster;
pub mod sailing;
pub mod save;
pub mod schedule;
pub mod ship_class;
pub mod shipyard;
pub mod time_control;
pub mod time_scale;
pub mod ui;
pub mod user_files;
pub mod visuals;
pub mod wind;

#[cfg(test)]
mod test_support;

use bevy::prelude::*;

/// The world simulation, with no rendering, windowing, or input.
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        schedule::configure_sets(app);
        app.add_plugins((
            // The world and time.
            (
                alert::AlertPlugin,
                factions::FactionsPlugin,
                time_scale::TimeScalePlugin,
                clock::ClockPlugin,
                wind::WindPlugin,
                map::MapPlugin,
            ),
            // Ships and the sea.
            (
                sailing::SailingPlugin,
                navigation::NavigationPlugin,
                port::PortPlugin,
                crew::CrewPlugin,
                combat::CombatPlugin,
                prizes::PrizesPlugin,
                fleet::FleetPlugin,
                shipyard::ShipyardPlugin,
                officers::OfficersPlugin,
                loyalty::LoyaltyPlugin,
            ),
            // Trade.
            (
                goods::GoodsPlugin,
                cargo::CargoPlugin,
                economy::EconomyPlugin,
                merchants::MerchantsPlugin,
                patrols::PatrolsPlugin,
                raiders::RaidersPlugin,
                allegiance::AllegiancePlugin,
            ),
        ));
    }
}

/// The full game: simulation plus player input and presentation.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            SimulationPlugin,
            new_game::NewGamePlugin,
            controls::ControlsPlugin,
            time_control::TimeControlPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            visuals::VisualsPlugin,
            ui::UiPlugin,
            save::quicksave::QuicksavePlugin,
        ));
    }
}
