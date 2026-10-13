//! The sea, the sky, and the sun.

use bevy::prelude::*;

use crate::player::Player;

/// Half-width of the ocean surface, meters. Large enough that its edge is
/// never seen at max zoom on the first map.
const OCEAN_HALF_SIZE: f32 = 100_000.0;
/// Reference grid spacing, meters. Gives a sense of motion on open water.
const GRID_SPACING: f32 = 100.0;
const GRID_CELLS: u32 = 80;

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.85)))
        .add_systems(Startup, spawn_ocean)
        .add_systems(Update, draw_grid);
}

fn spawn_ocean(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("Ocean"),
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(OCEAN_HALF_SIZE * 2.0, OCEAN_HALF_SIZE * 2.0),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.22, 0.35),
            perceptual_roughness: 0.25,
            ..default()
        })),
    ));
    commands.spawn((
        Name::new("Sun"),
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(300.0, 800.0, 200.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Draws a faint grid snapped around the player so motion is visible on open water.
fn draw_grid(player: Single<&Transform, With<Player>>, mut gizmos: Gizmos) {
    let snapped = (player.translation / GRID_SPACING).round() * GRID_SPACING;
    gizmos.grid(
        Isometry3d::new(
            snapped.with_y(0.05),
            Quat::from_rotation_arc(Vec3::Z, Vec3::Y),
        ),
        UVec2::splat(GRID_CELLS),
        Vec2::splat(GRID_SPACING),
        Color::srgba(1.0, 1.0, 1.0, 0.06),
    );
}
