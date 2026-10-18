//! Port markets: stock, prices, and trade.
//!
//! A market wants a target stock of each good it deals in. Prices rise as
//! stock falls below target and fall as it piles up. The port asks a little
//! over its price and bids a little under, so buying and selling straight
//! back always loses money.

use bevy::prelude::*;

use super::Refusal;
use crate::cargo::{Hold, Purse};
use crate::clock::DAY;
use crate::crew::{Complement, Crew};
use crate::goods::{Carriage, GoodId, Goods};
use crate::port::{Docked, Location, Port};

/// Fraction a market adds to its price when selling and takes off when buying.
pub const SPREAD: f64 = 0.1;
/// Price never falls below this fraction of the base price, however glutted the market.
const PRICE_FLOOR: f64 = 0.4;
/// Price never rises above this multiple of the base price, however short the market.
const PRICE_CEILING: f64 = 4.0;
/// Production stops once stock reaches this multiple of the target.
const MAX_STOCK: f64 = 3.0;

/// One good in a port's market.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketEntry {
    pub good: GoodId,
    pub stock: f64,
    /// Stock the market wants on hand. Always positive.
    pub target: f64,
    /// Change in stock per day: positive where the port produces the good,
    /// negative where it uses it up.
    pub per_day: f64,
}

impl MarketEntry {
    pub fn is_needed(&self) -> bool {
        self.per_day < 0.0
    }

    pub fn is_produced(&self) -> bool {
        self.per_day > 0.0
    }

    /// How much of the gap below target `quantity` more would fill, as a
    /// fraction of the target. Nothing for goods the port makes itself.
    pub fn gap_filled(&self, quantity: f64) -> f64 {
        if !self.is_needed() {
            return 0.0;
        }
        quantity.min(self.target - self.stock).max(0.0) / self.target
    }
}

/// A port's stock of everything it deals in.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Market {
    pub entries: Vec<MarketEntry>,
}

impl Market {
    pub fn entry(&self, good: GoodId) -> Option<&MarketEntry> {
        self.entries.iter().find(|entry| entry.good == good)
    }

    pub fn entry_mut(&mut self, good: GoodId) -> Option<&mut MarketEntry> {
        self.entries.iter_mut().find(|entry| entry.good == good)
    }
}

/// Price per unit at a given stock, before the spread.
pub fn unit_price(base_price: f64, stock: f64, target: f64) -> f64 {
    base_price * (target / stock).clamp(PRICE_FLOOR, PRICE_CEILING)
}

/// What the market asks for `quantity` units, priced one at a time as its
/// stock runs down. `None` if it hasn't got that many.
pub fn asking_price(entry: &MarketEntry, base_price: f64, quantity: u32) -> Option<u32> {
    if entry.stock < quantity as f64 {
        return None;
    }
    Some(
        (0..quantity)
            .map(|i| {
                let price = unit_price(base_price, entry.stock - i as f64, entry.target);
                (price * (1.0 + SPREAD)).ceil() as u32
            })
            .sum(),
    )
}

/// What the market bids for `quantity` units, priced one at a time as its
/// stock builds up.
pub fn bid_price(entry: &MarketEntry, base_price: f64, quantity: u32) -> u32 {
    (0..quantity)
        .map(|i| {
            let price = unit_price(base_price, entry.stock + i as f64, entry.target);
            (price * (1.0 - SPREAD)).floor() as u32
        })
        .sum()
}

/// How a good changes hands in port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dealing {
    /// Freight, bought and sold at the market.
    Freight,
    /// Sailors, hired at the tavern. Hired men join the crew.
    Hire,
}

impl Dealing {
    /// How `good` is dealt in, or `None` for goods that move only under
    /// contract, such as soldiers.
    pub fn of(goods: &Goods, good: GoodId) -> Option<Self> {
        if good == goods.sailors {
            Some(Self::Hire)
        } else if goods.carriage(good) == Carriage::Hold {
            Some(Self::Freight)
        } else {
            None
        }
    }

