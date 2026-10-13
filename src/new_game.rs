//! Starting a new game: load the opening map and put the player's ship on it.

use bevy::prelude::*;

use crate::map::data::{MapData, PORT_ROYAL};
use crate::map::load_map;
use crate::player::Player;
use crate::sailing::{Heading, Hull};

pub struct NewGamePlugin;

impl Plugin for NewGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_new_game);
    }
}

fn start_new_game(mut commands: Commands) {
    let map = MapData::parse(PORT_ROYAL).expect("the embedded map is validated by map tests");
    let start = load_map(&mut commands, &map);
    commands.spawn((
        Name::new("Player Sloop"),
        Player,
        Hull::SLOOP,
        Heading(start.heading),
        Transform::from_translation(start.position),
    ));
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::map::{Land, PlayerStart};
    use crate::navigation::{Arrival, SailOrder};
    use crate::port::{Docked, Port};
    use crate::test_support::{run_checked, sim_app};

    #[test]
    fn player_can_sail_from_the_start_and_dock_at_port_royal() {
        let mut app = sim_app();
        let start: PlayerStart = app
            .world_mut()
            .run_system_once(|mut commands: Commands| {
                let map = MapData::parse(PORT_ROYAL).unwrap();
                load_map(&mut commands, &map)
            })
            .unwrap();
        let ship = app
            .world_mut()
            .spawn((
                Hull::SLOOP,
                Heading(start.heading),
                Transform::from_translation(start.position),
            ))
            .id();
        let (port, anchorage) = app
            .world_mut()
            .query_filtered::<(Entity, &Transform), With<Port>>()
            .single(app.world())
            .map(|(entity, transform)| (entity, transform.translation))
            .unwrap();
        app.world_mut().write_message(SailOrder {
            ship,
            destination: anchorage,
            on_arrival: Arrival::Dock(port),
        });

        run_checked(&mut app, 3600.0, |world| {
            let position = world.get::<Transform>(ship).unwrap().translation;
            let room = world.resource::<Land>().clearance(position, position);
            assert!(room > 10.0, "came within {room} m of land at {position}");
        });
        assert_eq!(app.world().get::<Docked>(ship), Some(&Docked { port }));
    }
}
