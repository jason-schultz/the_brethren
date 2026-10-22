//! The captain's portrait: the captain standing on the quay beside his
//! ship, rendered to an image for the captain sheet. It lives on its own
//! render layer, far from the sea, and its camera only renders while
//! someone's looking.

use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use super::ships::{ShipAssets, ship_model};
use crate::player::Player;
use crate::ship_class::ShipClass;

/// The portrait's size, pixels. Shown at half this, for crisp edges.
pub const PORTRAIT_SIZE: UVec2 = UVec2::new(960, 520);
/// The render layer the portrait lives on.
const LAYER: usize = 1;
/// Where the stage is: well away from anything else.
const STAGE: Vec3 = Vec3::new(0.0, -5_000.0, 0.0);
/// Where she lies, from the middle of the stage, broadside on.
const SHIP_AT: Vec3 = Vec3::new(8.0, 0.0, -22.0);
/// Where he stands, close to the camera.
const CAPTAIN_AT: Vec3 = Vec3::new(-2.2, 0.0, 4.0);
/// Where the camera stands, and what it looks at: up a little, to take in
/// her masthead.
const CAMERA_AT: Vec3 = Vec3::new(0.0, 1.0, 9.0);
const LOOKING_AT: Vec3 = Vec3::new(2.0, 4.0, -10.0);
const SKY: Color = Color::srgb(0.62, 0.78, 0.9);
const SEA: Color = Color::srgb(0.1, 0.33, 0.48);

/// The portrait's image.
#[derive(Resource, Debug, Clone)]
pub struct Portrait(pub Handle<Image>);

/// The camera that renders the portrait. Activate it to keep the portrait
/// up to date.
#[derive(Component, Debug)]
pub struct PortraitCamera;

/// The model of the player's ship on the stage, kept to her class.
#[derive(Component, Debug)]
struct StageShip(ShipClass);

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, build_stage)
        .add_systems(Update, show_her_class);
}

fn build_stage(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ships: Res<ShipAssets>,
) {
    let image = images.add(Image::new_target_texture(
        PORTRAIT_SIZE.x,
        PORTRAIT_SIZE.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    commands.insert_resource(Portrait(image.clone()));
    let layer = RenderLayers::layer(LAYER);
    commands.spawn((
        PortraitCamera,
        Camera3d::default(),
        Camera {
            order: -1,
            is_active: false,
            clear_color: SKY.into(),
            ..default()
        },
        RenderTarget::Image(image.into()),
        Projection::Perspective(PerspectiveProjection {
            fov: 50_f32.to_radians(),
            ..default()
        }),
        Transform::from_translation(STAGE + CAMERA_AT).looking_at(STAGE + LOOKING_AT, Vec3::Y),
        layer.clone(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_translation(STAGE + Vec3::new(-6.0, 12.0, 10.0)).looking_at(STAGE, Vec3::Y),
        layer.clone(),
    ));

    let mut flat = |color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.8,
            ..default()
        })
    };
    let sea = flat(SEA);
    let planks = flat(Color::srgb(0.45, 0.33, 0.22));
    let mut prop = |mesh: Mesh, material: &Handle<StandardMaterial>, at: Vec3| {
        (
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(STAGE + at),
            layer.clone(),
        )
    };
    commands.spawn(prop(
        Plane3d::default().mesh().size(400.0, 400.0).build(),
        &sea,
        Vec3::new(0.0, -0.3, 0.0),
    ));
    // The quay he stands on.
    commands.spawn(prop(
        Cuboid::new(8.0, 1.0, 5.0).into(),
        &planks,
        Vec3::new(CAPTAIN_AT.x, -0.5, CAPTAIN_AT.z),
    ));
    commands.spawn(captain(&mut meshes, &mut materials, &layer));
    commands.spawn((
        StageShip(ShipClass::sloop()),
        Transform::from_translation(STAGE + SHIP_AT)
            .with_rotation(Quat::from_rotation_y(100_f32.to_radians())),
        Visibility::default(),
        children![ship_model(&ships, &ShipClass::sloop(), layer)],
    ));
}

