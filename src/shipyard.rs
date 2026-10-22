//! The shipyard: repairs, selling prizes, changing ships, and buying new
//! ones.
//!
//! All of them happen at a port with a shipyard, and take effect at once,
//! so they work while paused. The other captains' ships are refitted
//! while they lie at a yard, a little each day, at their owners' charge.

use bevy::ecs::query::QueryData;
use bevy::prelude::*;

use crate::alert::InSight;
use crate::cargo::{Hold, Purse};
use crate::clock::DAY;
use crate::combat::{FireControl, Timbers};
use crate::crew::{Complement, Crew};
use crate::economy::Refusal;
use crate::factions::{CaptainRecord, Colours};
use crate::goods::Goods;
use crate::officers::{Berth, Officer, reassign};
use crate::player::Player;
use crate::port::{Docked, Location, Port};
use crate::prizes::{Prize, reassign_prizes};
use crate::sailing::Rigging;
use crate::schedule::{FrameSet, SimSet};
use crate::ship_class::ShipClass;

/// Cost to make good a whole hull, or a whole rig, as fractions of the
/// ship's price.
const HULL_REPAIR: f64 = 0.4;
const RIG_REPAIR: f64 = 0.2;
/// A prize fetches this fraction of a sound ship's price, before damage.
const PRIZE_DISCOUNT: f64 = 0.6;
/// Her cargo fetches this fraction of its base price, sold with her.
const CARGO_DISCOUNT: f64 = 0.5;
/// How much of a whole hull and rig a yard makes good in a day on the other
/// captains' ships: a wreck is sound again in a couple of days in port.
const REFIT_PER_DAY: f32 = 0.5;

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

/// Order a docked ship exchanged for a new one of `class`.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct BuyShip {
    pub ship: Entity,
    pub class: ShipClass,
}

/// A ship offered in part exchange, as the yard sees her.
#[derive(Debug, Clone, Copy)]
pub struct PartExchange<'a> {
    pub class: &'a ShipClass,
    pub timbers: f32,
    pub rigging: f32,
    pub hold: &'a Hold,
    pub crew: &'a Crew,
}

/// What it costs to make a ship of `class` sound again.
pub fn repair_cost(class: &ShipClass, timbers: f32, rigging: f32) -> u32 {
    let price = class.price as f64;
    let hull = (1.0 - timbers.clamp(0.0, 1.0)) as f64 * HULL_REPAIR;
    let rig = (1.0 - rigging.clamp(0.0, 1.0)) as f64 * RIG_REPAIR;
    (price * (hull + rig)).round() as u32
}

/// What a ship of `class` fetches without her cargo: her hull, less what
/// she needs in repairs.
fn hull_value(class: &ShipClass, timbers: f32, rigging: f32) -> u32 {
    let hull = class.price as f64 * PRIZE_DISCOUNT;
    (hull - repair_cost(class, timbers, rigging) as f64)
        .max(0.0)
        .round() as u32
}

/// What a prize fetches: her hull, and whatever cargo is still aboard.
pub fn prize_value(
    goods: &Goods,
    class: &ShipClass,
    timbers: f32,
    rigging: f32,
    hold: &Hold,
) -> u32 {
    let cargo: f64 = hold
        .owned
        .iter()
        .map(|(good, quantity)| goods.get(good).base_price * quantity * CARGO_DISCOUNT)
        .sum();
    hull_value(class, timbers, rigging) + cargo.round() as u32
}

