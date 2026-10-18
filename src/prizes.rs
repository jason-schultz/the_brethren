//! Boarding and prizes: taking ships, and what becomes of them.
//!
//! A ship alongside her enemy can board. A struck ship is carried without a
//! fight; otherwise crews fight it out. The victor plunders her purse and
//! as much cargo as she can stow, then sends a prize crew of spare hands
//! aboard. The prize follows her captor from then on, and docks where she
//! docks, unless ordered to wait or to sail to a port. With no hands to
//! spare, the plundered ship is let go. NPC captains board whoever strikes
//! to them, and want plunder, not ships: they burn their prizes.

use bevy::prelude::*;

use crate::alert::{Alert, AlertKind};
use crate::cargo::{Hold, Purse};
use crate::combat::tactics::BOARDING_RANGE;
use crate::combat::{Engaged, Struck, Sunk};
use crate::crew::{Complement, Crew, PRIZE_HANDS};
use crate::economy::jobs::{Carrier, Job};
use crate::factions::{Colours, Deed, DeedKind};
use crate::goods::{Carriage, Goods};
use crate::merchants::{Merchant, Plan};
use crate::navigation::{Arrival, Route, SailOrder};
use crate::officers::Afterguard;
use crate::patrols::Patrol;
use crate::player::Player;
use crate::port::{CastOff, Docked, Port};
use crate::sailing::{Helm, Sails};
use crate::schedule::{FrameSet, SimSet};

/// Defenders fight from behind their bulwarks: each counts for this many
/// boarders.
const DEFENDERS_EDGE: f64 = 1.25;
/// Fraction of the defence the winning boarders lose.
const VICTORS_LOSS: f64 = 0.3;
/// Fractions of the boarding party lost, and of it inflicted, when repulsed.
const REPULSED_LOSS: f64 = 0.4;
const REPULSE_TOLL: f64 = 0.2;
/// A prize keeps within this distance of her captor, meters.
const KEEP_UP: f32 = 250.0;
/// A prize lies this far astern of her captor when she catches up, meters.
const ASTERN: f32 = 120.0;

/// A captured ship sailed by a prize crew, under her captor's orders.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[require(PrizeOrders)]
pub struct Prize {
    pub captor: Entity,
}

/// What a prize has been told to do.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PrizeOrders {
    /// Keep company with her captor, and dock where she docks.
    #[default]
    Follow,
    /// Stay where she is: heave to at sea, or stay in port.
    Wait,
    /// Sail to this port and lie there.
    SailTo(Entity),
}

/// Give a prize her orders.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderPrize {
    pub prize: Entity,
    pub orders: PrizeOrders,
}

/// Order a ship to board the enemy she's engaged with.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardOrder {
    pub ship: Entity,
}

/// How a boarding went.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Melee {
    pub carried: bool,
    pub boarders_lost: f64,
    pub defenders_lost: f64,
}

/// One side of a fight for a ship's deck.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Party {
    pub men: u32,
    /// What each man is worth: his heart, and who leads him.
    pub spirit: f64,
}

impl Party {
    /// A ship's company, as her morale and officers make it.
    pub fn of(crew: &Crew, afterguard: Option<&Afterguard>) -> Self {
        Self {
            men: crew.count,
            spirit: crew.spirits().work_rate() as f64
                * afterguard.map_or(1.0, Afterguard::boarding),
        }
    }

    fn strength(self) -> f64 {
        self.men as f64 * self.spirit
    }
}

/// The fight for a ship's deck between `boarders` and `defenders`.
pub fn melee(boarders: Party, defenders: Party, struck: bool) -> Melee {
    let attack = boarders.strength();
    let defence = defenders.strength() * DEFENDERS_EDGE;
    if struck || attack > defence {
        Melee {
            carried: true,
            boarders_lost: if struck {
                0.0
            } else {
                defenders.men as f64 * DEFENDERS_EDGE * VICTORS_LOSS
            },
            defenders_lost: if struck { 0.0 } else { defenders.men as f64 },
        }
    } else {
        Melee {
            carried: false,
            boarders_lost: boarders.men as f64 * REPULSED_LOSS,
            defenders_lost: boarders.men as f64 * REPULSE_TOLL,
        }
    }
}

