//! The records every build notes: what the player does, what she's told,
//! where she is, and how fast time runs. They're cheap, and they're what a
//! crash report needs to say what led up to it.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use chronicle::{Record, Run};

use super::{Chronicle, Noting, crash, name_of};
use crate::alert::{Alert, AlertKind};
use crate::combat::EngageOrder;
use crate::controls::ControlUsed;
use crate::crew::ShoreLeave;
use crate::economy::jobs::{AcceptJob, Job};
use crate::economy::market::{Side, Trade};
use crate::escorts::{AcceptEscort, Escort};
use crate::factions::HoistColours;
use crate::goods::Goods;
use crate::pardons::SeekPardon;
use crate::player::Player;
use crate::port::{CastOff, Docked};
use crate::prizes::BoardOrder;
use crate::service::{Join, Resign};
use crate::shipyard::{BuyShip, RepairOrder, SellPrize, TakeCommand};
use crate::time_scale::TimeScale;
use crate::word::{Fate, GiveAccount};

pub(super) fn plugin(app: &mut App, run: Run) {
    app.configure_sets(Last, Noting).add_systems(
        Last,
        (crash::note_logs, note_alerts, note_orders, note_dock).in_set(Noting),
    );
    // Only someone playing has controls to use and a clock to set.
    if run == Run::Play {
        app.add_systems(Last, (note_controls, note_clock).in_set(Noting));
    }
}

fn note_controls(mut controls: MessageReader<ControlUsed>, mut chronicle: ResMut<Chronicle>) {
    for &ControlUsed(control) in controls.read() {
        chronicle.note(Record::Control {
            control: control.name().to_string(),
        });
    }
}

fn note_alerts(
    mut alerts: MessageReader<Alert>,
    player: Query<Entity, With<Player>>,
    mut chronicle: ResMut<Chronicle>,
) {
    for alert in alerts.read().filter(|alert| player.contains(alert.ship)) {
        chronicle.note(Record::Alert {
            alert: alert_name(alert.kind),
        });
    }
}

