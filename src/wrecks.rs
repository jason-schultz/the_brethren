//! Wrecks: ships lost, and what's left of them.
//!
//! Every ship that goes down goes through [`Founder`]: sunk in battle,
//! burned after she was plundered, holed on a reef, or broken up running
//! hard aground. Word of it goes in the ledger (see [`crate::word`]), told
//! by whoever saw it and by her survivors, who row for the nearest port.
//! Within reach of land or a shoal the water is shallow enough to dive on
//! her, and she leaves a [`Wreck`] with whatever was in her hold and her
//! strongbox, until the sea takes it. Any ship hove to over a wreck
//! salvages it, her men bringing up a little at a time; a wreck belongs to
//! her nation, which calls salvaging it theft.
//!
//! Running aground hard enough holes a ship, and a ship sailing near
//! shoals may strike a reef. Either can sink her.

use bevy::ecs::query::QueryData;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::alert::{Alert, AlertKind};
use crate::cargo::{Hold, Lading, Purse};
use crate::clock::{DAY, HOUR, WorldClock};
use crate::combat::rounds::Dice;
use crate::combat::{Engaged, Timbers};
use crate::crew::{Complement, Crew};
use crate::economy::jobs::Carrier;
use crate::factions::{Colours, Deed, DeedKind, Flag, Group, GroupKind, Nation};
use crate::goods::{Carriage, Goods};
use crate::loyalty::{Boat, Captaincy, chance_in};
use crate::map::Land;
use crate::map::data::Terrain;
use crate::navigation::Route;
use crate::player::Player;
use crate::port::{Docked, Port};
use crate::sailing::{Grounded, Heading, Speed};
use crate::schedule::{FrameSet, SimSet};
use crate::word::{
    Fate, Happening, Heard, Knowledge, Ledger, Loss, Place, Tale, boat_to, in_those_waters,
    witnesses,
};

/// A wreck within this distance of land or a shoal, meters, lies shallow
/// enough to dive on.
pub const SHALLOW: f32 = 5_000.0;
/// A ship hove to within this distance of a wreck, meters, salvages it.
pub const SALVAGE_RANGE: f32 = 150.0;
/// Each man aboard brings up this many tons an hour.
const TONS_PER_MAN_HOUR: f64 = 0.05;
/// A ship this slow, meters a second, is hove to.
const HOVE_TO: f32 = 0.5;
/// The sea takes this share of a wreck's cargo a day...
const SEA_TAKES_PER_DAY: f64 = 0.02;
/// ...and breaks her up after this long.
const WRECK_DAYS: f64 = 30.0;
/// Running aground faster than this, meters a second, holes a ship...
const SAFE_GROUNDING: f32 = 1.5;
/// ...costing this much of her timbers for every meter a second over.
const GROUNDING_DAMAGE: f32 = 0.15;
/// A ship within this distance of a shoal, meters, may strike a reef...
const REEF_WATERS: f32 = 2_000.0;
/// ...with this chance a day...
const REEFS_PER_DAY: f32 = 0.03;
/// ...losing between these shares of her timbers.
const REEF_DAMAGE: (f32, f32) = (0.2, 0.7);

/// What's left of a ship that went down in shallow water: her name is
/// "Wreck of the ..." and her place is where she lies.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wreck {
    /// The nation she belonged to, whose she still is by right.
    pub nation: Option<Nation>,
    /// Her cargo, as much as the sea has left.
    pub cargo: Lading,
    /// Her strongbox, pieces of eight.
    pub purse: u32,
    /// [`WorldClock`] time the sea breaks her up.
    pub until: f64,
    /// The ships that have salvaged her.
    pub salvors: Vec<Entity>,
}

impl Wreck {
    /// Whether there's anything left to bring up.
    pub fn has_anything(&self) -> bool {
        self.purse > 0 || self.cargo.iter().any(|(_, tons)| tons >= 1.0)
    }
}

/// How a ship was lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// Holed in battle by this ship.
    Battle(Entity),
    /// Burned by the captain who took her.
    Burned(Entity),
    /// Broken up running hard aground.
    Grounding,
    /// Holed on a reef.
    Reef,
}

impl Cause {
    /// Whether her people lived to row for port and tell of it. Dead men
    /// tell no tales, and a burned prize's crew was set adrift already.
    fn survivors(self) -> bool {
        matches!(self, Self::Grounding | Self::Reef)
    }

