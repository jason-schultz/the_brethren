//! Pausing. The simulation runs on virtual time, so pausing it stops the
//! world while input and rendering continue.

use bevy::prelude::*;

use crate::schedule::FrameSet;

pub struct TimeControlPlugin;

impl Plugin for TimeControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, toggle_pause.in_set(FrameSet::Input));
    }
}

fn toggle_pause(keys: Res<ButtonInput<KeyCode>>, mut time: ResMut<Time<Virtual>>) {
    if keys.just_pressed(KeyCode::Space) {
        time.toggle();
    }
}