/// Re-points every prize of `from` to `to`, as when the captain shifts her
/// flag.
pub fn reassign_prizes<'a>(prizes: impl Iterator<Item = Mut<'a, Prize>>, from: Entity, to: Entity) {
    for mut prize in prizes {
        if prize.captor == from {
            prize.captor = to;
        }
    }
}

/// Men the captor can spare for a prize crew needing `needed` hands,
/// keeping the `keep` she needs herself.
pub fn spare_hands(captor_crew: u32, keep: u32, needed: u32) -> u32 {
    captor_crew.saturating_sub(keep).min(needed)
}

/// Moves up to `room` tons of freight from a prize's hold to her captor's.
/// Her passengers are put ashore, and whatever stays aboard, contract
/// freight included, is the captor's own now.
pub fn plunder(goods: &Goods, prize: &mut Hold, captor: &mut Hold, room: f64) {
    let mut stowed = 0.0;
    let consigned = std::mem::take(&mut prize.consigned);
    for (good, quantity) in consigned.iter() {
        prize.owned.add(good, quantity);
    }
    for (good, quantity) in prize.owned.iter().collect::<Vec<_>>() {
        if goods.carriage(good) != Carriage::Hold {
            prize.owned.remove(good, quantity);
            continue;
        }
        let taken = prize
            .owned
            .remove(good, quantity.min((room - stowed).max(0.0)));
        captor.owned.add(good, taken);
        stowed += taken;
    }
}

pub struct PrizesPlugin;

impl Plugin for PrizesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BoardOrder>()
            .add_message::<OrderPrize>()
            // Boarding and orders work while paused.
            .add_systems(Update, (board, order_prizes).in_set(FrameSet::Orders))
            .add_systems(
                FixedUpdate,
                (board_the_struck, sail_prizes).in_set(SimSet::Economy),
            );
    }
}

fn board(
    mut orders: MessageReader<BoardOrder>,
    goods: Res<Goods>,
    engaged: Query<&Engaged>,
    mut ships: Query<
        (
            &Transform,
            &mut Crew,
            &Complement,
            &mut Hold,
            &mut Purse,
            Has<Struck>,
            Option<&Colours>,
            Option<&Afterguard>,
        ),
        Without<Sunk>,
    >,
    jobs: Query<(Entity, &Carrier), With<Job>>,
    players: Query<(), With<Player>>,
    mut alerts: MessageWriter<Alert>,
    mut deeds: MessageWriter<Deed>,
    mut commands: Commands,
) {
    // Several orders to board one ship can come in a frame, before the
    // first one's commands land.
    let mut taken = Vec::new();
    for order in orders.read() {
        // Her quarry needn't be engaged: a struck ship has given up the fight.
        let Ok(&Engaged { enemy, .. }) = engaged.get(order.ship) else {
            continue;
        };
        if taken.contains(&enemy) {
            continue;
        }
        let Ok([captor, prize]) = ships.get_many_mut([order.ship, enemy]) else {
            continue;
        };
        let (
            captor_at,
            mut captor_crew,
            captor_complement,
            mut captor_hold,
            mut captor_purse,
            _,
            _,
            captor_officers,
        ) = captor;
        let (
            prize_at,
            mut prize_crew,
            _,
            mut prize_hold,
            mut prize_purse,
            struck,
            colours,
            prize_officers,
        ) = prize;
        if captor_at.translation.distance(prize_at.translation) > BOARDING_RANGE {
            continue;
        }
        let fight = melee(
            Party::of(&captor_crew, captor_officers),
            Party::of(&prize_crew, prize_officers),
            struck,
        );
        let losses = |officers: Option<&Afterguard>| officers.map_or(1.0, Afterguard::losses);
        captor_crew.take_casualties(fight.boarders_lost * losses(captor_officers));
        prize_crew.take_casualties(fight.defenders_lost * losses(prize_officers));
        if !fight.carried {
            alerts.write(Alert {
                ship: order.ship,
                kind: AlertKind::Repulsed { other: enemy },
            });
            continue;
        }

        taken.push(enemy);
        if let Some(&Colours(nation)) = colours {
            deeds.write(Deed {
                doer: order.ship,
                kind: DeedKind::Captured,
                nation,
            });
        }
        // Plunder: her purse, and what cargo fits.
        captor_purse.0 += std::mem::take(&mut prize_purse.0);
        let room = captor_hold.room(
            &goods,
            Carriage::Hold,
            captor_crew.count,
            captor_complement.berths,
        );
        plunder(&goods, &mut prize_hold, &mut captor_hold, room);
        for (job, carrier) in &jobs {
            if carrier.0 == enemy {
                commands.entity(job).despawn();
            }
        }

        commands.entity(order.ship).remove::<Engaged>();
        if !players.contains(order.ship) {
            commands.entity(enemy).despawn();
            continue;
        }
        let hands = spare_hands(captor_crew.count, captor_complement.min, PRIZE_HANDS);
        if hands == 0 {
            // No one to sail her: let her go, plundered.
            commands.entity(enemy).remove::<Struck>();
            alerts.write(Alert {
                ship: order.ship,
                kind: AlertKind::Plundered { other: enemy },
            });
            continue;
        }
        captor_crew.count -= hands;
        // Her own men are set adrift in the boats.
        *prize_crew = Crew::new(hands);
        commands
            .entity(enemy)
            // She sails under her captor now, not her old colours or orders.
            .remove::<(Merchant, Patrol, Struck, Engaged, Route, Colours)>()
            .insert(Prize { captor: order.ship });
        alerts.write(Alert {
            ship: order.ship,
            kind: AlertKind::PrizeTaken { other: enemy },
        });
    }
}

