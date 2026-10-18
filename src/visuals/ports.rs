//! Placeholder town models and names for ports.

use bevy::prelude::*;

use super::labels::{LabelStyle, world_label};
use crate::port::Port;

/// Town buildings as (east, north) offsets from the town center in meters,
/// plus height. The layout is fixed, so every port looks the same for now.
const BUILDINGS: [(f32, f32, f32); 9] = [
    (-30.0, 10.0, 8.0),
    (-12.0, 18.0, 10.0),
    (6.0, 14.0, 7.0),
    (24.0, 8.0, 9.0),
    (-22.0, -8.0, 7.0),
    (-4.0, -4.0, 12.0),
    (16.0, -10.0, 8.0),
    (34.0, -4.0, 6.0),
    (0.0, -22.0, 9.0),
];
const BUILDING_FOOTPRINT: f32 = 12.0;
/// How far along the line from town to anchorage the pier reaches.
const PIER_REACH: f32 = 0.65;
const PIER_WIDTH: f32 = 6.0;
const LABEL_HEIGHT: f32 = 30.0;

#[derive(Resource)]
struct PortAssets {
    building: Handle<Mesh>,
    pier: Handle<Mesh>,
    walls: Handle<StandardMaterial>,
    timber: Handle<StandardMaterial>,
}

impl FromWorld for PortAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        // Unit-height building and unit-length pier, scaled per instance.
        let building = meshes.add(Cuboid::new(BUILDING_FOOTPRINT, 1.0, BUILDING_FOOTPRINT));
        let pier = meshes.add(Cuboid::new(PIER_WIDTH, 1.0, 1.0));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self {
            building,
            pier,
            walls: materials.add(Color::srgb(0.85, 0.8, 0.7)),
            timber: materials.add(Color::srgb(0.4, 0.28, 0.18)),
        }
    }
}

/// A port's name label, which shows its flag.
#[derive(Component)]
struct PortLabel(Entity);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PortAssets>()
        .add_observer(build_town)
        .add_systems(Update, relabel_ports);
}

fn label_text(name: &Name, port: &Port) -> String {
    format!("{}\n{}", name, port.flag)
}

/// A port that changes hands shows its new flag.
fn relabel_ports(
    ports: Query<(&Name, &Port), Changed<Port>>,
    mut labels: Query<(&PortLabel, &mut Text)>,
) {
    for (label, mut text) in &mut labels {
        if let Ok((name, port)) = ports.get(label.0) {
            let shown = label_text(name, port);
            if text.0 != shown {
                text.0 = shown;
            }
        }
    }
}

/// Builds the town and pier as children of the port, whose origin is the anchorage.
fn build_town(
    add: On<Add<Port>>,
    ports: Query<(&Port, &Name, &Transform)>,
    assets: Res<PortAssets>,
    mut commands: Commands,
) {
    let Ok((port, name, transform)) = ports.get(add.entity) else {
        return;
    };
    let anchorage = transform.translation;
    let town = port.town - anchorage;

    commands.entity(add.entity).insert(Visibility::default());
    for (east, north, height) in BUILDINGS {
        commands.spawn((
            Mesh3d(assets.building.clone()),
            MeshMaterial3d(assets.walls.clone()),
            Transform::from_translation(town + Vec3::new(east, height / 2.0, -north))
                .with_scale(Vec3::new(1.0, height, 1.0)),
            ChildOf(add.entity),
        ));
    }

    let pier_end = town * (1.0 - PIER_REACH);
    let pier_center = (town + pier_end) / 2.0;
    commands.spawn((
        Mesh3d(assets.pier.clone()),
        MeshMaterial3d(assets.timber.clone()),
        Transform::from_translation(pier_center.with_y(1.0))
            .looking_to(pier_end - town, Vec3::Y)
            .with_scale(Vec3::new(1.0, 1.0, town.distance(pier_end))),
        ChildOf(add.entity),
    ));

    commands.spawn((
        PortLabel(add.entity),
        world_label(
            label_text(name, port),
            port.town.with_y(LABEL_HEIGHT),
            LabelStyle::Port,
        ),
    ));
}
