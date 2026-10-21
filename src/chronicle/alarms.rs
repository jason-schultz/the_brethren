//! Alarms: things that look broken, checked once a game day and raised
//! once each, when they start. A ship stuck with a route, a position that
//! isn't a number, a port with less than nothing of a good or a month
//! without one it needs, a war two months long, and the player's standing
//! at its bound.

use std::collections::HashMap;

use bevy::prelude::*;
use chronicle::{Alarm, AlarmKind, Record};

use super::{Chronicle, Noting, new_day};
use crate::clock::{DAY, WorldClock};
use crate::economy::market::Market;
use crate::factions::{HIGHEST, LOWEST, Relations, Standing, groups, pairs};
use crate::goods::Goods;
use crate::navigation::Route;
use crate::player::Player;
use crate::port::Docked;
use crate::ship_class::ShipClass;

/// A ship with a route that's moved less than this in a day, meters, is
/// stuck.
const STUCK_METERS: f32 = 100.0;
/// A port that's had none of a good it needs for this many days is
/// starved of it.
const STARVED_DAYS: f64 = 30.0;
/// Less than this of a good is none.
const NONE_LEFT: f64 = 1.0;
/// A war this many days long is a long war.
const LONG_WAR_DAYS: f64 = 60.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Last, watch.run_if(new_day).in_set(Noting));
}

/// How long each condition has held, by its kind and what it's about.
#[derive(Debug, Default)]
struct Lasting(HashMap<(AlarmKind, String), Held>);

#[derive(Debug)]
struct Held {
    since: f64,
    raised: bool,
}

impl Lasting {
    /// Notes whether `kind`'s condition holds for `subject` on `day`, and
    /// says whether to raise it: once, when it's held for `days`. A
    /// condition that stops holding can be raised again.
    fn raise(&mut self, kind: AlarmKind, subject: &str, holds: bool, day: f64, days: f64) -> bool {
        let key = (kind, subject.to_string());
        if !holds {
            self.0.remove(&key);
            return false;
        }
        let held = self.0.entry(key).or_insert(Held {
            since: day,
            raised: false,
        });
        if held.raised || day - held.since < days {
            return false;
        }
        held.raised = true;
        true
    }
}

fn watch(
    ships: Query<(Entity, &Name, &Transform, Has<Route>, Has<Docked>), With<ShipClass>>,
    ports: Query<(&Name, &Market)>,
    player: Query<&Standing, With<Player>>,
    relations: Res<Relations>,
    goods: Res<Goods>,
    clock: Res<WorldClock>,
    mut lasting: Local<Lasting>,
    mut yesterday: Local<HashMap<Entity, Vec3>>,
    mut chronicle: ResMut<Chronicle>,
) {
    let day = clock.elapsed / DAY;
    let mut check = |alarm, subject: &str, holds, days, detail: String| {
        if lasting.raise(alarm, subject, holds, day, days) {
            chronicle.note(Record::Alarm(Alarm {
                alarm,
                subject: subject.to_string(),
                detail,
            }));
        }
    };
    let mut today = HashMap::new();
    for (ship, name, transform, routed, docked) in &ships {
        let at = transform.translation;
        today.insert(ship, at);
        check(
            AlarmKind::NotFinite,
            name,
            !at.is_finite(),
            0.0,
            format!("at {at}"),
        );
        let unmoved = yesterday
            .get(&ship)
            .is_some_and(|was| was.distance(at) < STUCK_METERS);
        check(
            AlarmKind::Stuck,
            name,
            routed && !docked && unmoved,
            0.0,
            format!(
                "at ({:.0}, {:.0}), with a route, unmoved in a day",
                at.x, at.z
            ),
        );
    }
    *yesterday = today;
    for (port, market) in &ports {
        for entry in &market.entries {
            let subject = format!("{port}: {}", goods.get(entry.good).name);
            check(
                AlarmKind::NegativeStock,
                &subject,
                entry.stock < 0.0,
                0.0,
                format!("{:.1} in stock", entry.stock),
            );
            check(
                AlarmKind::Starved,
                &subject,
                entry.is_needed() && entry.stock < NONE_LEFT,
                STARVED_DAYS,
                format!("none for {STARVED_DAYS} days, wanting {:.0}", entry.target),
            );
        }
    }
    for [a, b] in pairs() {
        check(
            AlarmKind::LongWar,
            &format!("{a} and {b}"),
            relations.at_war(a, b),
            LONG_WAR_DAYS,
            format!(
                "at war for {LONG_WAR_DAYS} days, standing at {:.0}",
                relations.between(a, b)
            ),
        );
    }
    if let Ok(standing) = player.single() {
        for group in groups() {
            let value = standing.of(group);
            check(
                AlarmKind::Pinned,
                &group.to_string(),
                value <= LOWEST || value >= HIGHEST,
                0.0,
                format!("at {value:.0}"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chronicle::Run;
    use crate::chronicle::tests::{noted, noting_app};

    #[test]
    fn an_alarm_is_raised_once_when_it_has_lasted_and_again_if_it_returns() {
        let mut lasting = Lasting::default();
        let mut starved =
            |holds, day| lasting.raise(AlarmKind::Starved, "Nassau", holds, day, 30.0);
        assert!(!starved(true, 0.0));
        assert!(!starved(true, 29.0));
        assert!(starved(true, 30.0));
        assert!(!starved(true, 31.0), "raised once");
        assert!(!starved(false, 32.0));
        assert!(!starved(true, 33.0));
        assert!(starved(true, 63.0), "raised again once it's lasted again");
    }

    fn alarms(app: &mut App) -> Vec<Alarm> {
        noted(app)
            .into_iter()
            .filter_map(|record| match record {
                Record::Alarm(alarm) => Some(alarm),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_ship_with_a_route_that_hasnt_moved_in_a_day_is_stuck() {
        let mut app = noting_app(Run::Soak);
        let route = Route {
            waypoints: [Vec3::new(10_000.0, 0.0, 0.0)].into(),
            on_arrival: default(),
        };
        app.world_mut().spawn((
            Name::new("Dove"),
            ShipClass::sloop(),
            Transform::default(),
            route,
        ));
        app.update();
        assert!(alarms(&mut app).is_empty());
        for _ in 0..2 {
            app.world_mut().resource_mut::<WorldClock>().elapsed += DAY;
            // Hold her where she is, whatever her helm would do.
            let mut ships = app
                .world_mut()
                .query_filtered::<&mut Transform, With<ShipClass>>();
            for mut transform in ships.iter_mut(app.world_mut()) {
                transform.translation = Vec3::ZERO;
            }
            app.update();
        }
        let raised = alarms(&mut app);
        assert_eq!(raised.len(), 1, "{raised:?}");
        assert_eq!(raised[0].alarm, AlarmKind::Stuck);
        assert_eq!(raised[0].subject, "Dove");
    }
}
