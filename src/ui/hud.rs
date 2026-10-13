//! On-screen readout of the player's ship and conditions.

use std::fmt::Write;

use bevy::prelude::*;

use crate::math::{MPS_TO_KNOTS, normalize_heading};
use crate::player::Player;
use crate::sailing::{Heading, Speed};
use crate::wind::Wind;

#[derive(Component)]
struct HudText;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_hud)
        .add_systems(Update, update_hud);
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        HudText,
        Text::default(),
        TextFont::from_font_size(18.0),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));
}

fn update_hud(
    mut text: Single<&mut Text, With<HudText>>,
    player: Single<(&Heading, &Speed), With<Player>>,
    wind: Res<Wind>,
    time: Res<Time<Virtual>>,
) {
    let (heading, speed) = *player;
    let text = &mut text.0;
    text.clear();
    // Writing to a String cannot fail.
    let _ = write!(
        text,
        "Heading {:03.0}°   Speed {:.1} kn\nWind {} {:.0} kn\n\nRight-click: sail to   Right-click a port: dock   Space: pause   Scroll: zoom",
        heading.0.to_degrees(),
        speed.0 * MPS_TO_KNOTS,
        compass_point(wind.from),
        wind.speed_mps * MPS_TO_KNOTS,
    );
    if time.is_paused() {
        text.push_str("\n\n— PAUSED —");
    }
}

/// The 16-point compass name for a heading.
fn compass_point(heading: f32) -> &'static str {
    const POINTS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    let sector = std::f32::consts::TAU / POINTS.len() as f32;
    let index = (normalize_heading(heading) / sector).round() as usize % POINTS.len();
    POINTS[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_points_match_headings() {
        assert_eq!(compass_point(0.0), "N");
        assert_eq!(compass_point(67.5_f32.to_radians()), "ENE");
        assert_eq!(compass_point(180_f32.to_radians()), "S");
        assert_eq!(
            compass_point(355_f32.to_radians()),
            "N",
            "wraps back to north"
        );
        assert_eq!(compass_point(-90_f32.to_radians()), "W");
    }
}