/// What a new ship of `class` costs at `port`, with `old` in part
/// exchange at what she'd fetch as a prize without her cargo: negative
/// when the yard owes the difference. Or why the yard won't sell her: no
/// shipyard here, no berths for everyone aboard, no room for the cargo, or
/// not the money. The port screen and the order both use this.
pub fn check_purchase(
    goods: &Goods,
    port: &Port,
    class: &ShipClass,
    old: PartExchange,
    purse: Purse,
) -> Result<i64, Refusal> {
    if !has_shipyard(port) {
        return Err(Refusal::NotTraded);
    }
    let aboard = old.crew.count as f64 + old.hold.passengers(goods);
    if aboard > class.complement.berths as f64 || old.hold.tons_used(goods) > class.hold_tons {
        return Err(Refusal::NoRoom);
    }
    let cost = class.price as i64 - hull_value(old.class, old.timbers, old.rigging) as i64;
    if cost > purse.0 as i64 {
        return Err(Refusal::CantAfford);
    }
    Ok(cost)
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
            .add_message::<BuyShip>()
            .add_systems(
                Update,
                (repair, sell_prizes, take_command, buy_ships).in_set(FrameSet::Orders),
            )
            .add_systems(FixedUpdate, refit.in_set(SimSet::Economy));
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

/// The other captains' ships: not the player's, and not prizes.
type OthersShips = (Without<Player>, Without<Prize>);

/// A ship offered in part exchange, with her owner's hold and purse.
#[derive(QueryData)]
#[query_data(mutable)]
struct PartExchanged {
    docked: &'static Docked,
    class: &'static ShipClass,
    timbers: &'static Timbers,
    rigging: &'static Rigging,
    hold: &'static mut Hold,
    crew: &'static Crew,
    purse: &'static mut Purse,
}

/// The other captains' ships lying at a yard are made good, a little at a
/// time. The player pays for her repairs, and prizes are sold as they lie.
fn refit(
    time: Res<Time>,
    ports: Query<&Port>,
    mut ships: Query<(&Docked, &mut Timbers, &mut Rigging), OthersShips>,
) {
    let mended = REFIT_PER_DAY * (time.delta_secs_f64() / DAY) as f32;
    for (docked, mut timbers, mut rigging) in &mut ships {
        if !ports.get(docked.port).is_ok_and(has_shipyard) {
            continue;
        }
        if timbers.0 < 1.0 {
            timbers.0 = (timbers.0 + mended).min(1.0);
        }
        if rigging.0 < 1.0 {
            rigging.0 = (rigging.0 + mended).min(1.0);
        }
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
            .move_components::<CaptainRecord>(order.prize)
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

/// The yard takes her old ship and fits her company into a new one: her
/// crew, cargo, officers, prizes, and record stay as they were, and so does
/// her name.
fn buy_ships(
    mut orders: MessageReader<BuyShip>,
    goods: Res<Goods>,
    ports: Query<&Port>,
    mut ships: Query<PartExchanged>,
    mut commands: Commands,
) {
    for order in orders.read() {
        let Ok(PartExchangedItem {
            docked,
            class: old,
            timbers,
            rigging,
            mut hold,
            crew,
            mut purse,
        }) = ships.get_mut(order.ship)
        else {
            continue;
        };
        let Ok(port) = ports.get(docked.port) else {
            continue;
        };
        let old = PartExchange {
            class: old,
            timbers: timbers.0,
            rigging: rigging.0,
            hold: &hold,
            crew,
        };
        let Ok(cost) = check_purchase(&goods, port, &order.class, old, *purse) else {
            continue;
        };
        purse.0 = (purse.0 as i64 - cost) as u32;
        hold.capacity_tons = order.class.hold_tons;
        commands.entity(order.ship).insert(order.class.new_hull());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Battery;
    use crate::sailing::Hull;
    use crate::test_support::{run_seconds, sim_app};
    use crate::time_scale::MAX_STEP;

    #[test]
    fn the_other_captains_ships_are_refitted_at_a_yard_and_the_players_is_not() {
        let mut app = sim_app();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let at = |app: &mut App, port: Port| app.world_mut().spawn(port).id();
        let (yard, no_yard) = (
            at(&mut app, yard()),
            at(
                &mut app,
                Port {
                    locations: vec![Location::Market],
                    ..yard()
                },
            ),
        );
        let battered = |app: &mut App, port, player| {
            let mut ship = app
                .world_mut()
                .spawn((Docked { port }, Timbers(0.1), Rigging(0.3)));
            if player {
                ship.insert(Player);
            }
            ship.id()
        };
        let other = battered(&mut app, yard, false);
        let elsewhere = battered(&mut app, no_yard, false);
        let player = battered(&mut app, yard, true);
        run_seconds(&mut app, (3.0 * DAY) as f32);
        let world = app.world();
        let sound = |ship| {
            (
                world.get::<Timbers>(ship).unwrap().0,
                world.get::<Rigging>(ship).unwrap().0,
            )
        };
        assert_eq!(sound(other), (1.0, 1.0));
        assert_eq!(sound(elsewhere), (0.1, 0.3), "no yard, no refit");
        assert_eq!(sound(player), (0.1, 0.3), "she pays for hers");
    }

    #[test]
    fn a_sound_ship_needs_no_repairs_and_a_wreck_costs_most() {
        assert_eq!(repair_cost(&ShipClass::sloop(), 1.0, 1.0), 0);
        let wreck = repair_cost(&ShipClass::sloop(), 0.0, 0.0);
        assert!(wreck > repair_cost(&ShipClass::sloop(), 0.5, 0.5));
        assert!(wreck < ShipClass::sloop().price, "cheaper than a new ship");
        assert!(
            repair_cost(&ShipClass::sloop(), 0.5, 1.0) > repair_cost(&ShipClass::sloop(), 1.0, 0.5),
            "hulls cost more"
        );
    }

    #[test]
    fn a_prize_is_worth_less_battered_and_more_laden() {
        let goods = Goods::standard();
        let empty = Hold::new(40.0);
        let mut laden = Hold::new(40.0);
        laden.owned.add(goods.id("rum").unwrap(), 20.0);
        let sound = prize_value(&goods, &ShipClass::sloop(), 1.0, 1.0, &empty);
        assert!(prize_value(&goods, &ShipClass::sloop(), 0.3, 0.3, &empty) < sound);
        assert!(prize_value(&goods, &ShipClass::sloop(), 1.0, 1.0, &laden) > sound);
        assert!(sound < ShipClass::sloop().price);
    }

    fn yard() -> Port {
        Port {
            flag: crate::factions::Nation::ENGLAND,
            town: Vec3::ZERO,
            locations: vec![Location::Shipyard],
        }
    }

    fn class(name: &str) -> ShipClass {
        ShipClass::named(name).unwrap()
    }

    /// What a `class` costs a sloop with `crew` men, `hold`, and
    /// `timbers`, and a purse of `purse`.
    fn price(
        class: &ShipClass,
        crew: u32,
        hold: &Hold,
        timbers: f32,
        purse: u32,
    ) -> Result<i64, Refusal> {
        let crew = Crew::new(crew);
        let old = PartExchange {
            class: &ShipClass::sloop(),
            timbers,
            rigging: 1.0,
            hold,
            crew: &crew,
        };
        check_purchase(&Goods::standard(), &yard(), class, old, Purse(purse))
    }

    #[test]
    fn her_old_ship_goes_in_part_exchange() {
        let hold = ShipClass::sloop().empty_hold();
        let brig = class("Brig");
        let cost = price(&brig, 12, &hold, 1.0, 100_000).unwrap();
        assert!(0 < cost && cost < brig.price as i64, "{cost}");
        let battered = price(&brig, 12, &hold, 0.3, 100_000).unwrap();
        assert!(battered > cost, "a battered ship is worth less");
        assert!(
            price(&class("Longboat"), 4, &hold, 1.0, 0).unwrap() < 0,
            "the yard pays the difference for a smaller ship"
        );
    }

    #[test]
    fn the_yard_sells_only_what_she_can_pay_for_and_fill() {
        let goods = Goods::standard();
        let empty = ShipClass::sloop().empty_hold();
        let brig = class("Brig");
        assert_eq!(price(&brig, 12, &empty, 1.0, 100), Err(Refusal::CantAfford));
        let schooner = class("Schooner");
        assert_eq!(
            price(&schooner, 70, &empty, 1.0, 100_000),
            Err(Refusal::NoRoom),
            "too few berths for her crew"
        );
        let mut laden = ShipClass::sloop().empty_hold();
        laden.owned.add(goods.id("rum").unwrap(), 35.0);
        assert_eq!(
            price(&schooner, 12, &laden, 1.0, 100_000),
            Err(Refusal::NoRoom),
            "too small a hold for her cargo"
        );
        assert!(price(&brig, 12, &laden, 1.0, 100_000).is_ok());
        let crew = Crew::new(12);
        let old = PartExchange {
            class: &ShipClass::sloop(),
            timbers: 1.0,
            rigging: 1.0,
            hold: &empty,
            crew: &crew,
        };
        let no_yard = Port {
            locations: vec![Location::Tavern],
            ..yard()
        };
        assert_eq!(
            check_purchase(&goods, &no_yard, &brig, old, Purse(100_000)),
            Err(Refusal::NotTraded)
        );
    }

    #[test]
    fn a_new_ship_comes_sound_and_armed_with_her_company_aboard() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, ShipyardPlugin))
            .insert_resource(Goods::standard());
        let port = app.world_mut().spawn(yard()).id();
        let goods = Goods::standard();
        let sloop = ShipClass::sloop();
        let mut hold = sloop.empty_hold();
        hold.owned.add(goods.id("rum").unwrap(), 30.0);
        let ship = app
            .world_mut()
            .spawn((
                Name::new("Sea Horse"),
                sloop.build(20, 4, hold),
                Timbers(0.5),
                Purse(10_000),
                Docked { port },
            ))
            .id();
        let brig = class("Brig");
        app.world_mut().write_message(BuyShip { ship, class: brig });
        app.update();
        let world = app.world();
        assert_eq!(world.get::<ShipClass>(ship), Some(&brig));
        assert_eq!(world.get::<Hull>(ship), Some(&brig.hull));
        assert_eq!(world.get::<Complement>(ship), Some(&brig.complement));
        assert_eq!(world.get::<Battery>(ship).unwrap().guns, brig.guns);
        assert_eq!(world.get::<Timbers>(ship), Some(&Timbers(1.0)));
        assert_eq!(world.get::<Crew>(ship).unwrap().count, 20);
        let hold = world.get::<Hold>(ship).unwrap();
        assert_eq!(hold.capacity_tons, brig.hold_tons);
        assert_eq!(hold.owned.get(goods.id("rum").unwrap()), 30.0);
        assert_eq!(world.get::<Name>(ship).unwrap().as_str(), "Sea Horse");
        let paid = 10_000 - world.get::<Purse>(ship).unwrap().0;
        assert!(0 < paid && paid < brig.price, "paid {paid}");
    }
}