/// NPC captains board an enemy who has struck to them, once alongside.
fn board_the_struck(
    captains: Query<
        (Entity, &Engaged, &Transform),
        (Without<Player>, Without<Prize>, Without<Struck>),
    >,
    struck: Query<&Transform, With<Struck>>,
    mut orders: MessageWriter<BoardOrder>,
) {
    for (ship, engaged, at) in &captains {
        if struck
            .get(engaged.enemy)
            .is_ok_and(|enemy| enemy.translation.distance(at.translation) <= BOARDING_RANGE)
        {
            orders.write(BoardOrder { ship });
        }
    }
}

fn order_prizes(mut orders: MessageReader<OrderPrize>, mut prizes: Query<&mut PrizeOrders>) {
    for order in orders.read() {
        if let Ok(mut orders) = prizes.get_mut(order.prize) {
            orders.set_if_neq(order.orders);
        }
    }
}

/// Sails each prize by her orders. A prize whose captor is gone is her own
/// ship again.
fn sail_prizes(
    mut prizes: Query<(
        Entity,
        &Prize,
        &PrizeOrders,
        &Transform,
        Option<&Docked>,
        Option<&Route>,
        &mut Helm,
    )>,
    captors: Query<(&Transform, Option<&Docked>)>,
    ports: Query<&Transform, With<Port>>,
    mut cast_offs: MessageWriter<CastOff>,
    mut orders: MessageWriter<SailOrder>,
    mut commands: Commands,
) {
    for (prize, &Prize { captor }, &prize_orders, transform, docked, route, mut helm) in &mut prizes
    {
        let Ok((captor_at, captor_docked)) = captors.get(captor) else {
            commands
                .entity(prize)
                .remove::<(Prize, PrizeOrders)>()
                .insert(Merchant { plan: Plan::AtSea });
            continue;
        };
        let bound_for = route.and_then(|route| match route.on_arrival {
            Arrival::Dock(port) => Some(port),
            Arrival::HeaveTo => None,
        });
        match prize_orders {
            PrizeOrders::Follow => {}
            PrizeOrders::Wait => {
                if route.is_some() {
                    commands.entity(prize).remove::<Route>();
                }
                helm.sails = Sails::Furled;
                continue;
            }
            PrizeOrders::SailTo(port) => {
                let arrived = docked.is_some_and(|docked| docked.port == port);
                if !arrived
                    && bound_for != Some(port)
                    && let Ok(anchorage) = ports.get(port)
                {
                    if docked.is_some() {
                        cast_offs.write(CastOff { ship: prize });
                    }
                    orders.write(SailOrder {
                        ship: prize,
                        destination: anchorage.translation,
                        on_arrival: Arrival::Dock(port),
                    });
                }
                continue;
            }
        }
        match (captor_docked, docked) {
            (Some(captor_port), Some(here)) if captor_port.port == here.port => {}
            (Some(captor_port), _) => {
                if bound_for != Some(captor_port.port) {
                    if docked.is_some() {
                        cast_offs.write(CastOff { ship: prize });
                    }
                    orders.write(SailOrder {
                        ship: prize,
                        destination: captor_at.translation,
                        on_arrival: Arrival::Dock(captor_port.port),
                    });
                }
            }
            (None, _) => {
                let behind = captor_at.translation
                    + (transform.translation - captor_at.translation).normalize_or_zero() * ASTERN;
                let drifting = route
                    .and_then(|route| route.waypoints.back())
                    .is_none_or(|end| end.distance(behind) > KEEP_UP / 2.0);
                if transform.translation.distance(captor_at.translation) > KEEP_UP && drifting {
                    if docked.is_some() {
                        cast_offs.write(CastOff { ship: prize });
                    }
                    orders.write(SailOrder {
                        ship: prize,
                        destination: behind,
                        on_arrival: Arrival::HeaveTo,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn party(men: u32) -> Party {
        Party { men, spirit: 1.0 }
    }

    #[test]
    fn a_struck_ship_is_carried_without_a_fight() {
        let fight = melee(party(5), party(30), true);
        assert!(fight.carried);
        assert_eq!((fight.boarders_lost, fight.defenders_lost), (0.0, 0.0));
    }

    #[test]
    fn boarders_need_more_than_the_defenders_edge() {
        assert!(melee(party(40), party(10), false).carried);
        assert!(
            !melee(party(12), party(10), false).carried,
            "12 against 10 behind bulwarks"
        );
        let repulsed = melee(party(12), party(10), false);
        assert!(repulsed.boarders_lost > repulsed.defenders_lost);
        let won = melee(party(40), party(10), false);
        assert_eq!(won.defenders_lost, 10.0, "the whole defence");
        assert!(won.boarders_lost > 0.0 && won.boarders_lost < 10.0);
    }

    #[test]
    fn heart_and_a_first_mate_carry_a_deck_numbers_alone_would_not() {
        let led = Party {
            men: 12,
            spirit: 1.1 * 1.25,
        };
        assert!(melee(led, party(10), false).carried);
        let sullen = Party {
            men: 14,
            spirit: 0.8,
        };
        assert!(!melee(sullen, party(10), false).carried);
    }

    #[test]
    fn plunder_fills_the_captors_hold_and_leaves_the_rest_aboard() {
        let goods = Goods::standard();
        let rum = goods.id("rum").unwrap();
        let soldiers = goods.id("soldiers").unwrap();
        let mut prize = Hold::new(40.0);
        prize.owned.add(rum, 10.0);
        prize.consigned.add(goods.provisions, 20.0);
        prize.consigned.add(soldiers, 15.0);
        let mut captor = Hold::new(40.0);
        plunder(&goods, &mut prize, &mut captor, 25.0);

        assert_eq!(captor.tons_used(&goods), 25.0, "filled to the room she had");
        assert_eq!(prize.tons_used(&goods), 5.0, "the rest stays aboard");
        assert_eq!(prize.consigned.iter().count(), 0, "contracts are void");
        assert_eq!(
            prize.owned.get(soldiers) + captor.owned.get(soldiers),
            0.0,
            "put ashore"
        );
    }

    #[test]
    fn a_prize_crew_comes_from_spare_hands() {
        assert_eq!(spare_hands(40, 8, 8), 8);
        assert_eq!(spare_hands(12, 8, 8), 4, "only four to spare");
        assert_eq!(spare_hands(8, 8, 8), 0);
        assert_eq!(spare_hands(3, 8, 8), 0);
    }

    mod at_sea {
        use super::super::*;
        use crate::alert::InSight;
        use crate::combat::{EngageOrder, Timbers};
        use crate::economy::market::Market;
        use crate::math::heading_to_direction;
        use crate::player::Player;
        use crate::port::{Location, Port};
        use crate::sailing::{Heading, Helm, Sails};
        use crate::ship_class::ShipClass;
        use crate::shipyard::{SellPrize, TakeCommand};
        use crate::test_support::{run_until, sim_app};
        use crate::time_scale::MAX_STEP;

        const PURSE: u32 = 250;

        /// A raider (the player) and a laden merchant to her west, and a
        /// port with a shipyard well to the south.
        fn chase() -> (App, Entity, Entity, Entity) {
            let mut app = sim_app();
            app.world_mut()
                .resource_mut::<Time<Fixed>>()
                .set_timestep_seconds(MAX_STEP);
            let goods = app.world().resource::<Goods>().clone();
            let rum = goods.id("rum").unwrap();
            let class = ShipClass::SLOOP;
            let port = app
                .world_mut()
                .spawn((
                    Port {
                        flag: crate::factions::Nation::England,
                        town: Vec3::new(0.0, 0.0, 4000.0),
                        locations: vec![Location::Shipyard],
                    },
                    Market::default(),
                    Transform::from_xyz(0.0, 0.0, 3000.0),
                ))
                .id();
            let raider = app
                .world_mut()
                .spawn((
                    Name::new("Raider"),
                    Player,
                    InSight::default(),
                    class.build(40, 8, class.empty_hold()),
                    Purse(0),
                    Helm {
                        course: 0.0,
                        sails: Sails::Set,
                    },
                ))
                .id();
            let mut hold = class.empty_hold();
            hold.owned.add(rum, 30.0);
            let merchant = app
                .world_mut()
                .spawn((
                    Name::new("Swallow"),
                    Merchant { plan: Plan::AtSea },
                    class.build(10, 4, hold),
                    Purse(PURSE),
                    Heading(0.0),
                    Helm {
                        course: 0.0,
                        sails: Sails::Set,
                    },
                    Transform::from_translation(heading_to_direction(270_f32.to_radians()) * 900.0),
                ))
                .id();
            (app, raider, merchant, port)
        }

        /// Runs the chase until the merchant strikes and the raider lies
        /// alongside, then boards her.
        fn take_her(app: &mut App, raider: Entity, merchant: Entity) {
            app.world_mut().write_message(EngageOrder {
                ship: raider,
                target: merchant,
            });
            let alongside = run_until(app, 4.0 * 3600.0, |world| {
                let at = |ship| world.get::<Transform>(ship).unwrap().translation;
                world.get::<Struck>(merchant).is_some()
                    && at(raider).distance(at(merchant)) <= BOARDING_RANGE
            });
            assert!(alongside, "never got alongside a struck merchant");
            app.world_mut().write_message(BoardOrder { ship: raider });
            app.update();
        }

        #[test]
        fn a_struck_merchant_is_boarded_plundered_and_sailed_as_a_prize() {
            let (mut app, raider, merchant, _) = chase();
            let crew_before = app.world().get::<Crew>(raider).unwrap().count;
            take_her(&mut app, raider, merchant);

            let world = app.world();
            assert_eq!(
                world.get::<Prize>(merchant),
                Some(&Prize { captor: raider })
            );
            assert!(world.get::<Merchant>(merchant).is_none());
            assert_eq!(world.get::<Purse>(raider), Some(&Purse(PURSE)));
            let goods = world.resource::<Goods>();
            assert_eq!(
                world
                    .get::<Hold>(raider)
                    .unwrap()
                    .owned
                    .get(goods.id("rum").unwrap()),
                30.0
            );
            let prize_crew = world.get::<Crew>(merchant).unwrap().count;
            assert_eq!(prize_crew, PRIZE_HANDS);
            assert!(world.get::<Crew>(raider).unwrap().count <= crew_before - prize_crew);
        }

        #[test]
        fn a_prize_follows_her_captor_into_port_and_is_sold() {
            let (mut app, raider, merchant, port) = chase();
            take_her(&mut app, raider, merchant);
            let anchorage = app.world().get::<Transform>(port).unwrap().translation;
            app.world_mut().write_message(SailOrder {
                ship: raider,
                destination: anchorage,
                on_arrival: Arrival::Dock(port),
            });
            let both_in = run_until(&mut app, 4.0 * 3600.0, |world| {
                world.get::<Docked>(raider).is_some() && world.get::<Docked>(merchant).is_some()
            });
            assert!(both_in, "the prize never followed her captor in");

            let purse = app.world().get::<Purse>(raider).unwrap().0;
            let crew = app.world().get::<Crew>(raider).unwrap().count;
            app.world_mut().write_message(SellPrize { prize: merchant });
            app.update();
            assert!(app.world().get_entity(merchant).is_err(), "not sold");
            assert!(app.world().get::<Purse>(raider).unwrap().0 > purse);
            assert!(
                app.world().get::<Crew>(raider).unwrap().count > crew,
                "prize crew rejoins"
            );
        }

        #[test]
        fn a_small_prize_crew_keeps_up_with_her_captor_at_sea() {
            let (mut app, raider, merchant, _) = chase();
            // The player's starting crew: only a couple of hands to spare.
            app.world_mut().get_mut::<Crew>(raider).unwrap().count = 12;
            take_her(&mut app, raider, merchant);
            assert!(app.world().get::<Crew>(merchant).unwrap().count <= 4);
            let at = app.world().get::<Transform>(raider).unwrap().translation;
            app.world_mut().write_message(SailOrder {
                ship: raider,
                destination: at + Vec3::new(3000.0, 0.0, 0.0),
                on_arrival: Arrival::HeaveTo,
            });
            let gap = |world: &World| {
                let at = |ship| world.get::<Transform>(ship).unwrap().translation;
                at(raider).distance(at(merchant))
            };
            let mut widest: f32 = 0.0;
            run_until(&mut app, 1800.0, |world| {
                widest = widest.max(gap(world));
                false
            });
            assert!(widest < 1000.0, "fell {widest:.0} m behind");
            assert!(gap(app.world()) <= KEEP_UP, "never caught up once hove to");
        }

        #[test]
        fn a_prize_ordered_to_wait_stays_put_and_one_sent_to_port_docks_there() {
            let (mut app, raider, merchant, port) = chase();
            take_her(&mut app, raider, merchant);
            app.world_mut().write_message(OrderPrize {
                prize: merchant,
                orders: PrizeOrders::Wait,
            });
            let start = app.world().get::<Transform>(merchant).unwrap().translation;
            let at = app.world().get::<Transform>(raider).unwrap().translation;
            app.world_mut().write_message(SailOrder {
                ship: raider,
                destination: at + Vec3::new(2000.0, 0.0, 0.0),
                on_arrival: Arrival::HeaveTo,
            });
            run_until(&mut app, 1200.0, |_| false);
            let moved = app
                .world()
                .get::<Transform>(merchant)
                .unwrap()
                .translation
                .distance(start);
            assert!(moved < 50.0, "drifted {moved:.0} m while waiting");

            app.world_mut().write_message(OrderPrize {
                prize: merchant,
                orders: PrizeOrders::SailTo(port),
            });
            let docked = run_until(&mut app, 4.0 * 3600.0, |world| {
                world.get::<Docked>(merchant) == Some(&Docked { port })
            });
            assert!(docked, "never made port");
            assert!(
                app.world().get::<Docked>(raider).is_none(),
                "she went alone"
            );
        }

        #[test]
        fn cargo_and_men_move_between_ships_together() {
            use crate::fleet::{Load, Transfer};
            let (mut app, raider, merchant, _) = chase();
            take_her(&mut app, raider, merchant);
            let rum = app.world().resource::<Goods>().id("rum").unwrap();
            // Her cargo went to the raider when she was plundered; put some back.
            app.world_mut().write_message(Transfer {
                from: raider,
                to: merchant,
                load: Load::Cargo {
                    good: rum,
                    quantity: 10,
                },
            });
            app.world_mut().write_message(Transfer {
                from: raider,
                to: merchant,
                load: Load::Crew(3),
            });
            app.update();
            let world = app.world();
            assert_eq!(world.get::<Hold>(merchant).unwrap().owned.get(rum), 10.0);
            assert_eq!(world.get::<Crew>(merchant).unwrap().count, PRIZE_HANDS + 3);

            // Apart, nothing moves.
            app.world_mut()
                .get_mut::<Transform>(merchant)
                .unwrap()
                .translation += Vec3::new(5000.0, 0.0, 0.0);
            app.world_mut().write_message(Transfer {
                from: merchant,
                to: raider,
                load: Load::Crew(1),
            });
            app.update();
            assert_eq!(
                app.world().get::<Crew>(merchant).unwrap().count,
                PRIZE_HANDS + 3
            );
        }

        #[test]
        fn the_captain_can_shift_her_flag_into_a_prize() {
            let (mut app, raider, merchant, port) = chase();
            take_her(&mut app, raider, merchant);
            for ship in [raider, merchant] {
                app.world_mut().entity_mut(ship).insert(Docked { port });
            }
            app.world_mut()
                .write_message(TakeCommand { prize: merchant });
            app.update();
            let world = app.world();
            assert!(world.get::<Player>(merchant).is_some());
            assert!(world.get::<Player>(raider).is_none());
            assert_eq!(
                world.get::<Prize>(raider),
                Some(&Prize { captor: merchant })
            );
            assert_eq!(
                world.get::<Purse>(merchant),
                Some(&Purse(PURSE)),
                "her purse came with her"
            );
            assert!(world.get::<Timbers>(merchant).is_some());
        }
    }
}