    /// How she was lost, as the truth of it.
    fn fate(self) -> Fate {
        match self {
            Self::Battle(by) => Fate::Sunk(by),
            Self::Burned(by) => Fate::Taken(by),
            Self::Grounding | Self::Reef => Fate::Wrecked,
        }
    }
}

/// A ship goes down: the one way ships leave the world.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Founder {
    pub ship: Entity,
    pub cause: Cause,
}

/// The name for the wreck of a ship called `ship`: "Wreck of the Dove", or
/// "Wreck of the Dove (2)" if another Dove lies wrecked already. Names stay
/// unique, which saves rely on.
pub fn wreck_name<'a>(ship: &str, wrecks: impl Iterator<Item = &'a Name> + Clone) -> String {
    let base = format!("Wreck of the {ship}");
    (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base} ({n})")
            }
        })
        .find(|name| !wrecks.clone().any(|wreck| wreck.as_str() == name))
        .expect("there's always a free number")
}

/// The wrecks, and what's said of them in the ledger: how captains learn
/// of wrecks worth diving on, or guarding.
#[derive(SystemParam)]
pub struct WreckTalk<'w, 's> {
    pub ledger: Res<'w, Ledger>,
    pub wrecks: Query<'w, 's, (Entity, &'static Transform, &'static Wreck)>,
}

impl WreckTalk<'_, '_> {
    /// The wrecks worth diving on that are talked of at `port`, nearest
    /// `at` first.
    pub fn known_at(&self, port: Entity, at: Vec3) -> Vec<(Entity, Vec3)> {
        let mut known: Vec<(Entity, Vec3)> = self
            .ledger
            .news
            .iter()
            .filter(|news| news.known_at.contains(&port))
            .filter_map(|news| match news.event {
                Happening::Wrecked {
                    wreck: Some(wreck), ..
                } => self.wrecks.get(wreck).ok(),
                _ => None,
            })
            .filter(|(_, _, wreck)| wreck.has_anything())
            .map(|(wreck, lies, _)| (wreck, lies.translation))
            .collect();
        known.sort_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)));
        known
    }
}

/// Whether a wreck at `at` lies shallow enough to dive on.
pub fn shallow(land: &Land, at: Vec3) -> bool {
    land.clearance(at, at) <= SHALLOW
}

/// Whether a ship at `at` sails in reef waters, near a shoal.
pub fn in_reef_waters(land: &Land, at: Vec3) -> bool {
    land.distance_to(at, Terrain::Shoal) <= REEF_WATERS
}

/// Damage to a ship's timbers from running aground at `speed_mps`.
pub fn grounding_damage(speed_mps: f32) -> f32 {
    (speed_mps - SAFE_GROUNDING).max(0.0) * GROUNDING_DAMAGE
}

/// Brings up to `tons` from `wreck` into a hold with `room` tons free: her
/// strongbox first, then her cargo, the most valuable by the ton first.
/// Returns the pieces of eight and the cargo brought up.
pub fn salvage(goods: &Goods, wreck: &mut Wreck, tons: f64, room: f64) -> (u32, Lading) {
    let silver = std::mem::take(&mut wreck.purse);
    let mut haul = Lading::default();
    let mut left = tons.min(room).max(0.0);
    let mut cargo: Vec<_> = wreck
        .cargo
        .iter()
        .filter(|&(good, _)| goods.carriage(good) == Carriage::Hold)
        .collect();
    cargo.sort_by(|a, b| {
        goods
            .get(b.0)
            .base_price
            .total_cmp(&goods.get(a.0).base_price)
    });
    for (good, _) in cargo {
        if left <= 0.0 {
            break;
        }
        let brought = wreck.cargo.remove(good, left);
        haul.add(good, brought);
        left -= brought;
    }
    (silver, haul)
}

pub struct WrecksPlugin;

impl Plugin for WrecksPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Founder>()
            // Ships sunk by orders given while paused go down at once, after
            // word of the deed is recorded.
            .add_systems(
                Update,
                go_down
                    .after(FrameSet::Orders)
                    .after(crate::word::record_deeds),
            )
            .add_systems(
                FixedUpdate,
                (
                    (run_aground, strike_reefs).in_set(SimSet::Resolve),
                    salvage_wrecks.in_set(SimSet::Economy),
                ),
            );
    }
}

