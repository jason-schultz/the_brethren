//! What a ship carries, and the captain's money.

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::goods::{Carriage, GoodId, Goods};
use crate::sailing::Laden;
use crate::schedule::SimSet;

/// Quantities of goods: tons of freight or heads of people.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lading(BTreeMap<GoodId, f64>);

impl Lading {
    pub fn get(&self, good: GoodId) -> f64 {
        self.0.get(&good).copied().unwrap_or(0.0)
    }

    pub fn add(&mut self, good: GoodId, quantity: f64) {
        *self.0.entry(good).or_insert(0.0) += quantity;
    }

    /// Removes up to `quantity` and returns how much was actually removed.
    pub fn remove(&mut self, good: GoodId, quantity: f64) -> f64 {
        let held = self.get(good);
        let removed = held.min(quantity);
        if held - removed <= f64::EPSILON {
            self.0.remove(&good);
        } else {
            self.0.insert(good, held - removed);
        }
        removed
    }

    pub fn iter(&self) -> impl Iterator<Item = (GoodId, f64)> + '_ {
        self.0.iter().map(|(&good, &quantity)| (good, quantity))
    }

    /// Total quantity of goods that travel by `carriage`.
    fn total(&self, goods: &Goods, carriage: Carriage) -> f64 {
        self.iter()
            .filter(|&(good, _)| goods.carriage(good) == carriage)
            .map(|(_, quantity)| quantity)
            .sum()
    }
}

/// Everything aboard a ship besides its crew.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Hold {
    /// Cargo space in tons burthen.
    pub capacity_tons: f64,
    /// The captain's own goods.
    pub owned: Lading,
    /// Goods and passengers carried under contract for others.
    pub consigned: Lading,
}

impl Hold {
    pub fn new(capacity_tons: f64) -> Self {
        Self {
            capacity_tons,
            ..default()
        }
    }

    /// Tons of freight aboard, owned and consigned.
    pub fn tons_used(&self, goods: &Goods) -> f64 {
        self.owned.total(goods, Carriage::Hold) + self.consigned.total(goods, Carriage::Hold)
    }

    /// Passengers carried under contract.
    pub fn passengers(&self, goods: &Goods) -> f64 {
        self.consigned.total(goods, Carriage::Berths)
    }

    /// Room left for goods that travel by `carriage`: tons in the hold, or
    /// berths not taken by the crew of `crew` men or by passengers.
    pub fn room(&self, goods: &Goods, carriage: Carriage, crew: u32, berths: u32) -> f64 {
        match carriage {
            Carriage::Hold => self.capacity_tons - self.tons_used(goods),
            Carriage::Berths => berths as f64 - crew as f64 - self.passengers(goods),
        }
    }
}

pub struct CargoPlugin;

impl Plugin for CargoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, trim.in_set(SimSet::Navigation));
    }
}

/// Keeps each ship's [`Laden`] in step with her hold.
fn trim(goods: Res<Goods>, mut ships: Query<(&Hold, &mut Laden)>) {
    for (hold, mut laden) in &mut ships {
        let full = (hold.tons_used(&goods) / hold.capacity_tons.max(1.0)) as f32;
        laden.set_if_neq(Laden(full));
    }
}

/// The captain's money, in pieces of eight.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Purse(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lading_adds_removes_and_never_goes_negative() {
        let goods = Goods::standard();
        let mut lading = Lading::default();
        lading.add(goods.provisions, 5.0);
        lading.add(goods.provisions, 2.5);
        assert_eq!(lading.get(goods.provisions), 7.5);
        assert_eq!(lading.remove(goods.provisions, 10.0), 7.5);
        assert_eq!(lading.get(goods.provisions), 0.0);
        assert_eq!(lading.iter().count(), 0, "empty entries are dropped");
    }

    #[test]
    fn freight_fills_the_hold_and_passengers_fill_berths() {
        let goods = Goods::standard();
        let soldiers = goods.id("soldiers").unwrap();
        let rum = goods.id("rum").unwrap();
        let mut hold = Hold::new(40.0);
        hold.owned.add(goods.provisions, 5.0);
        hold.consigned.add(rum, 20.0);
        hold.consigned.add(soldiers, 10.0);

        assert_eq!(hold.tons_used(&goods), 25.0);
        assert_eq!(hold.room(&goods, Carriage::Hold, 12, 75), 15.0);
        assert_eq!(hold.passengers(&goods), 10.0);
        assert_eq!(hold.room(&goods, Carriage::Berths, 12, 75), 53.0);
    }
}
