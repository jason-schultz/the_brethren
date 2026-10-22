//! Placeholder 3D models for ships, rigged after their class and rebuilt
//! when she changes it, and the names of ships at sea, in the colour of
//! their flags.

use bevy::camera::visibility::RenderLayers;
use bevy::ecs::query::QueryData;
use bevy::prelude::*;

use super::labels::{LabelStyle, WorldLabel, world_label};
use super::nation_colour;
use crate::factions::Colours;
use crate::player::Player;
use crate::port::Docked;
use crate::sailing::Hull;
use crate::ship_class::{Rig, ShipClass};

/// A ship's name label floats this high above her, meters, clear of her masts.
const LABEL_HEIGHT: f32 = 25.0;

/// The name label of a ship.
#[derive(Component, Debug)]
struct ShipLabel(Entity);

/// The model drawn for the ship she's a child of.
#[derive(Component, Debug)]
struct ShipModel;

/// How a sail is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sail {
    /// Across the ship, hung from a yard.
    Square,
    /// Along the ship, abaft the mast.
    ForeAndAft,
}

/// Where each of a rig's masts stands on a model's deck, meters from
/// amidships (forward is −Z), and how she sets her sail on it.
fn masts(rig: Rig) -> &'static [(f32, Sail)] {
    match rig {
        Rig::Lugger | Rig::Sloop => &[(-2.0, Sail::ForeAndAft)],
        Rig::Schooner => &[(-5.0, Sail::ForeAndAft), (2.0, Sail::ForeAndAft)],
        Rig::Brigantine => &[(-5.0, Sail::Square), (2.0, Sail::ForeAndAft)],
        Rig::Brig => &[(-5.0, Sail::Square), (3.0, Sail::Square)],
        Rig::Ship => &[
            (-6.0, Sail::Square),
            (0.0, Sail::Square),
            (6.0, Sail::Square),
        ],
    }
}

/// Meshes and materials shared by every ship model, created once.
#[derive(Resource)]
pub(super) struct ShipAssets {
    hull: Handle<Mesh>,
    mast: Handle<Mesh>,
    square_sail: Handle<Mesh>,
    fore_and_aft_sail: Handle<Mesh>,
    wood: Handle<StandardMaterial>,
    canvas: Handle<StandardMaterial>,
}

impl FromWorld for ShipAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let hull = meshes.add(Cuboid::new(6.0, 2.5, 20.0));
        let mast = meshes.add(Cylinder::new(0.3, 18.0));
        let square_sail = meshes.add(Cuboid::new(9.0, 10.0, 0.2));
        let fore_and_aft_sail = meshes.add(Cuboid::new(0.2, 12.0, 6.0));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self {
            hull,
            mast,
            square_sail,
            fore_and_aft_sail,
            wood: materials.add(Color::srgb(0.36, 0.22, 0.12)),
            canvas: materials.add(Color::srgb(0.93, 0.9, 0.82)),
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ShipAssets>()
        .add_observer(model_ship)
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

/// A ship as her label follows her: where she is, whether she's in port
/// or the player's, and whose colours she flies.
#[derive(QueryData)]
struct Labelled {
    at: &'static Transform,
    docked: Has<Docked>,
    player: Has<Player>,
    colours: Option<&'static Colours>,
}

/// Keeps each label over her ship, in the colour of the flag she flies.
/// Ships in port and the player's own go unlabelled, and a label outlives
/// its ship only until the next frame.
fn follow_ships(
    mut labels: Query<(Entity, &ShipLabel, &mut WorldLabel, &mut TextColor)>,
    ships: Query<Labelled>,
    mut commands: Commands,
) {
    for (label, &ShipLabel(ship), mut world_label, mut color) in &mut labels {
        let Ok(LabelledItem {
            at: transform,
            docked,
            player,
            colours,
        }) = ships.get(ship)
        else {
            commands.entity(label).despawn();
            continue;
        };
        world_label.anchor = transform.translation + Vec3::Y * LABEL_HEIGHT;
        world_label.shown = !docked && !player;
        if let Some(&Colours(nation)) = colours {
            color.set_if_neq(TextColor(nation_colour(nation)));
        }
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
    let rigging = masts(class.rig).iter().flat_map(|&(at, sail)| {
        let (mesh, sail_at) = match sail {
            Sail::Square => (&assets.square_sail, at),
            Sail::ForeAndAft => (&assets.fore_and_aft_sail, at + 3.2),
        };
        [
            part(&assets.mast, &assets.wood, Vec3::new(0.0, 9.0, at)),
            part(mesh, &assets.canvas, Vec3::new(0.0, 9.0, sail_at)),
        ]
    });
    let parts: Vec<_> = std::iter::once(part(&assets.hull, &assets.wood, Vec3::new(0.0, 0.5, 0.0)))
        .chain(rigging)
        .collect();
    (
        // Models are drawn at a sloop's size; others are scaled to theirs.
        Transform::from_scale(Vec3::splat(class.length_m / ShipClass::sloop().length_m)),
        Visibility::default(),
        Children::spawn(parts),
    )
}

/// Draws a ship as her class, afresh whenever she changes it.
fn model_ship(
    insert: On<Insert<ShipClass>>,
    assets: Res<ShipAssets>,
    classes: Query<&ShipClass>,
    models: Query<(Entity, &ChildOf), With<ShipModel>>,
    mut commands: Commands,
) {
    let ship = insert.entity;
    let Ok(class) = classes.get(ship) else {
        return;
    };
    for (model, _) in models.iter().filter(|(_, parent)| parent.parent() == ship) {
        commands.entity(model).despawn();
    }
    commands.entity(ship).insert(Visibility::default());
    commands.spawn((
        ShipModel,
        ship_model(&assets, class, RenderLayers::default()),
        ChildOf(ship),
    ));
}