/// A ship going down, as her wreck and the news will remember her.
#[derive(QueryData)]
struct Foundering {
    name: &'static Name,
    at: &'static Transform,
    heading: &'static Heading,
    hold: &'static Hold,
    purse: &'static Purse,
    crew: &'static Crew,
    flag: Flag,
    is_player: Has<Player>,
}

/// Word of a ship lost: the ships in sight who saw it and those in those
/// waters, the ports her boats make for, the captains who know it, and the
/// ledger it goes in.
#[derive(SystemParam)]
struct WordOfLoss<'w, 's> {
    at_sea: Query<'w, 's, (Entity, &'static Name, &'static Transform), AtSea>,
    ports: Query<'w, 's, (Entity, &'static Transform), With<Port>>,
    captains: Query<'w, 's, (Entity, &'static mut Knowledge)>,
    ledger: ResMut<'w, Ledger>,
}

impl WordOfLoss<'_, '_> {
    /// `ship` has gone down at `now` by `cause`, leaving `wreck` if any: the
    /// loss is noted, and unless she was a prize burned (news already, as
    /// a capture), word of it goes about with those who saw it and the
    /// boats.
    fn tell(
        &mut self,
        now: f64,
        ship: Entity,
        lost: &FounderingItem,
        cause: Cause,
        wreck: Option<Entity>,
    ) {
        let position = lost.at.translation;
        let saw = witnesses(
            position,
            self.at_sea
                .iter()
                .map(|(other, _, at)| (other, at.translation)),
            &[ship],
        );
        let nearby = in_those_waters(
            position,
            ship,
            self.at_sea
                .iter()
                .map(|(other, name, at)| (other, name, at.translation)),
        );
        self.ledger.note_loss(Loss {
            ship,
            name: lost.name.to_string(),
            group: Group::of(lost.flag.colours, lost.flag.serves),
            fate: cause.fate(),
            at: now,
            nearby,
        });
        if matches!(cause, Cause::Burned(_)) {
            return;
        }
        let boats = cause
            .survivors()
            .then(|| {
                boat_to(
                    position,
                    now,
                    self.ports.iter().map(|(p, at)| (p, at.translation)),
                )
            })
            .flatten()
            .into_iter()
            .collect();
        let hold = lost.hold;
        let largest = hold
            .owned
            .iter()
            .chain(hold.consigned.iter())
            .filter(|&(_, tons)| tons >= 1.0)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(good, tons)| (good, tons as u32));
        let event = Happening::Wrecked {
            ship,
            name: lost.name.to_string(),
            nation: lost.flag.colours.map(|colours| colours.0),
            cargo: largest,
            wreck,
        };
        let place = Place::Sea(position);
        let id = self
            .ledger
            .record(now, place, event.clone(), Vec::new(), saw.clone(), boats);
        for (_, mut knowledge) in self
            .captains
            .iter_mut()
            .filter(|(captain, _)| saw.contains(captain))
        {
            knowledge.0.push(Tale {
                news: id,
                at: now,
                place,
                event: event.clone(),
                heard: Heard::Saw,
            });
        }
    }
}

/// Every ship going down: word of it goes in the ledger, she leaves a
/// wreck if the water's shallow, and she's gone. A player wrecked on a
/// reef or the shore takes to the boat with her men.
fn go_down(
    mut founders: MessageReader<Founder>,
    clock: Res<WorldClock>,
    land: Res<Land>,
    ships: Query<Foundering>,
    players: Query<(), With<Player>>,
    jobs: Query<(Entity, &Carrier)>,
    wreck_names: Query<&Name, With<Wreck>>,
    mut word: WordOfLoss,
    mut captaincy: Captaincy,
    mut alerts: MessageWriter<Alert>,
) {
    let now = clock.elapsed;
    let mut gone = Vec::new();
    for &Founder { ship, cause } in founders.read() {
        if gone.contains(&ship) {
            continue;
        }
        let Ok(lost) = ships.get(ship) else {
            // Nothing to tell of or leave behind, but she's gone all the same.
            if !players.contains(ship) {
                captaincy.commands.entity(ship).try_despawn();
            }
            continue;
        };
        gone.push(ship);
        let position = lost.at.translation;
        let mut cargo = lost.hold.owned.clone();
        for (good, tons) in lost.hold.consigned.iter() {
            cargo.add(good, tons);
        }
        let wreck = Wreck {
            nation: lost.flag.colours.map(|colours| colours.0),
            cargo,
            purse: lost.purse.0,
            until: now + WRECK_DAYS * DAY,
            salvors: Vec::new(),
        };
        let wreck = (shallow(&land, position) && wreck.has_anything()).then(|| {
            captaincy
                .commands
                .spawn((
                    Name::new(wreck_name(lost.name.as_str(), wreck_names.iter())),
                    wreck,
                    Transform::from_translation(position),
                ))
                .id()
        });
        word.tell(now, ship, &lost, cause, wreck);
        for (job, carrier) in &jobs {
            if carrier.0 == ship {
                captaincy.commands.entity(job).despawn();
            }
        }
        if lost.is_player {
            // The player sunk in battle stays where she went down.
            if cause.survivors() {
                let boat = Boat {
                    at: position,
                    heading: lost.heading.0,
                    docked: None,
                    hands: lost.crew.count,
                    purse: 0,
                };
                let boat = captaincy.lose(ship, boat, None);
                alerts.write(Alert {
                    ship: boat,
                    kind: AlertKind::Wrecked { wreck },
                });
            }
        } else {
            captaincy.commands.entity(ship).despawn();
        }
    }
}

/// Running aground hard holes a ship, and can break her up.
fn run_aground(
    mut groundings: MessageReader<Grounded>,
    mut ships: Query<&mut Timbers>,
    mut alerts: MessageWriter<Alert>,
    mut founders: MessageWriter<Founder>,
) {
    for &Grounded { ship, speed_mps } in groundings.read() {
        let damage = grounding_damage(speed_mps);
        if damage > 0.0 {
            hole(
                ship,
                damage,
                Cause::Grounding,
                &mut ships,
                &mut alerts,
                &mut founders,
            );
        }
    }
}

/// Every hour, a ship sailing in reef waters may strike.
fn strike_reefs(
    time: Res<Time>,
    clock: Res<WorldClock>,
    land: Res<Land>,
    mut dice: ResMut<Dice>,
    at_sea: Query<(Entity, &Transform), (With<Timbers>, AtSea)>,
    mut ships: Query<&mut Timbers>,
    mut alerts: MessageWriter<Alert>,
    mut founders: MessageWriter<Founder>,
) {
    let hours = clock.periods_begun(time.delta_secs_f64(), HOUR);
    if hours == 0 {
        return;
    }
    let chance = chance_in(REEFS_PER_DAY, (hours as f64 * HOUR / DAY) as f32);
    for (ship, at) in &at_sea {
        if !in_reef_waters(&land, at.translation) || dice.roll() >= chance {
            continue;
        }
        let (least, most) = REEF_DAMAGE;
        let damage = least + (most - least) * dice.roll();
        hole(
            ship,
            damage,
            Cause::Reef,
            &mut ships,
            &mut alerts,
            &mut founders,
        );
    }
}

/// Takes `damage` off `ship`'s timbers: she's holed, or she goes down.
fn hole(
    ship: Entity,
    damage: f32,
    cause: Cause,
    ships: &mut Query<&mut Timbers>,
    alerts: &mut MessageWriter<Alert>,
    founders: &mut MessageWriter<Founder>,
) {
    let Ok(mut timbers) = ships.get_mut(ship) else {
        return;
    };
    timbers.0 = (timbers.0 - damage).max(0.0);
    if timbers.0 <= 0.0 {
        founders.write(Founder { ship, cause });
    } else {
        alerts.write(Alert {
            ship,
            kind: AlertKind::Holed {
                timbers: (timbers.0 * 100.0).round() as u8,
                on_reef: cause == Cause::Reef,
            },
        });
    }
}

/// A ship at sea: not in port, and not a wreck.
type AtSea = (Without<Docked>, Without<Wreck>);

/// A ship lying to at sea: not in battle, and with no course to sail.
type LyingTo = (AtSea, Without<Engaged>, Without<Route>);

/// A ship diving on a wreck.
#[derive(QueryData)]
#[query_data(mutable)]
struct Salvor {
    entity: Entity,
    at: &'static Transform,
    speed: &'static Speed,
    hold: &'static mut Hold,
    purse: &'static mut Purse,
    crew: &'static Crew,
    complement: &'static Complement,
    colours: Option<&'static Colours>,
}

/// The sea takes its share of every wreck, and breaks her up in time.
/// Every ship hove to over a wreck brings up what her men can and her hold
/// has room for. Salvaging another nation's wreck is a deed against its
/// governor, once for each ship.
fn salvage_wrecks(
    time: Res<Time>,
    clock: Res<WorldClock>,
    goods: Res<Goods>,
    mut wrecks: Query<(Entity, &Transform, &mut Wreck)>,
    mut ships: Query<Salvor, LyingTo>,
    mut deeds: MessageWriter<Deed>,
    mut alerts: MessageWriter<Alert>,
    mut commands: Commands,
) {
    let days = time.delta_secs_f64() / DAY;
    let hours = time.delta_secs_f64() / HOUR;
    for (wreck_entity, lies, mut wreck) in &mut wrecks {
        if clock.elapsed >= wreck.until {
            commands.entity(wreck_entity).despawn();
            continue;
        }
        let kept = (1.0 - SEA_TAKES_PER_DAY * days).max(0.0);
        let cargo: Vec<_> = wreck.cargo.iter().collect();
        for (good, tons) in cargo {
            wreck.cargo.remove(good, tons * (1.0 - kept));
        }
        for mut salvor in &mut ships {
            let ship = salvor.entity;
            let lying_over = salvor.at.translation.distance(lies.translation) <= SALVAGE_RANGE;
            if !lying_over || salvor.speed.0 > HOVE_TO {
                continue;
            }
            if !wreck.salvors.contains(&ship) {
                wreck.salvors.push(ship);
                alerts.write(Alert {
                    ship,
                    kind: AlertKind::Salvaging {
                        wreck: wreck_entity,
                    },
                });
                if let Some(nation) = wreck.nation
                    && salvor.colours.map(|colours| colours.0) != Some(nation)
                {
                    deeds.write(Deed {
                        doer: ship,
                        kind: DeedKind::Salvaged,
                        group: Group {
                            nation,
                            kind: GroupKind::Governor,
                        },
                        about: Some(wreck_entity),
                    });
                }
            }
            let (men, berths) = (salvor.crew.count, salvor.complement.berths);
            let room = salvor.hold.room(&goods, Carriage::Hold, men, berths);
            let tons = TONS_PER_MAN_HOUR * men as f64 * hours;
            let (silver, haul) = salvage(&goods, &mut wreck, tons, room);
            salvor.purse.0 += silver;
            for (good, tons) in haul.iter() {
                salvor.hold.owned.add(good, tons);
            }
        }
        if !wreck.has_anything() {
            commands.entity(wreck_entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::LandMass;
    use crate::ship_class::ShipClass;
    use crate::test_support::{run_checked, sim_app};
    use crate::time_scale::MAX_STEP;

    #[test]
    fn a_second_wreck_of_the_same_name_is_numbered() {
        let first = [Name::new("Wreck of the Dove")];
        assert_eq!(wreck_name("Dove", [].iter()), "Wreck of the Dove");
        assert_eq!(wreck_name("Dove", first.iter()), "Wreck of the Dove (2)");
    }

    #[test]
    fn only_a_hard_grounding_holes_her() {
        assert_eq!(grounding_damage(1.0), 0.0);
        assert!((grounding_damage(5.0) - 0.525).abs() < 1e-5);
    }

    #[test]
    fn divers_bring_up_silver_first_then_the_dearest_cargo() {
        let goods = Goods::standard();
        let (sugar, indigo) = (goods.id("sugar").unwrap(), goods.id("indigo").unwrap());
        let mut cargo = Lading::default();
        cargo.add(sugar, 20.0);
        cargo.add(indigo, 5.0);
        cargo.add(goods.soldiers, 10.0);
        let mut wreck = Wreck {
            nation: None,
            cargo,
            purse: 120,
            until: DAY,
            salvors: Vec::new(),
        };
        let (silver, haul) = salvage(&goods, &mut wreck, 8.0, 100.0);
        assert_eq!(silver, 120);
        assert_eq!((haul.get(indigo), haul.get(sugar)), (5.0, 3.0));
        assert_eq!(haul.get(goods.soldiers), 0.0, "the drowned stay down");
        let (_, more) = salvage(&goods, &mut wreck, 100.0, 4.0);
        assert_eq!(more.get(sugar), 4.0, "as much as there's room for");
    }

    /// A sea with a shoal to the north, 3 km off.
    fn reef_sea() -> App {
        let mut app = sim_app();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let square = |x: f32, z: f32, half: f32| {
            vec![
                Vec2::new(x - half, z - half),
                Vec2::new(x + half, z - half),
                Vec2::new(x + half, z + half),
                Vec2::new(x - half, z + half),
            ]
        };
        app.world_mut().resource_mut::<Land>().masses = vec![LandMass::new(
            "Banks",
            Terrain::Shoal,
            square(0.0, -4_000.0, 1_000.0),
            None,
        )];
        app
    }

    fn merchant(app: &mut App, at: Vec3) -> Entity {
        let goods = app.world().resource::<Goods>().clone();
        let mut hold = ShipClass::sloop().empty_hold();
        hold.owned.add(goods.id("sugar").unwrap(), 20.0);
        app.world_mut()
            .spawn((
                Name::new("Dove"),
                ShipClass::sloop().build(10, 4, hold),
                Purse(50),
                Colours(Nation::SPAIN),
                Transform::from_translation(at),
            ))
            .id()
    }

    #[test]
    fn a_ship_lost_in_shallows_leaves_a_wreck_and_word_of_it() {
        let mut app = reef_sea();
        let dove = merchant(&mut app, Vec3::ZERO);
        let deep = merchant(&mut app, Vec3::new(0.0, 0.0, 50_000.0));
        app.world_mut().write_message(Founder {
            ship: dove,
            cause: Cause::Reef,
        });
        app.world_mut().write_message(Founder {
            ship: deep,
            cause: Cause::Battle(dove),
        });
        app.update();
        let world = app.world_mut();
        assert!(world.get_entity(dove).is_err() && world.get_entity(deep).is_err());
        let wrecks: Vec<Wreck> = world.query::<&Wreck>().iter(world).cloned().collect();
        assert_eq!(wrecks.len(), 1, "the deep one is gone for good");
        assert_eq!(wrecks[0].purse, 50);
        assert_eq!(wrecks[0].nation, Some(Nation::SPAIN));
        let ledger = world.resource::<Ledger>();
        assert_eq!(ledger.news.len(), 2, "both are news");
        let reef = ledger
            .news
            .iter()
            .find(|news| matches!(news.event, Happening::Wrecked { wreck: Some(_), .. }))
            .unwrap();
        assert!(reef.boats.is_empty(), "no port to row for on this map");
    }

    #[test]
    fn a_ship_hove_to_over_a_wreck_salvages_her_and_offends_her_nation() {
        let mut app = reef_sea();
        let dove = merchant(&mut app, Vec3::ZERO);
        app.world_mut().write_message(Founder {
            ship: dove,
            cause: Cause::Reef,
        });
        app.update();
        let diver = merchant(&mut app, Vec3::new(50.0, 0.0, 0.0));
        app.world_mut()
            .entity_mut(diver)
            .insert(Colours(Nation::ENGLAND));
        let start = app.world().get::<Purse>(diver).unwrap().0;
        let mut deeds = Vec::new();
        run_checked(&mut app, 3.0 * HOUR as f32, |world| {
            let messages = world.resource::<Messages<Deed>>();
            deeds.extend(
                messages
                    .iter_current_update_messages()
                    .map(|deed| deed.kind),
            );
        });
        assert_eq!(deeds, [DeedKind::Salvaged], "once");
        assert_eq!(app.world().get::<Purse>(diver).unwrap().0, start + 50);
        let sugar = app.world().resource::<Goods>().id("sugar").unwrap();
        let brought = app.world().get::<Hold>(diver).unwrap().owned.get(sugar);
        assert!(brought > 20.0 + 1.0, "ten men, three hours: {brought}");
    }

    #[test]
    fn reefs_lie_near_shoals_not_in_open_water() {
        let app = reef_sea();
        let land = app.world().resource::<Land>();
        assert!(in_reef_waters(land, Vec3::new(0.0, 0.0, -2_500.0)));
        assert!(!in_reef_waters(land, Vec3::new(0.0, 0.0, 50_000.0)));
        assert!(shallow(land, Vec3::ZERO));
        assert!(!shallow(land, Vec3::new(0.0, 0.0, 50_000.0)));
    }
}