/// A captain of the pirates' golden age: tricorn, long coat, breeches, and boots, built
/// from simple shapes.
fn captain(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    layer: &RenderLayers,
) -> impl Bundle + use<> {
    let mut paint = |color: Color| materials.add(color);
    let coat = paint(Color::srgb(0.55, 0.1, 0.1));
    let cuffs = paint(Color::srgb(0.85, 0.7, 0.3));
    let skin = paint(Color::srgb(0.87, 0.68, 0.55));
    let black = paint(Color::srgb(0.08, 0.07, 0.07));
    let linen = paint(Color::srgb(0.92, 0.9, 0.84));
    let hair = paint(Color::srgb(0.3, 0.2, 0.12));
    let mut part = |mesh: Mesh, material: &Handle<StandardMaterial>, transform: Transform| {
        (
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            transform,
            layer.clone(),
        )
    };
    let at = |x: f32, y: f32, z: f32| Transform::from_xyz(x, y, z);
    let tilted = |x: f32, y: f32, z: f32, roll: f32| {
        Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_z(roll))
    };
    (
        Transform::from_translation(STAGE + CAPTAIN_AT)
            // Facing us, turned a little toward his ship.
            .with_rotation(Quat::from_rotation_y(20_f32.to_radians())),
        Visibility::default(),
        children![
            // Boots and breeches.
            part(
                Capsule3d::new(0.09, 0.45).into(),
                &black,
                at(-0.13, 0.3, 0.0)
            ),
            part(
                Capsule3d::new(0.09, 0.45).into(),
                &black,
                at(0.13, 0.3, 0.0)
            ),
            part(
                Capsule3d::new(0.1, 0.25).into(),
                &linen,
                at(-0.13, 0.75, 0.0)
            ),
            part(
                Capsule3d::new(0.1, 0.25).into(),
                &linen,
                at(0.13, 0.75, 0.0)
            ),
            // His long coat, flaring to the knee, and the waistcoat under it.
            part(
                ConicalFrustum {
                    radius_top: 0.24,
                    radius_bottom: 0.36,
                    height: 0.9,
                }
                .into(),
                &coat,
                at(0.0, 1.05, 0.0),
            ),
            part(Capsule3d::new(0.23, 0.35).into(), &coat, at(0.0, 1.38, 0.0)),
            part(
                Cuboid::new(0.2, 0.4, 0.05).into(),
                &linen,
                at(0.0, 1.35, 0.22)
            ),
            part(Sphere::new(0.06).into(), &linen, at(0.0, 1.6, 0.17)),
            // Arms, one on his hip.
            part(
                Capsule3d::new(0.07, 0.5).into(),
                &coat,
                tilted(-0.33, 1.3, 0.0, -0.15)
            ),
            part(
                Capsule3d::new(0.07, 0.5).into(),
                &coat,
                tilted(0.36, 1.32, 0.05, 0.6)
            ),
            part(
                Cylinder::new(0.085, 0.1).into(),
                &cuffs,
                tilted(-0.37, 1.02, 0.0, -0.15)
            ),
            part(Sphere::new(0.06).into(), &skin, at(-0.39, 0.92, 0.0)),
            // A cutlass at his side.
            part(
                Cuboid::new(0.03, 0.75, 0.05).into(),
                &cuffs,
                tilted(-0.3, 0.75, 0.15, 0.25)
            ),
            // Head, hair tied back, and a tricorn.
            part(Sphere::new(0.13).into(), &skin, at(0.0, 1.78, 0.0)),
            part(Sphere::new(0.12).into(), &hair, at(0.0, 1.8, -0.04)),
            part(Sphere::new(0.018).into(), &black, at(-0.045, 1.8, 0.12)),
            part(Sphere::new(0.018).into(), &black, at(0.045, 1.8, 0.12)),
            part(Cylinder::new(0.26, 0.05).into(), &black, at(0.0, 1.9, 0.0)),
            part(
                ConicalFrustum {
                    radius_top: 0.1,
                    radius_bottom: 0.15,
                    height: 0.12,
                }
                .into(),
                &black,
                at(0.0, 1.97, 0.0),
            ),
        ],
    )
}

/// Keeps the ship on the stage to the class of the player's ship.
fn show_her_class(
    player: Single<&ShipClass, With<Player>>,
    stage: Single<(Entity, &mut StageShip)>,
    ships: Res<ShipAssets>,
    mut commands: Commands,
) {
    let (entity, mut shown) = stage.into_inner();
    if shown.0 == **player {
        return;
    }
    shown.0 = **player;
    commands
        .entity(entity)
        .despawn_children()
        .with_child(ship_model(&ships, &player, RenderLayers::layer(LAYER)));
}
