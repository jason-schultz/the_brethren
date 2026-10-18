// Release builds on Windows open no console window beside the game.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::prelude::*;
use the_brethren::GamePlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "The Brethren".into(),
                    ..default()
                }),
                ..default()
            }),
            GamePlugin,
        ))
        .run();
}
