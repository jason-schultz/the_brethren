// Release builds on Windows open no console window beside the game.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::log::LogPlugin;
use bevy::prelude::*;
use false_colours::GamePlugin;
use false_colours::chronicle::log_layer;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "False Colours".into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(LogPlugin {
                    custom_layer: log_layer,
                    ..default()
                }),
            GamePlugin,
        ))
        .run();
}