    /// Where in town this dealing happens.
    pub fn location(self) -> Location {
        match self {
            Self::Freight => Location::Market,
            Self::Hire => Location::Tavern,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// Order a docked ship to buy or sell at its port. Buying sailors hires them.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trade {
    pub ship: Entity,
    pub good: GoodId,
    pub quantity: u32,
    pub side: Side,
}

/// A ship brought a port goods it needed, by sale or contract. `share` is
/// how much of its shortage she filled, as a fraction of its target.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct Supplied {
    pub port: Entity,
    pub ship: Entity,
    pub share: f64,
}

/// The ship's side of a trade.
#[derive(Debug, Clone, Copy)]
pub struct Trader<'a> {
    pub hold: &'a Hold,
    pub purse: Purse,
    pub crew: &'a Crew,
    pub complement: &'a Complement,
}

impl Trader<'_> {
    /// Room aboard for goods that travel by `carriage`.
    pub fn room(&self, goods: &Goods, carriage: Carriage) -> f64 {
        self.hold
            .room(goods, carriage, self.crew.count, self.complement.berths)
    }
}

/// The price of a trade, or why it can't happen. Both the port screen and
/// the trade itself use this, so the screen only offers trades that will go
/// through.
pub fn quote(
    goods: &Goods,
    port: &Port,
    market: &Market,
    trader: &Trader,
    good: GoodId,
    quantity: u32,
    side: Side,
) -> Result<u32, Refusal> {
    let dealing = Dealing::of(goods, good).ok_or(Refusal::NotTraded)?;
    if !port.locations.contains(&dealing.location()) {
        return Err(Refusal::NotTraded);
    }
    let entry = market.entry(good).ok_or(Refusal::NotTraded)?;
    let base_price = goods.get(good).base_price;
    match (dealing, side) {
        (_, Side::Buy) => {
            let cost = asking_price(entry, base_price, quantity).ok_or(Refusal::OutOfStock)?;
            if cost > trader.purse.0 {
                Err(Refusal::CantAfford)
            } else if trader.room(goods, goods.carriage(good)) < quantity as f64 {
                Err(Refusal::NoRoom)
            } else {
                Ok(cost)
            }
        }
        (Dealing::Freight, Side::Sell) => {
            if trader.hold.owned.get(good) < quantity as f64 {
                Err(Refusal::NotEnough)
            } else {
                Ok(bid_price(entry, base_price, quantity))
            }
        }
        // Men already aboard aren't sold back to the tavern.
        (Dealing::Hire, Side::Sell) => Err(Refusal::NotTraded),
    }
}

/// Production and consumption: every market's stock drifts each step.
pub(super) fn drift_stock(time: Res<Time>, mut markets: Query<&mut Market>) {
    let days = time.delta_secs_f64() / DAY;
    for mut market in &mut markets {
        for entry in &mut market.entries {
            entry.stock = (entry.stock + entry.per_day * days).clamp(0.0, entry.target * MAX_STOCK);
        }
    }
}

