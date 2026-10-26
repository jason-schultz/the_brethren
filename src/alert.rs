//! Alerts: moments that need a captain's attention, such as a sail on the
//! horizon or a hull on the rocks. Simulation domains raise them for any
//! ship; the game decides what to do about the player's (slow down, pause).
//!
//! Also the lookout, which raises an alert when another ship comes in sight.

use bevy::ecs::query::QueryData;
use bevy::prelude::*;

use crate::factions::{
    Colours, DeedKind, Flag, Group, GroupKind, Nation, Relations, Standing, Terms,
};
use crate::hunting::{Hunter, Mark, is_quarry};
use crate::loyalty::Rumour;
use crate::officers::Role;
use crate::pardons::Plea;
use crate::patrols::Patrol;
use crate::port::{Docked, Refused};
use crate::sailing::Hull;
use crate::schedule::SimSet;
use crate::service::{Post, StruckOff};
use crate::tuning::TUNING;
use crate::word::Purpose;

/// Ships within this distance (meters) of each other are in sight: about
/// the horizon from a masthead, at the map's half scale.
pub fn sighting_range() -> f32 {
    TUNING.sighting_range
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alert {
    pub ship: Entity,
    pub kind: AlertKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    /// Another ship has come in sight, and whether she'd come for this one.
    SailSighted { other: Entity, hostile: bool },
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
    /// set her captain adrift. Raised for the boat she's left with, and the
    /// port she was put ashore at if no one was left to row it.
    Mutiny {
        leader: Option<Role>,
        ashore: Option<Entity>,
    },
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
    /// Her standing has earned her a new rank.
    Promoted { post: Post },
    /// She's been struck off every group of a nation she served.
    StruckOff { nation: Nation, why: StruckOff },
    /// A governor paid her for an enemy ship, under a letter of marque.
    Bounty { group: Group, amount: u32 },
    /// News: a crown has proclaimed an Act of Grace.
    Grace { nation: Nation },
    /// A crown has pardoned her.
    Pardoned { nation: Nation, plea: Plea },
    /// She ran a group's errand, and it would take her on.
    Introduced { group: Group },
    /// News from a group she serves: a ship that concerns it has sailed.
    Intelligence {
        ship: Entity,
        from: Entity,
        purpose: Purpose,
        group: Group,
    },
    /// The merchant she was escorting came in safe, and paid her `fee`.
    WardSafe { ward: Entity, fee: u32 },
    /// The merchant she was escorting came in without her in sight.
    WardAlone { ward: Entity },
    /// The merchant she was escorting has been lost or taken.
    WardLost,
    /// The merchant she's escorting is under attack.
    WardAttacked { ward: Entity, by: Entity },
    /// She's holed, running hard aground or on a reef, and her timbers
    /// are down to this percent.
    Holed { timbers: u8, on_reef: bool },
    /// Her ship was wrecked or foundered; she and her men took to the boat.
    /// Her wreck, if the water's shallow enough, and the port she was put
    /// ashore at if no one was left to row.
    Wrecked {
        wreck: Option<Entity>,
        ashore: Option<Entity>,
    },
    /// Boarders from `by` took her ship and put her in the boat with the
    /// men they spared; ashore at this port, if no one was left to row.
    Taken { by: Entity, ashore: Option<Entity> },
    /// No one was left to work her ship, so she took to the boat and left
    /// her a derelict; ashore at this port, if no one was left to row.
    Abandoned { ashore: Option<Entity> },
    /// The water is gaining on her pumps.
    Leaking,
    /// Her men are diving on a wreck.
    Salvaging { wreck: Entity },
    /// Word of a blow she struck against a nation has reached it.
    WordOut {
        nation: Nation,
        kind: DeedKind,
        victim: Option<Entity>,
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
                | Self::Promoted { .. }
                | Self::StruckOff { .. }
                | Self::Bounty { .. }
                | Self::Grace { .. }
                | Self::Pardoned { .. }
                | Self::Introduced { .. }
                | Self::Intelligence { .. }
                | Self::WordOut { .. }
                | Self::Salvaging { .. }
                | Self::WardSafe { .. }
                | Self::WardAlone { .. }
                | Self::WardLost
        )
    }

    /// Whether it drops the game to normal speed. Rumours, news, and
    /// word of her service are only noted.
    pub fn slows_time(self) -> bool {
        !matches!(
            self,
            Self::Rumour { .. }
                | Self::Terms { .. }
                | Self::Restless { .. }
                | Self::WentOver { .. }
                | Self::Promoted { .. }
                | Self::StruckOff { .. }
                | Self::Bounty { .. }
                | Self::Grace { .. }
                | Self::Pardoned { .. }
                | Self::Introduced { .. }
                | Self::Intelligence { .. }
                | Self::WordOut { .. }
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

/// A ship keeping a lookout, and what she can tell of herself: whom a
/// hunter in sight would take her for.
#[derive(QueryData)]
#[query_data(mutable)]
struct Lookout {
    entity: Entity,
    at: &'static Transform,
    in_sight: &'static mut InSight,
    colours: Option<&'static Colours>,
    warship: Has<Patrol>,
    standing: Option<&'static Standing>,
}

/// A ship under sail: one with a hull, out of port.
type UnderSail = (With<Hull>, Without<Docked>);

/// Updates who can see whom, and alerts on every ship newly in sight,
/// saying whether she's a hunter that would come for the lookout. Every
/// pair is checked, which is fine for the handful of ships so far. Ships in
/// port neither keep a lookout nor count as sails: a harbour full of
/// merchantmen isn't news.
fn keep_lookout(
    mut lookouts: Query<Lookout, Without<Docked>>,
    ships: Query<(Entity, &Transform, Flag), UnderSail>,
    relations: Res<Relations>,
    mut alerts: MessageWriter<Alert>,
) {
    for LookoutItem {
        entity: ship,
        at: transform,
        mut in_sight,
        colours,
        warship,
        standing,
    } in &mut lookouts
    {
        let position = transform.translation;
        let mark = Mark {
            colours: colours.map(|colours| colours.0),
            warship,
            attacking: None,
            standing,
            salvaging: None,
        };
        let seen = ships.iter().filter(|&(other, other_transform, ..)| {
            other != ship && other_transform.translation.distance(position) <= sighting_range()
        });
        let mut now = Vec::new();
        for (other, _, flag) in seen {
            let (other_colours, serves) = (flag.colours, flag.serves);
            if !in_sight.0.contains(&other) {
                let hunter = other_colours.and_then(|&Colours(nation)| {
                    Hunter::of(
                        nation,
                        serves.map_or(GroupKind::Merchants, |serves| serves.0),
                    )
                });
                let hostile = hunter.is_some_and(|hunter| is_quarry(hunter, &mark, &relations));
                alerts.write(Alert {
                    ship,
                    kind: AlertKind::SailSighted { other, hostile },
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
    use crate::factions::Serves;
    use crate::ship_class::ShipClass;
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
            .spawn((ShipClass::sloop().hull, InSight::default()))
            .id();
        let stranger = app
            .world_mut()
            .spawn((
                ShipClass::sloop().hull,
                Transform::from_xyz(sighting_range() * 2.0, 0.0, 0.0),
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
                kind: AlertKind::SailSighted {
                    other: stranger,
                    hostile: false
                },
            }]
        );
        app.update();
        assert!(sightings(&app).is_empty(), "already in sight");
    }

    #[test]
    fn the_lookout_knows_a_hunter_that_would_come_for_her() {
        let mut app = sim_app();
        app.world_mut().spawn((
            ShipClass::sloop().hull,
            InSight::default(),
            Standing::default(),
            Colours(Nation::ENGLAND),
        ));
        let raider = app
            .world_mut()
            .spawn((
                ShipClass::sloop().hull,
                Colours(Nation::PIRATES),
                Serves(GroupKind::Navy),
            ))
            .id();
        let merchant = app
            .world_mut()
            .spawn((ShipClass::sloop().hull, Colours(Nation::PIRATES)))
            .id();
        let mut seen = Vec::new();
        for _ in 0..2 {
            app.update();
            seen.extend(sightings(&app));
        }
        let hostile = |other| {
            seen.iter().find_map(|alert| match alert.kind {
                AlertKind::SailSighted { other: o, hostile } if o == other => Some(hostile),
                _ => None,
            })
        };
        assert_eq!(hostile(raider), Some(true));
        assert_eq!(hostile(merchant), Some(false), "merchantmen hunt no one");
    }

    #[test]
    fn ships_in_port_are_not_sighted() {
        let mut app = sim_app();
        let port = app.world_mut().spawn_empty().id();
        app.world_mut()
            .spawn((ShipClass::sloop().hull, InSight::default()));
        app.world_mut()
            .spawn((ShipClass::sloop().hull, Docked { port }));
        app.update();
        assert!(sightings(&app).is_empty());
    }
}
