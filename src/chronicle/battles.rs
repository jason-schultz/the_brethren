//! Battles, from the first ship engaging to both drawing off: who fought
//! in what, what each lost, and how it ended. Every ship's battles are
//! noted, not only the player's: they're how class matchups are judged.

use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use chronicle::{Battle, BattleEnd, Record, Side};

use super::{Chronicle, Noting, Roles, role};
use crate::clock::{DAY, WorldClock};
use crate::combat::{Battery, Engaged, Struck, Sunk, Timbers};
use crate::crew::Crew;
use crate::factions::{Colours, Deed, DeedKind};
use crate::sailing::Rigging;
use crate::ship_class::ShipClass;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Battles>()
        .add_observer(open_battle)
        .add_observer(strike)
        .add_observer(sink)
        .add_systems(Last, follow_battles.in_set(Noting));
}

/// The battles being fought.
#[derive(Resource, Debug, Default)]
struct Battles(Vec<Fight>);

#[derive(Debug)]
struct Fight {
    began: f64,
    /// The ship that started it first.
    ships: [Entity; 2],
    sides: [Side; 2],
    /// How far it went for each side.
    fates: [Option<BattleEnd>; 2],
}

impl Fight {
    /// Marks `ship`'s side as having come to `end`, if it's further than
    /// she'd come.
    fn mark(&mut self, ship: Entity, end: BattleEnd) {
        if let Some(index) = self.ships.iter().position(|&side| side == ship) {
            let fate = &mut self.fates[index];
            *fate = (*fate).max(Some(end));
        }
    }

    /// How it ended, and who lost: whoever came off worst.
    fn outcome(&self) -> (BattleEnd, Option<usize>) {
        match self.fates {
            [None, None] => (BattleEnd::BrokeOff, None),
            [first, second] if first >= second => (first.unwrap_or(BattleEnd::BrokeOff), Some(0)),
            [_, second] => (second.unwrap_or(BattleEnd::BrokeOff), Some(1)),
        }
    }
}

type Fighting<'a> = (
    &'a Name,
    &'a ShipClass,
    &'a Battery,
    (&'a Crew, &'a Timbers, &'a Rigging),
    Option<&'a Colours>,
    Roles,
);

/// A ship's side, as she is now: at the start and the end alike.
fn side(
    (name, class, battery, (crew, timbers, rigging), colours, roles): QueryItem<Fighting>,
) -> Side {
    Side {
        ship: name.to_string(),
        class: class.name.to_string(),
        role: role(roles),
        nation: colours.map(|colours| colours.0.to_string()),
        guns: battery.guns,
        crew: [crew.count; 2],
        timbers: [timbers.0; 2],
        rigging: [rigging.0; 2],
    }
}

/// A battle begins when a ship engages another she isn't already fighting.
fn open_battle(
    add: On<Add<Engaged>>,
    engaged: Query<&Engaged>,
    ships: Query<Fighting>,
    clock: Res<WorldClock>,
    mut battles: ResMut<Battles>,
) {
    let ship = add.entity;
    let Ok(&Engaged {
        enemy,
        aggressor: true,
    }) = engaged.get(ship)
    else {
        return;
    };
    if battles
        .0
        .iter()
        .any(|fight| fight.ships.contains(&ship) && fight.ships.contains(&enemy))
    {
        return;
    }
    let Ok([us, them]) = ships.get_many([ship, enemy]) else {
        return;
    };
    battles.0.push(Fight {
        began: clock.elapsed / DAY,
        ships: [ship, enemy],
        sides: [side(us), side(them)],
        fates: [None, None],
    });
}

fn strike(add: On<Add<Struck>>, mut battles: ResMut<Battles>) {
    for fight in &mut battles.0 {
        fight.mark(add.entity, BattleEnd::Struck);
    }
}

fn sink(add: On<Add<Sunk>>, mut battles: ResMut<Battles>) {
    for fight in &mut battles.0 {
        fight.mark(add.entity, BattleEnd::Sunk);
    }
}

