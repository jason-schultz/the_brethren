//! The player's ship and the player's orders, including how she likes to
//! fight.

use bevy::prelude::*;

use crate::camera::Pointer;
use crate::combat::gunnery::Broadside;
use crate::combat::{Battery, EngageOrder, FireControl, FireOrder, Sunk};
use crate::controls::{Control, ControlUsed, used};
use crate::escorts::Escort;
use crate::navigation::{Arrival, KeepCompany, SailOrder};
use crate::port::{Docked, Port};
use crate::prizes::BoardOrder;
use crate::schedule::FrameSet;
use crate::ui::Noticeboard;
use crate::wrecks::Wreck;

/// Marks the ship the player commands.
#[derive(Component, Debug)]
pub struct Player;

/// How the player fights. A preference, not part of the world: the battle
/// style control cycles it.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BattleStyle {
    /// Steer, and the guns fire as they bear.
    #[default]
    Auto,
    /// Steer, and fire each broadside yourself.
    Manual,
    /// The world pauses, and the battle is fought by rounds on a screen.
    Abstract,
}

impl BattleStyle {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Manual,
            Self::Manual => Self::Abstract,
            Self::Abstract => Self::Auto,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
            Self::Abstract => "abstract",
        }
    }

    fn fire_control(self) -> FireControl {
        match self {
            // Battles by rounds stop the world; between them, the guns fire
            // on their own.
            Self::Auto | Self::Abstract => FireControl::Auto,
            Self::Manual => FireControl::Manual { order: None },
        }
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattleStyle>().add_systems(
            Update,
            (
                order_sail_on_click,
                order_broadsides,
                cycle_battle_style.run_if(used(Control::CycleBattleStyle)),
                apply_battle_style,
            )
                .chain()
                .in_set(FrameSet::Input),
        );
    }
}

fn cycle_battle_style(mut style: ResMut<BattleStyle>) {
    *style = style.next();
}

/// Puts the player's guns under the chosen style. Also covers a player
/// ship that was just spawned or loaded.
fn apply_battle_style(style: Res<BattleStyle>, mut player: Query<&mut FireControl, With<Player>>) {
    let wanted = style.fire_control();
    for mut control in &mut player {
        // Compare the style only, so a queued manual order survives.
        if std::mem::discriminant(&*control) != std::mem::discriminant(&wanted) {
            *control = wanted;
        }
    }
}

/// The fire controls fire a broadside, under manual fire control, and the
/// board control boards the enemy alongside.
fn order_broadsides(
    mut controls: MessageReader<ControlUsed>,
    player: Single<Entity, (With<Player>, Without<Sunk>)>,
    mut orders: MessageWriter<FireOrder>,
    mut boardings: MessageWriter<BoardOrder>,
) {
    let ship = *player;
    for &ControlUsed(control) in controls.read() {
        match control {
            Control::Board => {
                boardings.write(BoardOrder { ship });
            }
            Control::FirePort => {
                orders.write(FireOrder {
                    ship,
                    side: Broadside::Port,
                });
            }
            Control::FireStarboard => {
                orders.write(FireOrder {
                    ship,
                    side: Broadside::Starboard,
                });
            }
            _ => {}
        }
    }
}

/// The player's ship at sea and afloat.
type PlayerAtSea = (With<Player>, Without<Docked>, Without<Sunk>);

/// Other armed ships than the player's.
type OtherArmed = (With<Battery>, Without<Player>);

/// Right-click on the ocean orders the player's ship to sail there. Right-click
/// near another ship at sea engages her, or keeps company with her if she's
/// the player's ward; near a port, sails there to dock; near a wreck, lies
/// over her to salvage. Any other order
/// ends her company. A left click looks a ship over instead. Orders aren't
/// given from the ocean view while docked; the port screen handles leaving.
fn order_sail_on_click(
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Pointer,
    player: Single<Entity, PlayerAtSea>,
    ships: Query<(Entity, &Name, &Transform, Has<Docked>), OtherArmed>,
    ports: Query<(Entity, &Port, &Transform)>,
    wrecks: Query<&Transform, With<Wreck>>,
    mut orders: MessageWriter<SailOrder>,
    mut engagements: MessageWriter<EngageOrder>,
    (wards, mut company): (Query<&Escort>, MessageWriter<KeepCompany>),
    mut notices: Noticeboard,
) {
    if !buttons.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(clicked) = pointer.on_ocean() else {
        return;
    };
    let near = |point: Vec3| pointer.near(point);
    // Ships at sea first: one may be passing the anchorage.
    let mut clicked_ships: Vec<_> = ships
        .iter()
        .filter(|(.., at, _)| near(at.translation))
        .collect();
    clicked_ships.sort_by_key(|&(.., docked)| docked);
    let ship = *player;
    let ward = |target| {
        wards
            .iter()
            .any(|escort| escort.ward == target && escort.escort == Some(ship))
    };
    match clicked_ships.first() {
        Some(&(target, _, _, false)) if ward(target) => {
            company.write(KeepCompany {
                ship,
                with: Some(target),
            });
            return;
        }
        Some(&(target, _, _, false)) => {
            engagements.write(EngageOrder { ship, target });
            return;
        }
        Some(&(_, name, _, true)) => notices.post(format!(
            "{name} lies in port, under the guns of the fort. Catch her at sea."
        )),
        None => {}
    }
    company.write(KeepCompany { ship, with: None });
    let order = match ports
        .iter()
        .find(|(_, port, anchorage)| near(port.town) || near(anchorage.translation))
    {
        Some((port, _, anchorage)) => SailOrder {
            ship: *player,
            destination: anchorage.translation,
            on_arrival: Arrival::Dock(port),
        },
        // Over a wreck, she lies where the divers can go down.
        None => SailOrder {
            ship: *player,
            destination: wrecks
                .iter()
                .map(|lies| lies.translation)
                .find(|&lies| near(lies))
                .unwrap_or(clicked),
            on_arrival: Arrival::HeaveTo,
        },
    };
    orders.write(order);
}
