//! Goods: everything ships carry, defined as data (`assets/data/goods.ron`).

use bevy::prelude::*;
use serde::Deserialize;

const GOODS: &str = include_str!("../assets/data/goods.ron");

/// Key of the provisions every soul aboard eats.
const PROVISIONS: &str = "provisions";
/// Key of the sailors that taverns hire out.
const SAILORS: &str = "sailors";
/// Key of the soldiers that garrison a port.
const SOLDIERS: &str = "soldiers";

/// Index of a good in [`Goods`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoodId(usize);

/// How a good travels.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carriage {
    /// Freight, measured in tons and stowed in the hold.
    Hold,
    /// People, counted by head. They take berths and eat provisions.
    Berths,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Good {
    pub key: String,
    pub name: String,
    /// Price per ton or per head, in pieces of eight, at a market holding
    /// exactly the stock it wants.
    pub base_price: f64,
    pub carriage: Carriage,
}

/// Every good in the game.
#[derive(Resource, Debug, Clone)]
pub struct Goods {
    goods: Vec<Good>,
    /// What ships' companies eat.
    pub provisions: GoodId,
    /// Men looking for a berth.
    pub sailors: GoodId,
    /// A port's garrison.
    pub soldiers: GoodId,
}

#[derive(Debug, PartialEq)]
pub enum GoodsError {
    Parse(ron::error::SpannedError),
    Missing(&'static str),
}

impl Goods {
    pub fn parse(source: &str) -> Result<Self, GoodsError> {
        let goods: Vec<Good> = ron::from_str(source).map_err(GoodsError::Parse)?;
        let find = |key: &'static str| {
            goods
                .iter()
                .position(|good| good.key == key)
                .map(GoodId)
                .ok_or(GoodsError::Missing(key))
        };
        Ok(Self {
            provisions: find(PROVISIONS)?,
            sailors: find(SAILORS)?,
            soldiers: find(SOLDIERS)?,
            goods,
        })
    }

    /// The goods shipped with the game.
    pub fn standard() -> Self {
        Self::parse(GOODS).expect("the embedded goods are validated by tests")
    }

    pub fn get(&self, id: GoodId) -> &Good {
        &self.goods[id.0]
    }

    pub fn id(&self, key: &str) -> Option<GoodId> {
        self.goods
            .iter()
            .position(|good| good.key == key)
            .map(GoodId)
    }

    pub fn carriage(&self, id: GoodId) -> Carriage {
        self.get(id).carriage
    }
}

pub struct GoodsPlugin;

impl Plugin for GoodsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Goods::standard());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_goods_parse_with_unique_keys() {
        let goods = Goods::standard();
        for (i, good) in goods.goods.iter().enumerate() {
            assert_eq!(
                goods.id(&good.key),
                Some(GoodId(i)),
                "duplicate {}",
                good.key
            );
            assert!(good.base_price >= 0.0, "{} has a negative price", good.key);
        }
        assert_eq!(goods.carriage(goods.provisions), Carriage::Hold);
        assert_eq!(goods.carriage(goods.sailors), Carriage::Berths);
    }

    #[test]
    fn goods_without_provisions_are_rejected() {
        let source = r#"[(key: "sailors", name: "Sailors", base_price: 1.0, carriage: Berths)]"#;
        assert_eq!(
            Goods::parse(source).unwrap_err(),
            GoodsError::Missing(PROVISIONS)
        );
    }
}