/// Keeps each battle's sides up to date, marks ships taken, and notes each
/// battle once neither ship is fighting the other.
fn follow_battles(
    mut deeds: MessageReader<Deed>,
    engaged: Query<&Engaged>,
    ships: Query<(&Crew, &Timbers, &Rigging)>,
    mut battles: ResMut<Battles>,
    mut chronicle: ResMut<Chronicle>,
) {
    for deed in deeds.read().filter(|deed| deed.kind == DeedKind::Captured) {
        for fight in &mut battles.0 {
            if let Some(victim) = deed.about
                && fight.ships.contains(&deed.doer)
            {
                fight.mark(victim, BattleEnd::Taken);
            }
        }
    }
    battles.0.retain_mut(|fight| {
        for (ship, side) in fight.ships.iter().zip(&mut fight.sides) {
            if let Ok((crew, timbers, rigging)) = ships.get(*ship) {
                side.crew[1] = crew.count;
                side.timbers[1] = timbers.0;
                side.rigging[1] = rigging.0;
            }
        }
        let [a, b] = fight.ships;
        let fighting = |ship, enemy| engaged.get(ship).is_ok_and(|e| e.enemy == enemy);
        if fighting(a, b) || fighting(b, a) {
            return true;
        }
        let (end, loser) = fight.outcome();
        chronicle.note(Record::Battle(Battle {
            began: fight.began,
            sides: fight.sides.clone(),
            end,
            loser,
        }));
        false
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chronicle::Run;
    use crate::chronicle::tests::{noted, noting_app};
    use crate::combat::EngageOrder;
    use crate::merchants::{Merchant, Plan};
    use crate::raiders::{Raid, Raider};

    fn battles(app: &mut App) -> Vec<Battle> {
        noted(app)
            .into_iter()
            .filter_map(|record| match record {
                Record::Battle(battle) => Some(battle),
                _ => None,
            })
            .collect()
    }

    fn ship(app: &mut App, name: &str, at: Vec3) -> Entity {
        let class = ShipClass::sloop();
        app.world_mut()
            .spawn((
                Name::new(name.to_string()),
                Transform::from_translation(at),
                class.build(20, class.guns, class.empty_hold()),
            ))
            .id()
    }

    #[test]
    fn the_side_that_came_off_worst_lost() {
        let fight = |fates| Fight {
            began: 0.0,
            ships: [Entity::PLACEHOLDER; 2],
            sides: [side_for_test(), side_for_test()],
            fates,
        };
        assert_eq!(fight([None, None]).outcome(), (BattleEnd::BrokeOff, None));
        assert_eq!(
            fight([None, Some(BattleEnd::Taken)]).outcome(),
            (BattleEnd::Taken, Some(1))
        );
        assert_eq!(
            fight([Some(BattleEnd::Sunk), Some(BattleEnd::Struck)]).outcome(),
            (BattleEnd::Sunk, Some(0))
        );
    }

    fn side_for_test() -> Side {
        Side {
            ship: "Dove".into(),
            class: "Sloop".into(),
            role: chronicle::Role::Merchant,
            nation: None,
            guns: 8,
            crew: [20; 2],
            timbers: [1.0; 2],
            rigging: [1.0; 2],
        }
    }

    #[test]
    fn a_battle_is_noted_with_each_sides_losses_when_they_break_off() {
        let mut app = noting_app(Run::Soak);
        let raider = ship(&mut app, "Ranger", Vec3::ZERO);
        let merchant = ship(&mut app, "Dove", Vec3::new(300.0, 0.0, 0.0));
        app.world_mut().entity_mut(raider).insert(Raider {
            plan: Raid::Cruising { until: f64::MAX },
        });
        app.world_mut()
            .entity_mut(merchant)
            .insert(Merchant { plan: Plan::AtSea });
        app.world_mut().write_message(EngageOrder {
            ship: raider,
            target: merchant,
        });
        app.update();
        app.update();
        assert!(battles(&mut app).is_empty(), "still fighting");

        app.world_mut().get_mut::<Crew>(merchant).unwrap().count = 12;
        app.world_mut().get_mut::<Timbers>(merchant).unwrap().0 = 0.5;
        app.update();
        // They draw apart.
        app.world_mut()
            .get_mut::<Transform>(merchant)
            .unwrap()
            .translation
            .x = 50_000.0;
        let mut noted = Vec::new();
        for _ in 0..3 {
            app.update();
            noted.extend(battles(&mut app));
        }
        let [battle] = &noted[..] else {
            panic!("expected one battle, got {noted:?}");
        };
        assert_eq!(battle.end, BattleEnd::BrokeOff);
        assert_eq!(battle.loser, None);
        assert_eq!(battle.sides[0].ship, "Ranger");
        assert_eq!(battle.sides[0].role, chronicle::Role::Raider);
        assert_eq!(battle.sides[1].role, chronicle::Role::Merchant);
        assert_eq!(battle.sides[1].crew, [20, 12]);
        assert_eq!(battle.sides[1].timbers, [1.0, 0.5]);
    }

    #[test]
    fn a_ship_that_strikes_loses_the_battle() {
        let mut app = noting_app(Run::Soak);
        let raider = ship(&mut app, "Ranger", Vec3::ZERO);
        let merchant = ship(&mut app, "Dove", Vec3::new(300.0, 0.0, 0.0));
        app.world_mut().write_message(EngageOrder {
            ship: raider,
            target: merchant,
        });
        app.update();
        app.update();
        app.world_mut().entity_mut(merchant).insert(Struck);
        app.world_mut().entity_mut(raider).remove::<Engaged>();
        app.world_mut().entity_mut(merchant).remove::<Engaged>();
        app.update();
        let [battle] = &battles(&mut app)[..] else {
            panic!("expected one battle");
        };
        assert_eq!((battle.end, battle.loser), (BattleEnd::Struck, Some(1)));
    }
}
