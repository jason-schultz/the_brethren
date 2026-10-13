//! Placeholder 3D models for ships. Attached to any entity that gains a [`Hull`].

use bevy::prelude::*;

use crate::sailing::Hull;

/// Meshes and materials shared by every ship model, created once.
#[derive(Resource)]
struct ShipAssets {
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
        .add_observer(attach_ship_model);
}

fn attach_ship_model(add: On<Add<Hull>>, assets: Res<ShipAssets>, mut commands: Commands) {
    let ship = add.entity;
    commands.entity(ship).insert(Visibility::default());
    let parts = [
        (
            &assets.hull,
            &assets.wood,
            Transform::from_xyz(0.0, 0.5, 0.0),
        ),
        (
            &assets.mast,
            &assets.wood,
            Transform::from_xyz(0.0, 9.0, -2.0),
        ),
        // Gaff mainsail, set fore-and-aft just aft of the mast.
        (
            &assets.sail,
            &assets.canvas,
            Transform::from_xyz(0.0, 9.0, 2.7),
        ),
    ];
    for (mesh, material, transform) in parts {
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            transform,
            ChildOf(ship),
        ));
    }
}
