//! Ports: docking and casting off.
//!
//! A port entity's `Transform` is its anchorage, where ships lie when docked.
//! Ships dock by sailing there with [`Arrival::Dock`].

use bevy::prelude::*;

use crate::navigation::{Arrival, Arrived};
use crate::schedule::{FrameSet, SimSet};

/// A town where ships can dock.
#[derive(Component, Debug, Clone)]
#[require(Transform)]
pub struct Port {
    /// The flag the port answers to. This becomes a faction reference once factions exist.
    pub nation: String,
    /// The town itself, on land.
    pub town: Vec3,
}

/// A ship lying at a port.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Docked {
    pub port: Entity,
}

/// Order a docked ship to leave port.
#[derive(Message, Debug, Clone, Copy)]
pub struct CastOff {
    pub ship: Entity,
}

pub struct PortPlugin;

impl Plugin for PortPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CastOff>()
            .add_systems(FixedUpdate, dock_on_arrival.in_set(SimSet::Resolve))
            // Casting off is an order, so it works while paused.
            .add_systems(Update, cast_off.in_set(FrameSet::Orders));
    }
}

fn dock_on_arrival(mut arrivals: MessageReader<Arrived>, mut commands: Commands) {
    for arrived in arrivals.read() {
        if let Arrival::Dock(port) = arrived.on_arrival {
            commands.entity(arrived.ship).try_insert(Docked { port });
        }
    }
}

fn cast_off(mut orders: MessageReader<CastOff>, mut commands: Commands) {
    for order in orders.read() {
        commands.entity(order.ship).try_remove::<Docked>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::SailOrder;
    use crate::sailing::Hull;
    use crate::test_support::{run_seconds, sim_app};

    fn spawn_port(app: &mut App, anchorage: Vec3) -> Entity {
        app.world_mut()
            .spawn((
                Port {
                    nation: "England".into(),
                    town: anchorage + Vec3::Z * 100.0,
                },
                Transform::from_translation(anchorage),
            ))
            .id()
    }

    #[test]
    fn ship_ordered_to_dock_is_docked_on_arrival() {
        let mut app = sim_app();
        let anchorage = Vec3::new(250.0, 0.0, 0.0);
        let port = spawn_port(&mut app, anchorage);
        let ship = app.world_mut().spawn(Hull::SLOOP).id();
        app.world_mut().write_message(SailOrder {
            ship,
            destination: anchorage,
            on_arrival: Arrival::Dock(port),
        });

        run_seconds(&mut app, 30.0);
        assert!(
            app.world().get::<Docked>(ship).is_none(),
            "docked before arriving"
        );
        run_seconds(&mut app, 300.0);
        assert_eq!(app.world().get::<Docked>(ship), Some(&Docked { port }));
    }

    #[test]
    fn plain_sail_order_does_not_dock() {
        let mut app = sim_app();
        let anchorage = Vec3::new(250.0, 0.0, 0.0);
        spawn_port(&mut app, anchorage);
        let ship = app.world_mut().spawn(Hull::SLOOP).id();
        app.world_mut().write_message(SailOrder {
            ship,
            destination: anchorage,
            on_arrival: Arrival::HeaveTo,
        });
        run_seconds(&mut app, 300.0);
        assert!(app.world().get::<Docked>(ship).is_none());
    }

    #[test]
    fn casting_off_leaves_port_even_while_paused() {
        let mut app = sim_app();
        let port = spawn_port(&mut app, Vec3::ZERO);
        let ship = app.world_mut().spawn((Hull::SLOOP, Docked { port })).id();
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.world_mut().write_message(CastOff { ship });
        app.update();
        assert!(app.world().get::<Docked>(ship).is_none());
    }
}
