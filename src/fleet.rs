//! The player's fleet: her flagship and her prizes, and moving cargo and
//! men between them when they're together.

use bevy::prelude::*;

use crate::cargo::Hold;
use crate::crew::{Complement, Crew};
use crate::economy::Refusal;
use crate::goods::{GoodId, Goods};
use crate::port::Docked;
use crate::prizes::Prize;
use crate::schedule::FrameSet;

/// Ships at sea this close, meters, can pass cargo and men between them.
pub const TOGETHER_AT_SEA: f32 = 200.0;

/// What's moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    /// Tons of a good from the captain's own stock.
    Cargo { good: GoodId, quantity: u32 },
    /// Hands.
    Crew(u32),
}

/// Order cargo or men moved between two ships of the same fleet.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub from: Entity,
    pub to: Entity,
    pub load: Load,
}

/// Whether two ships are together: in the same port, or close at sea.
pub fn together(a: (Vec3, Option<&Docked>), b: (Vec3, Option<&Docked>)) -> bool {
    match (a.1, b.1) {
        (Some(a), Some(b)) => a.port == b.port,
        (None, None) => a.0.distance(b.0) <= TOGETHER_AT_SEA,
        _ => false,
    }
}

/// One side of a transfer.
#[derive(Debug, Clone, Copy)]
pub struct Aboard<'a> {
    pub hold: &'a Hold,
    pub crew: &'a Crew,
    pub complement: &'a Complement,
}

/// Whether `load` can go from one ship to the other. The fleet screen and
/// the transfer itself both use this.
pub fn check_transfer(goods: &Goods, load: Load, from: Aboard, to: Aboard) -> Result<(), Refusal> {
    let (have, room) = match load {
        Load::Cargo { good, .. } => (
            from.hold.owned.get(good),
            to.hold.room(
                goods,
                goods.carriage(good),
                to.crew.count,
                to.complement.berths,
            ),
        ),
        Load::Crew(_) => (
            from.crew.count as f64,
            to.complement.berths as f64 - to.crew.count as f64 - to.hold.passengers(goods),
        ),
    };
    let quantity = match load {
        Load::Cargo { quantity, .. } | Load::Crew(quantity) => quantity as f64,
    };
    if have < quantity {
        Err(Refusal::NotEnough)
    } else if room < quantity {
        Err(Refusal::NoRoom)
    } else {
        Ok(())
    }
}

/// The flagship of the fleet `ship` sails in: her captor if she's a prize,
/// otherwise herself.
pub fn flagship(ship: Entity, prizes: &Query<&Prize>) -> Entity {
    prizes.get(ship).map_or(ship, |prize| prize.captor)
}

pub struct FleetPlugin;

impl Plugin for FleetPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Transfer>()
            // An order, so it works while paused.
            .add_systems(Update, transfer.in_set(FrameSet::Orders));
    }
}

fn transfer(
    mut orders: MessageReader<Transfer>,
    goods: Res<Goods>,
    prizes: Query<&Prize>,
    mut ships: Query<(
        &Transform,
        Option<&Docked>,
        &mut Hold,
        &mut Crew,
        &Complement,
    )>,
) {
    for order in orders.read() {
        if flagship(order.from, &prizes) != flagship(order.to, &prizes) {
            continue;
        }
        let Ok([from, to]) = ships.get_many_mut([order.from, order.to]) else {
            continue;
        };
        let (from_at, from_docked, mut from_hold, mut from_crew, from_complement) = from;
        let (to_at, to_docked, mut to_hold, mut to_crew, to_complement) = to;
        if !together(
            (from_at.translation, from_docked),
            (to_at.translation, to_docked),
        ) {
            continue;
        }
        let aboard = |hold, crew, complement| Aboard {
            hold,
            crew,
            complement,
        };
        if check_transfer(
            &goods,
            order.load,
            aboard(&from_hold, &from_crew, from_complement),
            aboard(&to_hold, &to_crew, to_complement),
        )
        .is_err()
        {
            continue;
        }
        match order.load {
            Load::Cargo { good, quantity } => {
                let moved = from_hold.owned.remove(good, quantity as f64);
                to_hold.owned.add(good, moved);
            }
            Load::Crew(hands) => {
                from_crew.count -= hands;
                to_crew.count += hands;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship_class::ShipClass;

    #[test]
    fn ships_are_together_in_one_port_or_close_at_sea() {
        let [a, b] = [
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
        ];
        let at = |port| Some(Docked { port });
        let here = Vec3::ZERO;
        let far = Vec3::new(5_000.0, 0.0, 0.0);
        assert!(together((here, at(a).as_ref()), (far, at(a).as_ref())));
        assert!(!together((here, at(a).as_ref()), (here, at(b).as_ref())));
        assert!(together((here, None), (Vec3::new(150.0, 0.0, 0.0), None)));
        assert!(!together((here, None), (far, None)));
        assert!(
            !together((here, at(a).as_ref()), (here, None)),
            "one's in port"
        );
    }

    #[test]
    fn transfers_need_goods_on_one_side_and_room_on_the_other() {
        let goods = Goods::standard();
        let rum = goods.id("rum").unwrap();
        let complement = ShipClass::SLOOP.complement;
        let mut laden = Hold::new(40.0);
        laden.owned.add(rum, 35.0);
        let mut empty = Hold::new(40.0);
        empty.owned.add(goods.provisions, 10.0);
        let crew = Crew::new(12);
        let aboard = |hold| Aboard {
            hold,
            crew: &crew,
            complement: &complement,
        };
        let cargo = |quantity| Load::Cargo {
            good: rum,
            quantity,
        };
        assert_eq!(
            check_transfer(&goods, cargo(30), aboard(&laden), aboard(&empty)),
            Ok(())
        );
        assert_eq!(
            check_transfer(&goods, cargo(31), aboard(&laden), aboard(&empty)),
            Err(Refusal::NoRoom),
            "30 tons of room"
        );
        assert_eq!(
            check_transfer(&goods, cargo(1), aboard(&empty), aboard(&laden)),
            Err(Refusal::NotEnough)
        );
        assert_eq!(
            check_transfer(&goods, Load::Crew(12), aboard(&laden), aboard(&empty)),
            Ok(())
        );
        assert_eq!(
            check_transfer(&goods, Load::Crew(13), aboard(&laden), aboard(&empty)),
            Err(Refusal::NotEnough)
        );
    }
}
