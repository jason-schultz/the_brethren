//! The influence overlay: a translucent wash of each nation's colour over
//! the waters it holds. A nation holds the sea around its ports, farther
//! the more loyal they are, and the lanes its ships keep sailing: wakes
//! build up where its ships pass, patrols most of all, and fade when they
//! stop coming. Allies' colours blend where they share the sea.
//!
//! Toggled with [`Control::Influence`]. Wakes are laid all the time, so the
//! lanes are there the moment the overlay is shown.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::nation_colour;
use crate::allegiance::{Favour, OWNER_REST};
use crate::clock::{DAY, WorldClock};
use crate::controls::{Control, used};
use crate::factions::{Colours, NATIONS};
use crate::map::Land;
use crate::math::to_plane;
use crate::patrols::Patrol;
use crate::port::{Docked, Port};

/// Cells across the overlay's longer side.
const CELLS: u32 = 240;
/// The overlay reaches this far past the outermost land, meters.
const MARGIN: f32 = 60_000.0;
/// Above the water, and below the land.
const OVERLAY_Y: f32 = 0.3;
/// A loyal port holds the sea this far around it, meters. A restless one
/// holds less.
const PORT_REACH: f32 = 70_000.0;
/// A wake halves in strength over this many days without ships passing.
const WAKE_HALF_LIFE: f64 = 3.0;
/// A ship passing a cell lays this much wake a day she spends there; a
/// patrol counts for more.
const WAKE_PER_DAY: f32 = 30.0;
const PATROL_WAKE: f32 = 3.0;
/// Wake at which a lane counts as fully held.
const WAKE_HELD: f32 = 1.0;
/// The most opaque the wash gets.
const MOST_ALPHA: f32 = 0.6;
/// Below this strength, the sea is no one's.
const FAINTEST: f32 = 0.05;
/// How often the shown overlay is repainted, real seconds.
const REPAINT_SECONDS: f32 = 0.5;

/// Whether the overlay is shown.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShowInfluence(pub bool);

/// The grid the overlay is painted on, and the wakes laid on it.
#[derive(Resource, Debug)]
struct Chart {
    /// The world rectangle it covers, in ocean-plane coordinates.
    bounds: Rect,
    size: UVec2,
    /// Wake laid in each cell by each nation's ships.
    wakes: Vec<[f32; NATIONS.len()]>,
    /// Whether each cell is land, which no one's colour covers.
    land: Vec<bool>,
    image: Handle<Image>,
    /// The clock at the last wakes laid.
    laid_at: f64,
    painted_at: f32,
}

impl Chart {
    fn cell(&self, at: Vec2) -> Option<usize> {
        let fraction = (at - self.bounds.min) / self.bounds.size();
        if !(0.0..1.0).contains(&fraction.x) || !(0.0..1.0).contains(&fraction.y) {
            return None;
        }
        let cell = (fraction * self.size.as_vec2()).as_uvec2();
        Some((cell.y * self.size.x + cell.x) as usize)
    }

    fn centre(&self, index: usize) -> Vec2 {
        let cell = UVec2::new(index as u32 % self.size.x, index as u32 / self.size.x);
        self.bounds.min + (cell.as_vec2() + 0.5) / self.size.as_vec2() * self.bounds.size()
    }
}

/// The overlay's plane.
#[derive(Component)]
struct InfluenceOverlay;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ShowInfluence>().add_systems(
        Update,
        (
            chart_the_sea.run_if(resource_changed::<Land>),
            toggle.run_if(used(Control::Influence)),
            (lay_wakes, paint).chain().run_if(resource_exists::<Chart>),
        ),
    );
}

/// How strongly a port holds the sea at `distance`: all of it near the
/// port, fading to nothing at its reach, which shrinks as its favour for
/// its owner falls.
pub fn port_hold(distance: f32, loyalty: f32) -> f32 {
    let reach = PORT_REACH * (loyalty / OWNER_REST).clamp(0.2, 1.2);
    let fall = (1.0 - distance / reach).max(0.0);
    fall * fall
}

/// The colour a cell is washed with, from each nation's hold on it: the
/// colours of every nation there, mixed by strength, as opaque as the
/// strongest hold. Nobody's sea is clear.
pub fn wash(holds: [f32; NATIONS.len()]) -> Color {
    let total: f32 = holds.iter().sum();
    let strongest = holds.iter().copied().fold(0.0, f32::max).min(1.0);
    if strongest < FAINTEST {
        return Color::NONE;
    }
    let mixed = NATIONS
        .iter()
        .zip(holds)
        .fold(Vec3::ZERO, |sum, (&nation, hold)| {
            sum + nation_colour(nation).to_linear().to_vec3() * hold / total
        });
    Color::LinearRgba(LinearRgba::from_vec3(mixed).with_alpha(MOST_ALPHA * strongest))
}

