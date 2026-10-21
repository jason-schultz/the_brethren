//! Wrecks on the chart: a marker and a name, for every wreck the captain
//! has heard of or can see.

use bevy::prelude::*;

use super::labels::{LabelStyle, WorldLabel, world_label};
use crate::alert::SIGHTING_RANGE;
use crate::camera::CameraRig;
use crate::player::Player;
use crate::word::{Happening, Knowledge};
use crate::wrecks::Wreck;

/// Markers are drawn this fraction of the camera distance across.
const MARKER_SCALE: f32 = 0.01;
/// Overlay height above the water.
const OVERLAY_Y: f32 = 0.5;
const COLOUR: Color = Color::srgb(0.75, 0.65, 0.5);

/// The name label of a wreck.
#[derive(Component, Debug)]
struct WreckLabel(Entity);

pub(super) fn plugin(app: &mut App) {
    app.add_observer(label_wreck)
        .add_systems(Update, (follow_wrecks, draw_wrecks));
}

fn label_wreck(add: On<Add<Wreck>>, names: Query<&Name>, mut commands: Commands) {
    if let Ok(name) = names.get(add.entity) {
        commands
            .spawn((
                WreckLabel(add.entity),
                world_label(name.as_str(), Vec3::ZERO, LabelStyle::Ship),
            ))
            .insert(TextColor(COLOUR));
    }
}

/// Whether the captain at `at`, knowing what `knowledge` holds, knows of
/// `wreck` lying at `lies`: she's heard of it, or she can see it.
fn known(knowledge: &Knowledge, at: Vec3, wreck: Entity, lies: Vec3) -> bool {
    at.distance(lies) <= SIGHTING_RANGE
        || knowledge.0.iter().any(|tale| {
            matches!(tale.event, Happening::Wrecked { wreck: Some(heard), .. } if heard == wreck)
        })
}

fn follow_wrecks(
    mut labels: Query<(Entity, &WreckLabel, &mut WorldLabel)>,
    wrecks: Query<&Transform, With<Wreck>>,
    player: Single<(&Transform, &Knowledge), With<Player>>,
    mut commands: Commands,
) {
    let (at, knowledge) = *player;
    for (label, &WreckLabel(wreck), mut world_label) in &mut labels {
        let Ok(lies) = wrecks.get(wreck) else {
            commands.entity(label).despawn();
            continue;
        };
        world_label.anchor = lies.translation + Vec3::Y * 10.0;
        world_label.shown = known(knowledge, at.translation, wreck, lies.translation);
    }
}

/// A cross over every wreck the captain knows of.
fn draw_wrecks(
    wrecks: Query<(Entity, &Transform), With<Wreck>>,
    player: Single<(&Transform, &Knowledge), With<Player>>,
    rig: Single<&CameraRig>,
    mut gizmos: Gizmos,
) {
    let (at, knowledge) = *player;
    let arm = rig.distance * MARKER_SCALE;
    for (wreck, lies) in &wrecks {
        let lies = lies.translation;
        if !known(knowledge, at.translation, wreck, lies) {
            continue;
        }
        let centre = lies.with_y(OVERLAY_Y);
        for corner in [Vec3::new(1.0, 0.0, 1.0), Vec3::new(1.0, 0.0, -1.0)] {
            gizmos.line(centre - corner * arm, centre + corner * arm, COLOUR);
        }
    }
}
