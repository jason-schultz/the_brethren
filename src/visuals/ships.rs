//! Placeholder 3D models for ships, attached to any entity that gains a
//! [`Hull`], and the names of ships at sea.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use super::labels::{LabelStyle, WorldLabel, world_label};
use crate::player::Player;
use crate::port::Docked;
use crate::sailing::Hull;
use crate::ship_class::ShipClass;

/// A ship's name label floats this high above her, meters, clear of her masts.
const LABEL_HEIGHT: f32 = 25.0;

/// The name label of a ship.
#[derive(Component, Debug)]
struct ShipLabel(Entity);

/// Meshes and materials shared by every ship model, created once.
#[derive(Resource)]
pub(super) struct ShipAssets {
    hull: Handle<Mesh>,
    mast: Handle<Mesh>,
    sail: Handle<Mesh>,
    wood: Handle<StandardMaterial>,
    canvas: Handle<StandardMaterial>,
}

impl FromWorld for ShipAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let hull = meshes.add(Cuboid::new(6.0, 2.5, 20.0));
        let mast = meshes.add(Cylinder::new(0.3, 18.0));
        let sail = meshes.add(Cuboid::new(0.2, 12.0, 9.0));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self {
            hull,
            mast,
            sail,
            wood: materials.add(Color::srgb(0.36, 0.22, 0.12)),
            canvas: materials.add(Color::srgb(0.93, 0.9, 0.82)),
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ShipAssets>()
        .add_observer(attach_ship_model)
        .add_observer(label_ship)
        .add_systems(Update, follow_ships);
}

fn label_ship(add: On<Add<Hull>>, names: Query<&Name>, mut commands: Commands) {
    if let Ok(name) = names.get(add.entity) {
        commands.spawn((
            ShipLabel(add.entity),
            world_label(name.as_str(), Vec3::ZERO, LabelStyle::Ship),
        ));
    }
}

/// Keeps each label over her ship. Ships in port and the player's own go
/// unlabelled, and a label outlives its ship only until the next frame.
fn follow_ships(
    mut labels: Query<(Entity, &ShipLabel, &mut WorldLabel)>,
    ships: Query<(&Transform, Has<Docked>, Has<Player>)>,
    mut commands: Commands,
) {
    for (label, &ShipLabel(ship), mut world_label) in &mut labels {
        let Ok((transform, docked, player)) = ships.get(ship) else {
            commands.entity(label).despawn();
            continue;
        };
        world_label.anchor = transform.translation + Vec3::Y * LABEL_HEIGHT;
        world_label.shown = !docked && !player;
    }
}

/// A ship model of `class`, for spawning as a child of whatever places
/// her. The parts go on `layers`, since a render layer isn't inherited.
pub(super) fn ship_model(
    assets: &ShipAssets,
    class: &ShipClass,
    layers: RenderLayers,
) -> impl Bundle + use<> {
    let part = |mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, at: Vec3| {
        (
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at),
            layers.clone(),
        )
    };
    (
        // Models are drawn at a sloop's size; others are scaled to theirs.
        Transform::from_scale(Vec3::splat(class.length_m / ShipClass::SLOOP.length_m)),
        Visibility::default(),
        children![
            part(&assets.hull, &assets.wood, Vec3::new(0.0, 0.5, 0.0)),
            part(&assets.mast, &assets.wood, Vec3::new(0.0, 9.0, -2.0)),
            // Gaff mainsail, set fore-and-aft just aft of the mast.
            part(&assets.sail, &assets.canvas, Vec3::new(0.0, 9.0, 2.7)),
        ],
    )
}

fn attach_ship_model(
    add: On<Add<Hull>>,
    assets: Res<ShipAssets>,
    classes: Query<&ShipClass>,
    mut commands: Commands,
) {
    let ship = add.entity;
    let class = classes.get(ship).copied().unwrap_or(ShipClass::SLOOP);
    commands.entity(ship).insert(Visibility::default());
    commands.spawn((
        ship_model(&assets, &class, RenderLayers::default()),
        ChildOf(ship),
    ));
}
