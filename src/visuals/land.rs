//! Land meshes and place names, rebuilt whenever [`Land`] changes.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use earcut::Earcut;

use super::labels::{LabelStyle, world_label};
use crate::map::Land;
use crate::map::data::Terrain;

/// Height of the land surface above the water, meters.
const LAND_HEIGHT: f32 = 0.5;
/// Shoals sit just above the water, under any land drawn on them.
const SHOAL_HEIGHT: f32 = 0.1;
/// Place names float this high over the land so they stay readable at an angle.
const LABEL_HEIGHT: f32 = 10.0;
/// Texture-coordinate scale for land meshes (one UV unit per this many meters).
const UV_METERS: f32 = 100.0;

/// Marks entities that belong to the current land visuals.
#[derive(Component)]
struct LandVisual;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, rebuild_land.run_if(resource_changed::<Land>));
}

fn rebuild_land(
    land: Res<Land>,
    old: Query<Entity, With<LandVisual>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    if land.masses.is_empty() {
        return;
    }
    let sand = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.78, 0.58),
        perceptual_roughness: 0.95,
        ..default()
    });
    let forest = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.38, 0.18),
        perceptual_roughness: 0.95,
        ..default()
    });
    let shoal = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.75, 0.75, 0.55),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.4,
        ..default()
    });
    for mass in &land.masses {
        let material = match mass.terrain {
            Terrain::Sand => sand.clone(),
            Terrain::Forest => forest.clone(),
            Terrain::Shoal => shoal.clone(),
        };
        commands.spawn((
            LandVisual,
            Name::new(mass.name.clone()),
            Mesh3d(meshes.add(land_mesh(&mass.outline, height(mass.terrain)))),
            MeshMaterial3d(material),
        ));
        commands.spawn((
            LandVisual,
            world_label(
                &mass.name,
                mass.label.with_y(LABEL_HEIGHT),
                LabelStyle::Place,
            ),
        ));
    }
}

fn height(terrain: Terrain) -> f32 {
    match terrain {
        Terrain::Sand | Terrain::Forest => LAND_HEIGHT,
        Terrain::Shoal => SHOAL_HEIGHT,
    }
}

/// A flat, upward-facing mesh filling `outline` at `height` above the water.
fn land_mesh(outline: &[Vec2], height: f32) -> Mesh {
    let positions: Vec<[f32; 3]> = outline.iter().map(|p| [p.x, height, p.y]).collect();
    let uvs: Vec<[f32; 2]> = outline
        .iter()
        .map(|p| (*p / UV_METERS).to_array())
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; outline.len()])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(triangulate(outline)))
}

/// Triangle indices covering a simple polygon given in `(x, z)` plane
/// coordinates. Every triangle faces up (+Y).
fn triangulate(outline: &[Vec2]) -> Vec<u32> {
    let mut indices: Vec<u32> = Vec::new();
    Earcut::new().earcut(
        outline.iter().map(|p| [p.x, p.y]),
        &[] as &[u32],
        &mut indices,
    );
    for triangle in indices.as_chunks_mut::<3>().0 {
        let [a, b, c] = [0, 1, 2].map(|i| outline[triangle[i] as usize]);
        // In (x, z) coordinates, an upward normal means clockwise winding.
        if (b - a).perp_dot(c - a) > 0.0 {
            triangle.swap(1, 2);
        }
    }
    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::signed_area_doubled;

    fn l_shape() -> Vec<Vec2> {
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 5.0),
            Vec2::new(5.0, 5.0),
            Vec2::new(5.0, 10.0),
            Vec2::new(0.0, 10.0),
        ]
    }

    fn triangles(outline: &[Vec2]) -> Vec<[Vec3; 3]> {
        triangulate(outline)
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                [0, 1, 2]
                    .map(|i| Vec3::new(outline[t[i] as usize].x, 0.0, outline[t[i] as usize].y))
            })
            .collect()
    }

    #[test]
    fn every_triangle_faces_up_for_either_winding() {
        let mut reversed = l_shape();
        reversed.reverse();
        for outline in [l_shape(), reversed] {
            for [a, b, c] in triangles(&outline) {
                assert!(
                    (b - a).cross(c - a).y > 0.0,
                    "triangle {a} {b} {c} faces down"
                );
            }
        }
    }

    #[test]
    fn triangles_cover_the_polygon_exactly() {
        let outline = l_shape();
        let triangle_area: f32 = triangles(&outline)
            .iter()
            .map(|[a, b, c]| (b - a).cross(c - a).length() / 2.0)
            .sum();
        let polygon_area = signed_area_doubled(&outline).abs() / 2.0;
        assert!(
            (triangle_area - polygon_area).abs() < 1e-3,
            "{triangle_area} vs {polygon_area}"
        );
        assert_eq!(triangles(&outline).len(), outline.len() - 2);
    }
}
