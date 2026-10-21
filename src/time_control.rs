//! The captain's control of time: pause, time compression, and being woken
//! when something happens.
//!
//! Space pauses; minus and equals step the speed down and up, and the HUD's
//! buttons pick one (see `controls`). An arrival or a sail in sight drops
//! the game back to normal speed. An alert that needs a decision, such as
//! running aground, also pauses it.

use bevy::prelude::*;

use crate::alert::Alert;
use crate::controls::{Control, ControlUsed};
use crate::navigation::Arrived;
use crate::player::Player;
use crate::schedule::FrameSet;
use crate::time_scale::{SPEEDS, TimeScale};

/// The alert that last stopped the clock, shown until the captain resumes.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LatestAlert(pub Option<Alert>);

pub struct TimeControlPlugin;

impl Plugin for TimeControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LatestAlert>().add_systems(
            Update,
            // Alerts come last, so one arriving this frame wins over a key press.
            (control_time, heed_alerts).chain().in_set(FrameSet::Input),
        );
    }
}

/// The speed a control picks, starting from `speed`, if it picks one.
pub fn speed_after(control: Control, speed: f64) -> Option<f64> {
    let index = SPEEDS.iter().position(|&s| s == speed).unwrap_or(0);
    let picked = match control {
        Control::Speed(index) => index,
        Control::Slower => index.saturating_sub(1),
        Control::Faster => index + 1,
        _ => return None,
    };
    SPEEDS.get(picked.min(SPEEDS.len() - 1)).copied()
}

/// Picks speeds and pauses. Picking a speed, or resuming, also acknowledges
/// the last alert.
fn control_time(
    mut controls: MessageReader<ControlUsed>,
    mut scale: ResMut<TimeScale>,
    mut time: ResMut<Time<Virtual>>,
    mut latest: ResMut<LatestAlert>,
) {
    for &ControlUsed(control) in controls.read() {
        if control == Control::Pause {
            time.toggle();
            if !time.is_paused() {
                latest.0 = None;
            }
        } else if let Some(speed) = speed_after(control, scale.speed) {
            scale.set_if_neq(TimeScale { speed });
            latest.0 = None;
        }
    }
}

fn heed_alerts(
    mut arrivals: MessageReader<Arrived>,
    mut alerts: MessageReader<Alert>,
    player: Single<Entity, With<Player>>,
    mut scale: ResMut<TimeScale>,
    mut time: ResMut<Time<Virtual>>,
    mut latest: ResMut<LatestAlert>,
) {
    let normal = TimeScale { speed: SPEEDS[0] };
    if arrivals.read().any(|arrived| arrived.ship == *player) {
        scale.set_if_neq(normal);
    }
    if let Some(alert) = alerts
        .read()
        .filter(|alert| alert.ship == *player && alert.kind.slows_time())
        .last()
    {
        scale.set_if_neq(normal);
        if alert.kind.needs_decision() {
            time.pause();
        }
        latest.0 = Some(*alert);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::AlertKind;
    use crate::navigation::Arrival;
    use crate::test_support::sim_app;

    fn app() -> (App, Entity) {
        let mut app = sim_app();
        app.add_message::<ControlUsed>()
            .add_plugins(TimeControlPlugin);
        let player = app.world_mut().spawn(Player).id();
        app.update();
        app.world_mut().resource_mut::<TimeScale>().speed = SPEEDS[4];
        app.update();
        (app, player)
    }

    fn speed(app: &App) -> f64 {
        app.world().resource::<TimeScale>().speed
    }

    fn control(app: &mut App, control: Control) {
        app.world_mut().write_message(ControlUsed(control));
        app.update();
    }

    #[test]
    fn controls_pick_and_step_the_speed_within_bounds() {
        let (mut app, _) = app();
        control(&mut app, Control::Speed(2));
        assert_eq!(speed(&app), SPEEDS[2]);
        control(&mut app, Control::Faster);
        assert_eq!(speed(&app), SPEEDS[3]);
        control(&mut app, Control::Slower);
        control(&mut app, Control::Slower);
        assert_eq!(speed(&app), SPEEDS[1]);
        assert_eq!(speed_after(Control::Slower, SPEEDS[0]), Some(SPEEDS[0]));
        assert_eq!(speed_after(Control::Faster, SPEEDS[4]), Some(SPEEDS[4]));
        assert_eq!(speed_after(Control::Board, SPEEDS[0]), None);
    }

    #[test]
    fn pause_toggles_the_clock() {
        let (mut app, _) = app();
        control(&mut app, Control::Pause);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        control(&mut app, Control::Pause);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    #[test]
    fn an_alert_for_the_player_drops_to_normal_speed_and_pauses() {
        let (mut app, player) = app();
        let alert = Alert {
            ship: player,
            kind: AlertKind::Aground,
        };
        app.world_mut().write_message(alert);
        app.update();
        assert_eq!(speed(&app), SPEEDS[0]);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        assert_eq!(app.world().resource::<LatestAlert>().0, Some(alert));
    }

    #[test]
    fn a_sail_in_sight_drops_to_normal_speed_without_pausing() {
        let (mut app, player) = app();
        let other = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(Alert {
            ship: player,
            kind: AlertKind::SailSighted {
                other,
                hostile: false,
            },
        });
        app.update();
        assert_eq!(speed(&app), SPEEDS[0]);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        assert!(app.world().resource::<LatestAlert>().0.is_some());

        control(&mut app, Control::Speed(4));
        assert_eq!(
            app.world().resource::<LatestAlert>().0,
            None,
            "acknowledged"
        );
    }

    #[test]
    fn arriving_drops_to_normal_speed_without_pausing() {
        let (mut app, player) = app();
        app.world_mut().write_message(Arrived {
            ship: player,
            on_arrival: Arrival::HeaveTo,
        });
        app.update();
        assert_eq!(speed(&app), SPEEDS[0]);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    #[test]
    fn other_ships_alerts_are_none_of_the_players_business() {
        let (mut app, _) = app();
        let stranger = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(Alert {
            ship: stranger,
            kind: AlertKind::Aground,
        });
        app.update();
        assert_eq!(speed(&app), SPEEDS[4]);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }
}
