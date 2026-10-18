//! The shipyard: repairs, selling prizes, and changing ships.
//!
//! All three happen at a port with a shipyard, and take effect at once, so
//! they work while paused.

use bevy::prelude::*;

use crate::alert::InSight;
use crate::cargo::{Hold, Purse};
use crate::combat::{FireControl, Timbers};
use crate::crew::{Complement, Crew};
use crate::factions::Colours;
use crate::goods::Goods;
use crate::officers::{Berth, Officer, reassign};
use crate::player::Player;
use crate::port::{Docked, Location, Port};
use crate::prizes::{Prize, reassign_prizes};
use crate::sailing::Rigging;
use crate::schedule::FrameSet;
use crate::ship_class::ShipClass;

/// Cost to make good a whole hull, or a whole rig, as fractions of the
/// ship's price.
const HULL_REPAIR: f64 = 0.4;
const RIG_REPAIR: f64 = 0.2;
/// A prize fetches this fraction of a sound ship's price, before damage.
const PRIZE_DISCOUNT: f64 = 0.6;
/// Her cargo fetches this fraction of its base price, sold with her.
const CARGO_DISCOUNT: f64 = 0.5;

/// Order a docked ship repaired, timbers and rigging both.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairOrder {
    pub ship: Entity,
}

/// Order a prize lying in port with her captor sold.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellPrize {
    pub prize: Entity,
}

/// Order the player to move her flag into a prize lying in port with her.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TakeCommand {
    pub prize: Entity,
}

/// What it costs to make a ship of `class` sound again.
pub fn repair_cost(class: &ShipClass, timbers: f32, rigging: f32) -> u32 {
    let price = class.price as f64;
    let hull = (1.0 - timbers.clamp(0.0, 1.0)) as f64 * HULL_REPAIR;
    let rig = (1.0 - rigging.clamp(0.0, 1.0)) as f64 * RIG_REPAIR;
    (price * (hull + rig)).round() as u32
}

/// What a prize fetches: her hull, less what she needs in repairs, and
/// whatever cargo is still aboard.
pub fn prize_value(
    goods: &Goods,
    class: &ShipClass,
    timbers: f32,
    rigging: f32,
    hold: &Hold,
) -> u32 {
    let hull = class.price as f64 * PRIZE_DISCOUNT;
    let cargo: f64 = hold
        .owned
        .iter()
        .map(|(good, quantity)| goods.get(good).base_price * quantity * CARGO_DISCOUNT)
        .sum();
    (hull + cargo - repair_cost(class, timbers, rigging) as f64)
        .max(0.0)
        .round() as u32
}

/// Whether `port` has a shipyard.
pub fn has_shipyard(port: &Port) -> bool {
    port.locations.contains(&Location::Shipyard)
}

pub struct ShipyardPlugin;

impl Plugin for ShipyardPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RepairOrder>()
            .add_message::<SellPrize>()
            .add_message::<TakeCommand>()
            .add_systems(
                Update,
                (repair, sell_prizes, take_command).in_set(FrameSet::Orders),
            );
    }
}

fn repair(
    mut orders: MessageReader<RepairOrder>,
    ports: Query<&Port>,
    mut ships: Query<(&Docked, &ShipClass, &mut Timbers, &mut Rigging, &mut Purse)>,
) {
    for order in orders.read() {
        let Ok((docked, class, mut timbers, mut rigging, mut purse)) = ships.get_mut(order.ship)
        else {
            continue;
        };
        let cost = repair_cost(class, timbers.0, rigging.0);
        if !ports.get(docked.port).is_ok_and(has_shipyard) || cost > purse.0 {
            continue;
        }
        purse.0 -= cost;
        timbers.0 = 1.0;
        rigging.0 = 1.0;
    }
}