pub(super) fn trade(
    mut trades: MessageReader<Trade>,
    goods: Res<Goods>,
    mut ships: Query<(&Docked, &mut Hold, &mut Purse, &mut Crew, &Complement)>,
    mut ports: Query<(&Port, &mut Market)>,
    mut supplies: MessageWriter<Supplied>,
) {
    for order in trades.read() {
        let Ok((docked, mut hold, mut purse, mut crew, complement)) = ships.get_mut(order.ship)
        else {
            continue;
        };
        let Ok((port, mut market)) = ports.get_mut(docked.port) else {
            continue;
        };
        let trader = Trader {
            hold: &hold,
            purse: *purse,
            crew: &crew,
            complement,
        };
        let quantity = order.quantity as f64;
        // The port screen only offers trades that quote, so a refusal here
        // means the world changed since the order was given.
        let Ok(price) = quote(
            &goods,
            port,
            &market,
            &trader,
            order.good,
            order.quantity,
            order.side,
        ) else {
            continue;
        };
        let Some(entry) = market.entry_mut(order.good) else {
            continue;
        };
        match order.side {
            Side::Buy => {
                entry.stock -= quantity;
                purse.0 -= price;
                if order.good == goods.sailors {
                    crew.count += order.quantity;
                } else {
                    hold.owned.add(order.good, quantity);
                }
            }
            Side::Sell => {
                supplies.write(Supplied {
                    port: docked.port,
                    ship: order.ship,
                    share: entry.gap_filled(quantity),
                });
                entry.stock += quantity;
                purse.0 += price;
                hold.owned.remove(order.good, quantity);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sim_app;

    fn entry(goods: &Goods, key: &str, stock: f64) -> MarketEntry {
        MarketEntry {
            good: goods.id(key).unwrap(),
            stock,
            target: 100.0,
            per_day: 10.0,
        }
    }

    #[test]
    fn prices_rise_as_stock_falls_and_stay_within_bounds() {
        assert_eq!(unit_price(10.0, 100.0, 100.0), 10.0);
        assert_eq!(unit_price(10.0, 50.0, 100.0), 20.0);
        assert_eq!(unit_price(10.0, 200.0, 100.0), 5.0);
        assert_eq!(unit_price(10.0, 0.0, 100.0), 10.0 * PRICE_CEILING);
        assert_eq!(unit_price(10.0, 1e6, 100.0), 10.0 * PRICE_FLOOR);
    }

    #[test]
    fn only_the_gap_below_target_of_a_needed_good_counts_as_supply() {
        let goods = Goods::standard();
        let needed = MarketEntry {
            per_day: -1.0,
            ..entry(&goods, "rum", 60.0)
        };
        assert_eq!(needed.gap_filled(10.0), 0.1);
        assert_eq!(needed.gap_filled(100.0), 0.4, "only up to target");
        assert_eq!(entry(&goods, "rum", 0.0).gap_filled(10.0), 0.0, "made here");
    }

    #[test]
    fn buying_and_selling_back_loses_money() {
        let goods = Goods::standard();
        let rum = entry(&goods, "rum", 100.0);
        let cost = asking_price(&rum, 90.0, 10).unwrap();
        let after = MarketEntry {
            stock: rum.stock - 10.0,
            ..rum.clone()
        };
        assert!(bid_price(&after, 90.0, 10) < cost);
    }

    #[test]
    fn each_unit_bought_costs_more_than_the_last() {
        let goods = Goods::standard();
        let rum = entry(&goods, "rum", 60.0);
        let one = asking_price(&rum, 90.0, 1).unwrap();
        let ten = asking_price(&rum, 90.0, 10).unwrap();
        assert!(ten > one * 10);
        assert_eq!(asking_price(&rum, 90.0, 61), None, "only 60 in stock");
    }

    struct Fixture {
        goods: Goods,
        port: Port,
        market: Market,
        hold: Hold,
        crew: Crew,
        complement: Complement,
    }

    fn fixture() -> Fixture {
        let goods = Goods::standard();
        let market = Market {
            entries: vec![
                entry(&goods, "rum", 100.0),
                entry(&goods, "sailors", 10.0),
                entry(&goods, "soldiers", 100.0),
            ],
        };
        Fixture {
            port: Port {
                flag: crate::factions::Nation::England,
                town: Vec3::ZERO,
                locations: vec![Location::Market, Location::Tavern],
            },
            goods,
            market,
            hold: Hold::new(40.0),
            crew: Crew::new(10),
            complement: Complement {
                min: 8,
                full: 40,
                berths: 12,
            },
        }
    }

    impl Fixture {
        fn quote(&self, purse: u32, key: &str, quantity: u32, side: Side) -> Result<u32, Refusal> {
            let trader = Trader {
                hold: &self.hold,
                purse: Purse(purse),
                crew: &self.crew,
                complement: &self.complement,
            };
            let good = self.goods.id(key).unwrap();
            quote(
                &self.goods,
                &self.port,
                &self.market,
                &trader,
                good,
                quantity,
                side,
            )
        }
    }

    #[test]
    fn quotes_refuse_what_the_ship_cannot_pay_for_or_carry() {
        let f = fixture();
        assert!(f.quote(10_000, "rum", 10, Side::Buy).is_ok());
        assert_eq!(f.quote(10, "rum", 10, Side::Buy), Err(Refusal::CantAfford));
        assert_eq!(f.quote(100_000, "rum", 41, Side::Buy), Err(Refusal::NoRoom));
        assert_eq!(
            f.quote(10_000, "rum", 1, Side::Sell),
            Err(Refusal::NotEnough)
        );
        assert_eq!(
            f.quote(10_000, "sugar", 1, Side::Buy),
            Err(Refusal::NotTraded)
        );
    }

    #[test]
    fn sailors_are_hired_into_berths_and_soldiers_are_not_for_sale() {
        let f = fixture();
        // Two berths free beside the crew of ten.
        assert!(f.quote(10_000, "sailors", 2, Side::Buy).is_ok());
        assert_eq!(
            f.quote(10_000, "sailors", 3, Side::Buy),
            Err(Refusal::NoRoom)
        );
        assert_eq!(
            f.quote(10_000, "sailors", 1, Side::Sell),
            Err(Refusal::NotTraded)
        );
        assert_eq!(
            f.quote(10_000, "soldiers", 1, Side::Buy),
            Err(Refusal::NotTraded)
        );
    }

    #[test]
    fn a_port_without_a_tavern_hires_no_one() {
        let mut f = fixture();
        f.port.locations = vec![Location::Market];
        assert_eq!(
            f.quote(10_000, "sailors", 1, Side::Buy),
            Err(Refusal::NotTraded)
        );
        assert!(f.quote(10_000, "rum", 1, Side::Buy).is_ok());
    }

    #[test]
    fn producers_fill_up_and_consumers_run_down_over_days() {
        let mut app = sim_app();
        let goods = app.world().resource::<Goods>().clone();
        let mut needed = entry(&goods, "rum", 10.0);
        needed.per_day = -1_000_000.0;
        let market = app
            .world_mut()
            .spawn(Market {
                entries: vec![entry(&goods, "sugar", 100.0), needed],
            })
            .id();
        crate::test_support::run_seconds(&mut app, 60.0);

        let market = app.world().get::<Market>(market).unwrap();
        assert!(market.entries[0].stock > 100.0, "production adds stock");
        assert_eq!(market.entries[1].stock, 0.0, "consumption stops at empty");
    }

    #[test]
    fn a_trade_moves_goods_and_money_between_ship_and_port() {
        let mut app = sim_app();
        let f = fixture();
        let rum = f.goods.id("rum").unwrap();
        let port = app
            .world_mut()
            .spawn((f.port.clone(), f.market.clone()))
            .id();
        let ship = app
            .world_mut()
            .spawn((
                Docked { port },
                f.hold.clone(),
                Purse(5_000),
                f.crew,
                f.complement,
            ))
            .id();
        let cost = f.quote(5_000, "rum", 10, Side::Buy).unwrap();
        app.world_mut().write_message(Trade {
            ship,
            good: rum,
            quantity: 10,
            side: Side::Buy,
        });
        app.world_mut().write_message(Trade {
            ship,
            good: f.goods.sailors,
            quantity: 2,
            side: Side::Buy,
        });
        app.update();

        let world = app.world();
        assert_eq!(world.get::<Hold>(ship).unwrap().owned.get(rum), 10.0);
        assert_eq!(world.get::<Crew>(ship).unwrap().count, 12);
        let purse = world.get::<Purse>(ship).unwrap().0;
        assert!(purse < 5_000 - cost, "paid for rum and sailors: {purse}");
        let market = world.get::<Market>(port).unwrap();
        assert!(market.entry(rum).unwrap().stock < 90.01);
    }
}
