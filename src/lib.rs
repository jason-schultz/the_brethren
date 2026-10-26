//! False Colours: an Age-of-Sail sandbox.
//!
//! [`SimulationPlugin`] is the headless world (safe for tests). [`GamePlugin`]
//! adds input and presentation on top and requires Bevy's `DefaultPlugins`.

pub mod alert;
pub mod allegiance;
pub mod camera;
pub mod cargo;
pub mod chronicle;
pub mod clock;
pub mod combat;
pub mod controls;
pub mod crew;
pub mod economy;
pub mod escorts;
pub mod factions;
pub mod fleet;
pub mod geometry;
pub mod goods;
pub mod hunting;
pub mod leaks;
pub mod loyalty;
pub mod map;
pub mod math;
pub mod merchants;
pub mod navigation;
pub mod new_game;
pub mod officers;
pub mod pardons;
pub mod patrols;
pub mod player;
pub mod port;
pub mod prizes;
pub mod raiders;
pub mod roster;
pub mod sailing;
pub mod save;
pub mod schedule;
pub mod seamanship;
pub mod service;
pub mod ship_class;
pub mod shipyard;
pub mod time_control;
pub mod time_scale;
pub mod tuning;
pub mod ui;
pub mod user_files;
pub mod visuals;
pub mod wind;
pub mod word;
pub mod wrecks;

#[cfg(test)]
mod test_support;

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use time_scale::MAX_STEP;

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
                service::ServicePlugin,
                pardons::PardonsPlugin,
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
                leaks::LeaksPlugin,
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
                seamanship::SeamanshipPlugin,
                allegiance::AllegiancePlugin,
                escorts::EscortsPlugin,
                word::WordPlugin,
                wrecks::WrecksPlugin,
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
            chronicle::ChroniclePlugin {
                run: chronicle::Run::Play,
                seed: None,
            },
        ));
    }
}

/// A headless app running the simulation as fast as it will go: each
/// `update()` advances exactly one fixed step, however long that takes.
/// Tests and soaks run on it.
pub fn stepped_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, SimulationPlugin))
        .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    // Virtual time normally clamps each frame to 250 ms, which would let a
    // long fixed step run only once every few updates.
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .set_max_delta(Duration::from_secs_f64(MAX_STEP * 2.0));
    app
}