/// An alert's name: its variant, as `Debug` begins it. The variant is all a
/// record needs, and the rest of `Debug` would name entities.
fn alert_name(kind: AlertKind) -> String {
    format!("{kind:?}")
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Every order the player can give, as it's given.
#[derive(SystemParam)]
struct Orders<'w, 's> {
    trades: MessageReader<'w, 's, Trade>,
    jobs: MessageReader<'w, 's, AcceptJob>,
    escorts: MessageReader<'w, 's, AcceptEscort>,
    repairs: MessageReader<'w, 's, RepairOrder>,
    purchases: MessageReader<'w, 's, BuyShip>,
    sales: MessageReader<'w, 's, SellPrize>,
    commands: MessageReader<'w, 's, TakeCommand>,
    leave: MessageReader<'w, 's, ShoreLeave>,
    joins: MessageReader<'w, 's, Join>,
    resignations: MessageReader<'w, 's, Resign>,
    pardons: MessageReader<'w, 's, SeekPardon>,
    colours: MessageReader<'w, 's, HoistColours>,
    engagements: MessageReader<'w, 's, EngageOrder>,
    boardings: MessageReader<'w, 's, BoardOrder>,
    accounts: MessageReader<'w, 's, GiveAccount>,
    cast_offs: MessageReader<'w, 's, CastOff>,
}

fn note_orders(
    mut orders: Orders,
    player: Single<Entity, With<Player>>,
    names: Query<&Name>,
    jobs: Query<&Job>,
    escorts: Query<&Escort>,
    goods: Res<Goods>,
    mut chronicle: ResMut<Chronicle>,
) {
    let player = *player;
    let name = |entity| name_of(&names, entity);
    let mut noted = Vec::new();
    for trade in orders.trades.read().filter(|order| order.ship == player) {
        let side = match trade.side {
            Side::Buy => "Buy",
            Side::Sell => "Sell",
        };
        let good = &goods.get(trade.good).name;
        noted.push(format!("{side} {} {good}", trade.quantity));
    }
    for order in orders.jobs.read().filter(|order| order.ship == player) {
        noted.push(jobs.get(order.job).map_or_else(
            |_| "Take a contract".to_string(),
            |job| {
                let good = &goods.get(job.good).name;
                let to = name(job.destination);
                format!("Take a contract: {} {good} to {to}", job.quantity)
            },
        ));
    }
    for order in orders.escorts.read().filter(|order| order.ship == player) {
        noted.push(escorts.get(order.escort).map_or_else(
            |_| "Take an escort".to_string(),
            |escort| format!("Escort the {}", name(escort.ward)),
        ));
    }
    for _ in orders.repairs.read().filter(|order| order.ship == player) {
        noted.push("Repair".to_string());
    }
    for order in orders.purchases.read().filter(|order| order.ship == player) {
        noted.push(format!("Buy a {}", order.class.name));
    }
    for order in orders.sales.read() {
        noted.push(format!("Sell the prize {}", name(order.prize)));
    }
    for order in orders.commands.read() {
        noted.push(format!("Take command of the {}", name(order.prize)));
    }
    for _ in orders.leave.read().filter(|order| order.ship == player) {
        noted.push("Shore leave".to_string());
    }
    for order in orders.joins.read().filter(|order| order.captain == player) {
        noted.push(format!("Join {}", order.group));
    }
    for order in orders
        .resignations
        .read()
        .filter(|order| order.captain == player)
    {
        noted.push(format!("Resign from {}", order.group));
    }
    for order in orders
        .pardons
        .read()
        .filter(|order| order.captain == player)
    {
        noted.push(format!("Seek the pardon of {}", order.nation));
    }
    for order in orders.colours.read().filter(|order| order.ship == player) {
        noted.push(format!("Hoist {}", order.nation.adjective()));
    }
    for order in orders
        .engagements
        .read()
        .filter(|order| order.ship == player)
    {
        noted.push(format!("Engage the {}", name(order.target)));
    }
    for _ in orders.boardings.read().filter(|order| order.ship == player) {
        noted.push("Board".to_string());
    }
    for order in orders.accounts.read().filter(|order| order.ship == player) {
        let says = match order.says {
            Fate::Taken(by) => format!("taken by the {}", name(by)),
            Fate::Sunk(by) => format!("sunk by the {}", name(by)),
            Fate::Wrecked => "wrecked".to_string(),
        };
        noted.push(format!("Tell of the {}: {says}", name(order.lost)));
    }
    for _ in orders.cast_offs.read().filter(|order| order.ship == player) {
        noted.push("Cast off".to_string());
    }
    for order in noted {
        chronicle.note(Record::Order { order });
    }
}

/// Notes the player coming into port and putting to sea.
fn note_dock(
    player: Single<Option<&Docked>, With<Player>>,
    names: Query<&Name>,
    mut was: Local<Option<Option<Entity>>>,
    mut chronicle: ResMut<Chronicle>,
) {
    let port = player.map(|docked| docked.port);
    if *was != Some(port) {
        *was = Some(port);
        chronicle.note(Record::Dock {
            port: port.map(|port| name_of(&names, port)),
        });
    }
}

/// Notes how fast time runs whenever it changes, paused or not.
fn note_clock(
    scale: Res<TimeScale>,
    time: Res<Time<Virtual>>,
    mut was: Local<Option<(f64, bool)>>,
    mut chronicle: ResMut<Chronicle>,
) {
    let now = (scale.speed, time.is_paused());
    if *was != Some(now) {
        *was = Some(now);
        chronicle.note(Record::Clock {
            speed: now.0,
            paused: now.1,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chronicle::tests::{noted, noting_app};
    use crate::economy::market::Side;

    #[test]
    fn an_alert_is_noted_by_its_name_alone() {
        assert_eq!(alert_name(AlertKind::Aground), "Aground");
        assert_eq!(
            alert_name(AlertKind::UnderAttack {
                by: Entity::PLACEHOLDER
            }),
            "UnderAttack"
        );
    }

    /// What's noted of the player's doings, leaving out the rest.
    fn doings(app: &mut App) -> Vec<Record> {
        noted(app)
            .into_iter()
            .filter(|record| {
                matches!(
                    record,
                    Record::Dock { .. } | Record::Order { .. } | Record::Alert { .. }
                )
            })
            .collect()
    }

    #[test]
    fn the_players_orders_and_alerts_are_noted_and_no_one_elses() {
        let mut app = noting_app(Run::Soak);
        let port = app.world_mut().spawn(Name::new("Port Royal")).id();
        let player = app
            .world_mut()
            .spawn((Player, Name::new("Swallow"), Docked { port }))
            .id();
        let other = app.world_mut().spawn(Name::new("Dove")).id();
        let sugar = app.world().resource::<Goods>().id("sugar").unwrap();
        app.update();
        assert_eq!(
            doings(&mut app),
            [Record::Dock {
                port: Some("Port Royal".into())
            }]
        );

        for ship in [player, other] {
            let world = app.world_mut();
            world.write_message(Trade {
                ship,
                good: sugar,
                quantity: 10,
                side: Side::Buy,
            });
            world.write_message(Alert {
                ship,
                kind: AlertKind::ProvisionsLow,
            });
        }
        app.world_mut().entity_mut(player).remove::<Docked>();
        app.update();
        let records = doings(&mut app);
        assert!(records.contains(&Record::Order {
            order: "Buy 10 Sugar".into()
        }));
        assert!(records.contains(&Record::Alert {
            alert: "ProvisionsLow".into()
        }));
        assert!(records.contains(&Record::Dock { port: None }));
        assert_eq!(records.len(), 3, "{records:?}");
    }
}