/// A prize is sold where she lies with her captor. Her prize crew rejoins
/// the captor, as many as there are berths for.
fn sell_prizes(
    mut orders: MessageReader<SellPrize>,
    goods: Res<Goods>,
    ports: Query<&Port>,
    prizes: Query<(
        &Prize,
        &Docked,
        &ShipClass,
        &Timbers,
        &Rigging,
        &Hold,
        &Crew,
    )>,
    mut captors: Query<(&Docked, &mut Purse, &mut Crew, &Complement), Without<Prize>>,
    mut commands: Commands,
) {
    for order in orders.read() {
        let Ok((prize, docked, class, timbers, rigging, hold, prize_crew)) =
            prizes.get(order.prize)
        else {
            continue;
        };
        let Ok((captor_docked, mut purse, mut crew, complement)) = captors.get_mut(prize.captor)
        else {
            continue;
        };
        if captor_docked.port != docked.port || !ports.get(docked.port).is_ok_and(has_shipyard) {
            continue;
        }
        purse.0 += prize_value(&goods, class, timbers.0, rigging.0, hold);
        crew.count = (crew.count + prize_crew.count).min(complement.berths);
        commands.entity(order.prize).despawn();
    }
}

/// The player moves her flag, her colours, her purse, her officers, and her lookout into
/// a prize lying in port with her. Her old ship becomes a prize of the new one, and so do
/// the old ship's other prizes. Each ship keeps her own crew and hold.
fn take_command(
    mut orders: MessageReader<TakeCommand>,
    mut prizes: Query<(&mut Prize, &Docked)>,
    mut players: Query<(Entity, &Docked, &mut Purse, Option<&Colours>), With<Player>>,
    mut officers: Query<(&Officer, &mut Berth)>,
    mut commands: Commands,
) {
    for order in orders.read() {
        let Ok((prize, docked)) = prizes.get(order.prize) else {
            continue;
        };
        let Ok((old, player_docked, mut purse, colours)) = players.get_mut(prize.captor) else {
            continue;
        };
        if player_docked.port != docked.port {
            continue;
        }
        let money = std::mem::take(&mut purse.0);
        reassign_prizes(prizes.iter_mut().map(|(prize, _)| prize), old, order.prize);
        reassign(officers.iter_mut(), old, order.prize, |_| true);
        if let Some(&colours) = colours {
            commands.entity(order.prize).insert(colours);
        }
        commands
            .entity(old)
            .remove::<(Player, InSight, Colours)>()
            // Her guns are no longer the captain's to fire.
            .insert((
                Prize {
                    captor: order.prize,
                },
                FireControl::Auto,
            ));
        commands.entity(order.prize).remove::<Prize>().insert((
            Player,
            InSight::default(),
            Purse(money),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOOP: ShipClass = ShipClass::SLOOP;

    #[test]
    fn a_sound_ship_needs_no_repairs_and_a_wreck_costs_most() {
        assert_eq!(repair_cost(&SLOOP, 1.0, 1.0), 0);
        let wreck = repair_cost(&SLOOP, 0.0, 0.0);
        assert!(wreck > repair_cost(&SLOOP, 0.5, 0.5));
        assert!(wreck < SLOOP.price, "cheaper than a new ship");
        assert!(
            repair_cost(&SLOOP, 0.5, 1.0) > repair_cost(&SLOOP, 1.0, 0.5),
            "hulls cost more"
        );
    }

    #[test]
    fn a_prize_is_worth_less_battered_and_more_laden() {
        let goods = Goods::standard();
        let empty = Hold::new(40.0);
        let mut laden = Hold::new(40.0);
        laden.owned.add(goods.id("rum").unwrap(), 20.0);
        let sound = prize_value(&goods, &SLOOP, 1.0, 1.0, &empty);
        assert!(prize_value(&goods, &SLOOP, 0.3, 0.3, &empty) < sound);
        assert!(prize_value(&goods, &SLOOP, 1.0, 1.0, &laden) > sound);
        assert!(sound < SLOOP.price);
    }
}
