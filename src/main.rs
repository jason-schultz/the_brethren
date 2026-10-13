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
