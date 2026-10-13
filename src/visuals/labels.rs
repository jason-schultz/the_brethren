//! Screen-space text labels pinned to points in the world (place names).

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::UiSystems;

/// Pins a UI text node over a world position.
#[derive(Component, Debug)]
pub(super) struct WorldLabel {
    anchor: Vec3,
}

/// Visual weight of a label.
#[derive(Debug, Clone, Copy)]
pub(super) enum LabelStyle {
    /// Islands, coasts, and other geography.
    Place,
    /// Towns and ports.
    Port,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        position_labels
            .after(TransformSystems::Propagate)
            .before(UiSystems::Layout),
    );
}

/// A label bundle. Spawn it on its own entity; it needs no parent.
pub(super) fn world_label(text: impl Into<String>, anchor: Vec3, style: LabelStyle) -> impl Bundle {
    let (size, color) = match style {
        LabelStyle::Place => (14.0, Color::srgba(0.95, 0.92, 0.8, 0.85)),
        LabelStyle::Port => (17.0, Color::srgb(1.0, 0.9, 0.55)),
    };
    (
        WorldLabel { anchor },
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(color),
        TextShadow::default(),
        TextLayout::new(Justify::Center, LineBreak::NoWrap),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Pickable::IGNORE,
    )
}

/// Centers each label over its anchor. Labels behind the camera are hidden.
fn position_labels(
    camera: Single<(&Camera, &GlobalTransform)>,
    mut labels: Query<(&WorldLabel, &ComputedNode, &mut Node, &mut Visibility)>,
) {
    let (camera, camera_transform) = *camera;
    for (label, computed, mut node, mut visibility) in &mut labels {
        match camera.world_to_viewport(camera_transform, label.anchor) {
            Ok(screen) => {
                let half_size = computed.size() * computed.inverse_scale_factor() / 2.0;
                node.left = px(screen.x - half_size.x);
                node.top = px(screen.y - half_size.y);
                *visibility = Visibility::Inherited;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }
}
