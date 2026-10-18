//! Alerts: moments that need a captain's attention, such as a sail on the
//! horizon or a hull on the rocks. Simulation domains raise them for any
//! ship; the game decides what to do about the player's (slow down, pause).
//!
//! Also the lookout, which raises an alert when another ship comes in sight.

use bevy::prelude::*;

use crate::factions::{Nation, Terms};
use crate::loyalty::Rumour;
use crate::officers::Role;
use crate::port::{Docked, Refused};
use crate::sailing::Hull;
use crate::schedule::SimSet;

/// Ships within this distance (meters) of each other are in sight: about
/// the horizon from a masthead, at the map's half scale.
pub const SIGHTING_RANGE: f32 = 8_000.0;

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alert {
    pub ship: Entity,
    pub kind: AlertKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    /// Another ship has come in sight.
    SailSighted { other: Entity },
    /// The ship has run onto land.
    Aground,
    /// Provisions are down to a few days.
    ProvisionsLow,
    /// A contract the ship carries is due within a day.
    DeadlineNear { job: Entity },
    /// Another ship has opened fire on her.
    UnderAttack { by: Entity },
    /// The ship she was fighting has struck her colours.
    EnemyStruck { other: Entity },
    /// The ship she was fighting has gone down.
    EnemySank { other: Entity },
    /// She herself has gone down.
    Sunk,
    /// Her boarders took `other`, and a prize crew sails her.
    PrizeTaken { other: Entity },
    /// Her boarders took and plundered `other`, but had no hands to sail her.
    Plundered { other: Entity },
    /// Her boarders were thrown back from `other`.
    Repulsed { other: Entity },
    /// A port wouldn't let her dock.
    TurnedAway { port: Entity, why: Refused },
    /// A sign of what one of her officers is up to.
    Rumour { officer: Entity, rumour: Rumour },
    /// Her crew rose against her and was put down. The ringleader, if an
    /// officer, is in irons.
    MutinyPutDown { leader: Option<Entity> },
    /// Her crew took the ship, led by her officer in this post if any, and
    /// set her captain adrift. Raised for the boat she's left with.
    Mutiny { leader: Option<Role> },
    /// Her officer in this post sold her out at `port`: her ship is
    /// seized. Raised for the boat she's left with.
    Betrayed { role: Role, port: Entity },
    /// News: two nations have come to new terms.
    Terms { nations: [Nation; 2], terms: Terms },
    /// News: a port has turned against the nation that holds it.
    Restless { port: Entity },
    /// News: a port has gone over from one nation to another.
    WentOver {
        port: Entity,
        from: Nation,
        to: Nation,
    },
}

impl AlertKind {
    /// Whether the captain must decide something now, so the game pauses.
    /// Otherwise it only drops to normal speed.
    pub fn needs_decision(self) -> bool {
        !matches!(
            self,
            Self::SailSighted { .. }
                | Self::UnderAttack { .. }
                | Self::EnemySank { .. }
                | Self::PrizeTaken { .. }
                | Self::Plundered { .. }
                | Self::Repulsed { .. }
                | Self::TurnedAway { .. }
                | Self::Rumour { .. }
                | Self::Terms { .. }
                | Self::Restless { .. }
                | Self::WentOver { .. }
        )
    }

    /// Whether it drops the game to normal speed. Rumours and news are
    /// only noted.
    pub fn slows_time(self) -> bool {
        !matches!(
            self,
            Self::Rumour { .. }
                | Self::Terms { .. }
                | Self::Restless { .. }
                | Self::WentOver { .. }
        )
    }
}

/// Whether a value dropped below `threshold` between two readings. Alerts
/// fire on the crossing, not on every step spent below it.
pub fn fell_below(before: f64, after: f64, threshold: f64) -> bool {
    before >= threshold && after < threshold
}

/// The ships a ship can currently see.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct InSight(pub Vec<Entity>);

pub struct AlertPlugin;

impl Plugin for AlertPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Alert>()
            .add_systems(FixedUpdate, keep_lookout.in_set(SimSet::Resolve));
    }
}

/// Updates who can see whom, and alerts on every ship newly in sight. Every
/// pair is checked, which is fine for the handful of ships so far. Ships in
/// port neither keep a lookout nor count as sails: a harbour full of
/// merchantmen isn't news.
fn keep_lookout(
    mut lookouts: Query<(Entity, &Transform, &mut InSight), Without<Docked>>,
    ships: Query<(Entity, &Transform), (With<Hull>, Without<Docked>)>,
    mut alerts: MessageWriter<Alert>,
) {
    for (ship, transform, mut in_sight) in &mut lookouts {
        let position = transform.translation;
        let seen = ships.iter().filter(|&(other, other_transform)| {
            other != ship && other_transform.translation.distance(position) <= SIGHTING_RANGE
        });
        let mut now = Vec::new();
        for (other, _) in seen {
            if !in_sight.0.contains(&other) {
                alerts.write(Alert {
                    ship,
                    kind: AlertKind::SailSighted { other },
                });
            }
            now.push(other);
        }
        in_sight.set_if_neq(InSight(now));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sim_app;

    #[test]
    fn falling_below_fires_once_on_the_crossing() {
        assert!(fell_below(3.0, 2.9, 3.0));
        assert!(!fell_below(2.9, 2.8, 3.0), "already below");
        assert!(!fell_below(3.5, 3.1, 3.0), "still above");
        assert!(!fell_below(2.0, 4.0, 3.0), "rising");
    }

    fn sightings(app: &App) -> Vec<Alert> {
        app.world()
            .resource::<Messages<Alert>>()
            .iter_current_update_messages()
            .copied()
            .collect()
    }

    #[test]
    fn a_ship_coming_in_sight_is_sighted_once() {
        let mut app = sim_app();
        let lookout = app
            .world_mut()
            .spawn((Hull::SLOOP, InSight::default()))
            .id();
        let stranger = app
            .world_mut()
            .spawn((
                Hull::SLOOP,
                Transform::from_xyz(SIGHTING_RANGE * 2.0, 0.0, 0.0),
            ))
            .id();
        app.update();
        assert!(sightings(&app).is_empty(), "out of sight");

        app.world_mut()
            .get_mut::<Transform>(stranger)
            .unwrap()
            .translation
            .x = 100.0;
        app.update();
        assert_eq!(
            sightings(&app),
            vec![Alert {
                ship: lookout,
                kind: AlertKind::SailSighted { other: stranger },
            }]
        );
        app.update();
        assert!(sightings(&app).is_empty(), "already in sight");
    }

    #[test]
    fn ships_in_port_are_not_sighted() {
        let mut app = sim_app();
        let port = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((Hull::SLOOP, InSight::default()));
        app.world_mut().spawn((Hull::SLOOP, Docked { port }));
        app.update();
        assert!(sightings(&app).is_empty());
    }
}