/// Lays out the grid over the map, and the plane to paint it on.
fn chart_the_sea(
    land: Res<Land>,
    old: Query<Entity, With<InfluenceOverlay>>,
    show: Res<ShowInfluence>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let Some(bounds) = land
        .masses
        .iter()
        .flat_map(|mass| mass.outline.iter())
        .map(|&point| Rect::from_center_size(point, Vec2::ZERO))
        .reduce(|a, b| a.union(b))
    else {
        commands.remove_resource::<Chart>();
        return;
    };
    let bounds = bounds.inflate(MARGIN);
    let cell = bounds.size().max_element() / CELLS as f32;
    let size = (bounds.size() / cell).ceil().as_uvec2();
    let image = images.add(Image::new_fill(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    let mut chart = Chart {
        bounds,
        size,
        wakes: vec![[0.0; NATIONS.len()]; (size.x * size.y) as usize],
        land: Vec::new(),
        image: image.clone(),
        laid_at: 0.0,
        painted_at: f32::NEG_INFINITY,
    };
    chart.land = (0..chart.wakes.len())
        .map(|i| {
            let at = chart.centre(i);
            land.contains(Vec3::new(at.x, 0.0, at.y))
        })
        .collect();
    let centre = bounds.center();
    commands.spawn((
        Name::new("Influence"),
        InfluenceOverlay,
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(bounds.width(), bounds.height()),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(image),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(centre.x, OVERLAY_Y, centre.y),
        if show.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
    ));
    commands.insert_resource(chart);
}

fn toggle(
    mut show: ResMut<ShowInfluence>,
    mut overlay: Query<&mut Visibility, With<InfluenceOverlay>>,
) {
    show.0 = !show.0;
    for mut visibility in &mut overlay {
        *visibility = if show.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Every ship at sea under colours lays wake where she is, and old wakes
/// fade, as game time passes.
fn lay_wakes(
    clock: Res<WorldClock>,
    mut chart: ResMut<Chart>,
    ships: Query<(&Transform, &Colours, Has<Patrol>), Without<Docked>>,
) {
    let days = ((clock.elapsed - chart.laid_at) / DAY).max(0.0);
    chart.laid_at = clock.elapsed;
    if days == 0.0 {
        return;
    }
    let fade = 0.5_f64.powf(days / WAKE_HALF_LIFE) as f32;
    for cell in &mut chart.wakes {
        for wake in cell {
            *wake *= fade;
        }
    }
    for (at, colours, patrol) in &ships {
        if let Some(cell) = chart.cell(to_plane(at.translation)) {
            let weight = if patrol { PATROL_WAKE } else { 1.0 };
            chart.wakes[cell][colours.0.index()] += WAKE_PER_DAY * weight * days as f32;
        }
    }
}

fn paint(
    time: Res<Time<Real>>,
    show: Res<ShowInfluence>,
    mut chart: ResMut<Chart>,
    ports: Query<(&Port, &Favour, &Transform)>,
    mut images: ResMut<Assets<Image>>,
) {
    let now = time.elapsed_secs();
    if !show.0 || now - chart.painted_at < REPAINT_SECONDS {
        return;
    }
    chart.painted_at = now;
    let Some(mut image) = images.get_mut(&chart.image) else {
        return;
    };
    let ports: Vec<_> = ports
        .iter()
        .map(|(port, favour, at)| (port.flag, favour.of(port.flag), to_plane(at.translation)))
        .collect();
    for (i, wakes) in chart.wakes.iter().enumerate() {
        let colour = if chart.land[i] {
            Color::NONE
        } else {
            let at = chart.centre(i);
            let mut holds = wakes.map(|wake| (wake / WAKE_HELD).min(1.0));
            for &(flag, loyalty, port) in &ports {
                let hold = &mut holds[flag.index()];
                *hold = hold.max(port_hold(at.distance(port), loyalty));
            }
            wash(holds)
        };
        let (x, y) = (i as u32 % chart.size.x, i as u32 / chart.size.x);
        // Every cell is inside the image it was sized for.
        let _ = image.set_color_at(x, y, colour);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factions::Nation;

    #[test]
    fn a_port_holds_the_sea_near_it_and_less_when_restless() {
        assert_eq!(port_hold(0.0, OWNER_REST), 1.0);
        assert_eq!(port_hold(PORT_REACH, OWNER_REST), 0.0);
        assert!(port_hold(PORT_REACH / 2.0, 10.0) < port_hold(PORT_REACH / 2.0, OWNER_REST));
    }

    #[test]
    fn the_sea_takes_the_colour_of_whoever_holds_it() {
        assert_eq!(wash([0.0; 3]), Color::NONE, "nobody's sea is clear");
        let english = wash([1.0, 0.0, 0.0]).to_linear();
        let red = nation_colour(Nation::England).to_linear();
        assert_eq!((english.red, english.green), (red.red, red.green));
        assert_eq!(english.alpha, MOST_ALPHA);
        let shared = wash([0.5, 0.5, 0.0]).to_linear();
        assert!(shared.green > english.green, "mixed with Spain's gold");
    }
}
