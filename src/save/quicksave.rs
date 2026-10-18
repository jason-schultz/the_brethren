//! Quicksave and quickload (F5 and F9 unless rebound), to one file in the
//! user's Application Support folder.

use std::fs;
use std::path::PathBuf;

use bevy::prelude::*;

use super::{SaveGame, capture, restore};
use crate::camera::CameraRig;
use crate::controls::{Control, used};
use crate::schedule::FrameSet;
use crate::time_control::LatestAlert;
use crate::time_scale::{SPEEDS, TimeScale};
use crate::ui::Notices;
use crate::user_files::user_file;

pub struct QuicksavePlugin;

impl Plugin for QuicksavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                quicksave.run_if(used(Control::Save)),
                quickload.run_if(used(Control::Load)),
            )
                .in_set(FrameSet::Input),
        );
    }
}

fn save_path() -> PathBuf {
    user_file("quicksave.ron")
}

fn notify(world: &mut World, text: String) {
    let now = world.resource::<Time<Real>>().elapsed_secs();
    world.resource_mut::<Notices>().post(text, now);
}

fn quicksave(world: &mut World) {
    let save = capture(world);
    let path = save_path();
    let written = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&path, save.to_ron()));
    let text = match written {
        Ok(()) => "Game saved.".to_string(),
        Err(error) => format!("Couldn't save: {error}"),
    };
    notify(world, text);
}

fn quickload(world: &mut World) {
    let loaded = fs::read_to_string(save_path())
        .map_err(|error| error.to_string())
        .and_then(|source| SaveGame::from_ron(&source).map_err(|error| error.to_string()))
        .and_then(|save| restore(world, &save).map_err(|error| error.to_string()));
    let text = match loaded {
        Ok(()) => {
            // Come back to the ship at normal speed, with nothing pending.
            world.resource_mut::<TimeScale>().speed = SPEEDS[0];
            world.resource_mut::<LatestAlert>().0 = None;
            if let Ok(mut rig) = world.query::<&mut CameraRig>().single_mut(world) {
                rig.following = true;
            }
            "Game loaded.".to_string()
        }
        Err(error) => format!("Couldn't load: {error}"),
    };
    notify(world, text);
}
