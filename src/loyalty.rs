//! Loyalty: what officers do with their hidden hearts, and what becomes of
//! a captain who misjudges them.
//!
//! Disloyal officers give themselves away, a little at a time: the
//! ambitious grumble with the hands, and turncoats keep company with the
//! law. Loyal officers now and then look suspicious too. An ambitious
//! officer leads a mutiny once morale is low enough for the men to follow;
//! a turncoat sells his captain out when she docks in a port of a nation
//! she's crossed. Either way a captain who loses her ship is left with a
//! boat, the men who stood by her, and a stained name.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::alert::{Alert, AlertKind, InSight};
use crate::cargo::Purse;
use crate::clock::DAY;
use crate::combat::Engaged;
use crate::combat::rounds::Dice;
use crate::crew::Crew;
use crate::economy::jobs::Carrier;
use crate::factions::{
    CaptainRecord, Colours, Deed, DeedKind, Group, GroupKind, Nation, Standing, nations,
};
use crate::goods::Goods;
use crate::math::heading_to_rotation;
use crate::navigation::{Arrival, Arrived};
use crate::officers::{Berth, Loyalty, Officer, OfficerAct, OfficerOrder, reassign};
use crate::player::Player;
use crate::port::{Docked, Port};
use crate::prizes::{Prize, reassign_prizes};
use crate::sailing::Heading;
use crate::schedule::SimSet;
use crate::ship_class::ShipClass;

/// Below this morale, an ambitious officer finds men to follow him...
pub const MUTINY_MORALE: f32 = 30.0;
/// ...and below this, the hands rise without one.
const LEADERLESS_MUTINY_MORALE: f32 = 10.0;
/// The daily chance that a crew ripe for mutiny rises.
const MUTINY_PER_DAY: f32 = 0.5;
/// Each loyal officer in a fight for the ship is worth this many hands.
const OFFICER_DEFENDS: u32 = 2;
/// Of a failed mutiny's men, this fraction is killed or hanged...
const MUTINEERS_LOST: f64 = 0.5;
/// ...and the rest settle down to this morale, the air cleared.
const AFTER_MUTINY_MORALE: f32 = 40.0;
/// The chance a turncoat sells his captain out on docking.
const BETRAYAL_CHANCE: f32 = 0.5;
/// What a captain keeps of her purse, set adrift: what's in her coat.
const KEPT_ADRIFT: f64 = 0.2;
/// What she keeps of it after paying her fine and her bribes, arrested.
const KEPT_ARRESTED: f64 = 0.5;
/// Hands who wait for an arrested captain with a boat.
const HANDS_WHO_WAIT: u32 = 2;
/// Tons of provisions in the boat: a few days' worth.
const BOAT_PROVISIONS: f64 = 0.3;
/// Daily chances of each sign of disloyalty, and of a loyal officer giving
/// a false one.
const GRUMBLING_PER_DAY: f32 = 0.3;
/// The ambitious grumble only once the men are less than content.
const GRUMBLING_MORALE: f32 = 60.0;
const ASHORE_PER_DAY: f32 = 0.5;
const LETTERS_PER_DAY: f32 = 0.15;
const FALSE_SIGN_PER_DAY: f32 = 0.04;
/// The boat's name, numbered if there's more than one.
const BOAT_NAME: &str = "Jolly Boat";

/// A sign of what an officer is up to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rumour {
    /// Heard grumbling with the hands: ambition.
    Grumbling,
    /// Seen ashore with the law: a turncoat, in port.
    Ashore,
    /// Writing letters he shows no one: a turncoat, at sea.
    Letters,
}

/// The chance of something with `per_day` odds happening in `days`.
pub fn chance_in(per_day: f32, days: f32) -> f32 {
    1.0 - (1.0 - per_day).powf(days)
}

/// The sign `officer` may give, and its daily chance. `lawful_port` is
/// whether she lies at a port of a nation with a navy to pay informers.
pub fn sign(
    officer: &Officer,
    docked: bool,
    lawful_port: bool,
    morale: f32,
) -> Option<(Rumour, f32)> {
    if officer.confined {
        return None;
    }
    match officer.loyalty {
        Loyalty::Ambitious if morale < GRUMBLING_MORALE => {
            Some((Rumour::Grumbling, GRUMBLING_PER_DAY))
        }
        Loyalty::Ambitious => None,
        Loyalty::Turncoat if !docked => Some((Rumour::Letters, LETTERS_PER_DAY)),
        Loyalty::Turncoat if lawful_port => Some((Rumour::Ashore, ASHORE_PER_DAY)),
        Loyalty::Turncoat => None,
        Loyalty::Loyal if !docked => Some((Rumour::Letters, FALSE_SIGN_PER_DAY)),
        Loyalty::Loyal if lawful_port => Some((Rumour::Ashore, FALSE_SIGN_PER_DAY)),
        Loyalty::Loyal => None,
    }
}

/// How many of a crew of `crew` follow a mutiny at `morale`.
pub fn mutineers(crew: u32, morale: f32) -> u32 {
    (crew as f32 * (1.0 - morale / 50.0).clamp(0.0, 1.0)).round() as u32
}

/// Whether a mutiny takes the ship: the mutineers against the rest of the
/// hands, the loyal officers, and the captain.
pub fn mutiny_succeeds(crew: u32, morale: f32, loyal_officers: u32) -> bool {
    let rising = mutineers(crew, morale);
    rising > crew - rising + loyal_officers * OFFICER_DEFENDS + 1
}

/// Whether a port's nation pays informers against a captain with
/// `standing` there: one who has crossed it, but isn't wanted, since a
/// wanted captain isn't let in.
pub fn informers_paid(nation: Nation, standing: &Standing) -> bool {
    nation.is_crown() && standing.with(nation) < 0.0 && !standing.wanted_by(nation)
}

/// The first name in the boat's line that no ship bears.
fn boat_name<'a>(names: impl Iterator<Item = &'a Name> + Clone) -> String {
    (1..)
        .map(|n| match n {
            1 => BOAT_NAME.to_string(),
            n => format!("{BOAT_NAME} {n}"),
        })
        .find(|candidate| !names.clone().any(|name| name.as_str() == candidate))
        .expect("there's always an unused number")
}

pub struct LoyaltyPlugin;

impl Plugin for LoyaltyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (scheme, mutiny, betray).chain().in_set(SimSet::Resolve),
        );
    }
}

/// Officers aboard the player's ship give signs of what they're about.
fn scheme(
    time: Res<Time>,
    mut dice: ResMut<Dice>,
    player: Single<(Entity, &Crew, Option<&Docked>), With<Player>>,
    ports: Query<&Port>,
    officers: Query<(Entity, &Officer, &Berth)>,
    mut alerts: MessageWriter<Alert>,
) {
    let days = (time.delta_secs_f64() / DAY) as f32;
    let (ship, crew, docked) = *player;
    let lawful = docked
        .and_then(|docked| ports.get(docked.port).ok())
        .is_some_and(|port| port.flag.is_crown());
    for (officer_entity, officer, berth) in &officers {
        if *berth != Berth::Aboard(ship) {
            continue;
        }
        let Some((rumour, per_day)) = sign(officer, docked.is_some(), lawful, crew.morale) else {
            continue;
        };
        if dice.roll() < chance_in(per_day, days) {
            alerts.write(Alert {
                ship,
                kind: AlertKind::Rumour {
                    officer: officer_entity,
                    rumour,
                },
            });
        }
    }
}

/// What a lost ship's captain is left with.
pub(crate) struct Boat {
    pub at: Vec3,
    pub heading: f32,
    pub docked: Option<Entity>,
    pub hands: u32,
    pub purse: u32,
}

/// The ship and fleet a captain loses, and what she keeps.
#[derive(SystemParam)]
pub(crate) struct Captaincy<'w, 's> {
    goods: Res<'w, Goods>,
    names: Query<'w, 's, &'static Name>,
    colours: Query<'w, 's, &'static Colours>,
    prizes: Query<'w, 's, (Entity, &'static mut Prize, Option<&'static Docked>)>,
    officers: Query<'w, 's, (Entity, &'static Officer, &'static mut Berth)>,
    jobs: Query<'w, 's, (Entity, &'static Carrier)>,
    deeds: MessageWriter<'w, Deed>,
    pub commands: Commands<'w, 's>,
}

impl Captaincy<'_, '_> {
    /// Takes `ship` from the player, leaving her `boat`. Her prizes and loyal
    /// officers follow her into it, except prizes lying at `seized_at`; her
    /// contracts go with the ship. Word gets around. Returns the boat.
    pub(crate) fn lose(&mut self, ship: Entity, boat: Boat, seized_at: Option<Entity>) -> Entity {
        let mut hold = ShipClass::boat().empty_hold();
        hold.owned.add(self.goods.provisions, BOAT_PROVISIONS);
        let mut spawned = self.commands.spawn((
            Name::new(boat_name(self.names.iter())),
            Player,
            InSight::default(),
            ShipClass::boat().build(boat.hands, 0, hold),
            Purse(boat.purse),
            Heading(boat.heading),
            Transform::from_translation(boat.at).with_rotation(heading_to_rotation(boat.heading)),
        ));
        if let Some(port) = boat.docked {
            spawned.insert(Docked { port });
        }
        if let Ok(&colours) = self.colours.get(ship) {
            spawned.insert(colours);
        }
        let new = spawned.id();
        for (prize, captor, docked) in &self.prizes {
            let seized = docked.is_some_and(|docked| Some(docked.port) == seized_at);
            if captor.captor == ship && seized {
                self.commands.entity(prize).despawn();
            }
        }
        reassign_prizes(self.prizes.iter_mut().map(|(_, prize, _)| prize), ship, new);
        reassign(
            self.officers
                .iter_mut()
                .map(|(_, officer, berth)| (officer, berth)),
            ship,
            new,
            |officer| officer.loyalty == Loyalty::Loyal,
        );
        for (job, carrier) in &self.jobs {
            if carrier.0 == ship {
                self.commands.entity(job).despawn();
            }
        }
        // Her name with the nations goes with her into the boat.
        self.commands
            .entity(ship)
            .move_components::<CaptainRecord>(new)
            .despawn();
        for nation in nations() {
            self.deeds.write(Deed {
                doer: new,
                kind: DeedKind::LostCommand,
                // The law takes note.
                group: Group {
                    nation,
                    kind: GroupKind::Governor,
                },
                about: None,
            });
        }
        new
    }
}

/// A crew ripe for mutiny may rise at sea, led by an ambitious officer or,
/// past bearing, by one of their own. Put down, the ringleader goes in
/// irons; otherwise the captain is set adrift.
fn mutiny(
    time: Res<Time>,
    mut dice: ResMut<Dice>,
    mut player: Query<
        (Entity, &Transform, &Heading, &mut Crew, &Purse),
        (With<Player>, Without<Docked>, Without<Engaged>),
    >,
    mut captaincy: Captaincy,
    mut alerts: MessageWriter<Alert>,
    mut orders: MessageWriter<OfficerOrder>,
) {
    let Ok((ship, at, heading, mut crew, purse)) = player.single_mut() else {
        return;
    };
    if crew.morale >= MUTINY_MORALE {
        return;
    }
    let free = |loyalty| {
        captaincy
            .officers
            .iter()
            .filter(move |(_, officer, berth)| {
                **berth == Berth::Aboard(ship) && !officer.confined && officer.loyalty == loyalty
            })
            .map(|(entity, officer, _)| (entity, officer.role))
    };
    let ringleader = free(Loyalty::Ambitious).next();
    if ringleader.is_none() && crew.morale >= LEADERLESS_MUTINY_MORALE {
        return;
    }
    let days = (time.delta_secs_f64() / DAY) as f32;
    if dice.roll() >= chance_in(MUTINY_PER_DAY, days) {
        return;
    }
    let loyal = free(Loyalty::Loyal).count() as u32;
    let rising = mutineers(crew.count, crew.morale);
    if mutiny_succeeds(crew.count, crew.morale, loyal) {
        let boat = Boat {
            at: at.translation,
            heading: heading.0,
            docked: None,
            hands: crew.count - rising,
            purse: (purse.0 as f64 * KEPT_ADRIFT) as u32,
        };
        let boat = captaincy.lose(ship, boat, None);
        alerts.write(Alert {
            ship: boat,
            kind: AlertKind::Mutiny {
                leader: ringleader.map(|(_, role)| role),
            },
        });
        return;
    }
    crew.take_casualties(rising as f64 * MUTINEERS_LOST);
    crew.morale = AFTER_MUTINY_MORALE;
    if let Some((leader, _)) = ringleader {
        orders.write(OfficerOrder {
            ship,
            officer: leader,
            act: OfficerAct::Confine,
        });
    }
    alerts.write(Alert {
        ship,
        kind: AlertKind::MutinyPutDown {
            leader: ringleader.map(|(entity, _)| entity),
        },
    });
}

/// A turncoat may sell his captain out as she docks at a port of a nation
/// she's crossed: her ship and the prizes with her are seized, and she's
/// fined. Two hands wait for her with a boat.
fn betray(
    mut arrivals: MessageReader<Arrived>,
    mut dice: ResMut<Dice>,
    player: Query<(Entity, &Heading, &Purse, &Standing), With<Player>>,
    ports: Query<(&Port, &Transform)>,
    mut captaincy: Captaincy,
    mut alerts: MessageWriter<Alert>,
) {
    let Ok((ship, heading, purse, standing)) = player.single() else {
        return;
    };
    for arrived in arrivals.read() {
        let Arrival::Dock(port) = arrived.on_arrival else {
            continue;
        };
        let Ok((details, anchorage)) = ports.get(port) else {
            continue;
        };
        if arrived.ship != ship || !informers_paid(details.flag, standing) {
            continue;
        }
        let traitor = captaincy
            .officers
            .iter()
            .find(|(_, officer, berth)| {
                **berth == Berth::Aboard(ship)
                    && !officer.confined
                    && officer.loyalty == Loyalty::Turncoat
            })
            .map(|(_, officer, _)| officer.role);
        let Some(role) = traitor else {
            continue;
        };
        if dice.roll() >= BETRAYAL_CHANCE {
            continue;
        }
        let boat = Boat {
            at: anchorage.translation,
            heading: heading.0,
            docked: Some(port),
            hands: HANDS_WHO_WAIT,
            purse: (purse.0 as f64 * KEPT_ARRESTED) as u32,
        };
        let boat = captaincy.lose(ship, boat, Some(port));
        alerts.write(Alert {
            ship: boat,
            kind: AlertKind::Betrayed { role, port },
        });
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Hold;
    use crate::new_game::new_game;
    use crate::officers::Role;
    use crate::time_scale::MAX_STEP;

    fn officer(loyalty: Loyalty) -> Officer {
        Officer {
            role: Role::FirstMate,
            loyalty,
            confined: false,
        }
    }

    #[test]
    fn chances_add_up_over_time_whatever_the_step() {
        assert_eq!(chance_in(0.5, 1.0), 0.5);
        assert!((chance_in(0.5, 2.0) - 0.75).abs() < 1e-6);
        let steps = (0..86_400).fold(1.0_f32, |unlikely, _| {
            unlikely * (1.0 - chance_in(0.5, 1.0 / 86_400.0))
        });
        assert!((1.0 - steps - 0.5).abs() < 0.01, "{steps}");
    }

    #[test]
    fn disloyal_officers_give_themselves_away_and_irons_silence_them() {
        let ambitious = officer(Loyalty::Ambitious);
        assert_eq!(
            sign(&ambitious, false, false, 40.0).unwrap().0,
            Rumour::Grumbling
        );
        assert_eq!(
            sign(&ambitious, false, false, 70.0),
            None,
            "happy men don't listen"
        );
        let turncoat = officer(Loyalty::Turncoat);
        assert_eq!(sign(&turncoat, true, true, 50.0).unwrap().0, Rumour::Ashore);
        assert_eq!(
            sign(&turncoat, false, false, 50.0).unwrap().0,
            Rumour::Letters
        );
        assert_eq!(sign(&turncoat, true, false, 50.0), None, "no law in Nassau");
        let (_, false_alarm) = sign(&officer(Loyalty::Loyal), true, true, 50.0).unwrap();
        assert!(
            false_alarm < ASHORE_PER_DAY,
            "the loyal look guilty less often"
        );
        let confined = Officer {
            confined: true,
            ..turncoat
        };
        assert_eq!(sign(&confined, true, true, 50.0), None);
    }

    #[test]
    fn the_unhappier_the_men_the_more_rise_and_loyal_officers_hold_the_line() {
        assert_eq!(mutineers(12, 50.0), 0);
        assert!(mutineers(12, 10.0) > mutineers(12, 25.0));
        assert!(!mutiny_succeeds(12, 28.0, 0), "a few malcontents");
        assert!(mutiny_succeeds(12, 15.0, 0));
        assert!(!mutiny_succeeds(12, 15.0, 3), "three loyal officers");
    }

    #[test]
    fn only_a_crossed_lawful_nation_pays_informers() {
        let mut standing = Standing::default();
        assert!(
            !informers_paid(Nation::ENGLAND, &standing),
            "in good standing"
        );
        standing.set(Nation::ENGLAND, -10.0);
        assert!(informers_paid(Nation::ENGLAND, &standing));
        standing.set(Nation::ENGLAND, -50.0);
        assert!(
            !informers_paid(Nation::ENGLAND, &standing),
            "wanted: turned away"
        );
        standing.set(Nation::PIRATES, -10.0);
        assert!(!informers_paid(Nation::PIRATES, &standing));
    }

    #[test]
    fn boats_take_the_first_free_name() {
        let names = [Name::new("Jolly Boat"), Name::new("Jolly Boat 2")];
        assert_eq!(boat_name(names.iter()), "Jolly Boat 3");
        assert_eq!(boat_name([].iter()), "Jolly Boat");
    }

    /// The player at sea south of Port Royal, with a first mate of
    /// `loyalty` aboard and her crew at `morale`. No raider or patrol comes
    /// for her: a ship in action doesn't mutiny.
    fn at_sea_with(loyalty: Loyalty, morale: f32) -> (App, Entity, Entity) {
        let (mut app, player) = new_game();
        crate::new_game::calm_seas(&mut app);
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        app.world_mut()
            .write_message(crate::port::CastOff { ship: player });
        app.update();
        let mate = app
            .world_mut()
            .spawn((
                Name::new("Israel Hands"),
                officer(loyalty),
                Berth::Aboard(player),
            ))
            .id();
        app.world_mut().get_mut::<Crew>(player).unwrap().morale = morale;
        (app, player, mate)
    }

    fn player(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<Player>>()
            .single(app.world())
            .unwrap()
    }

    #[test]
    fn an_ambitious_mate_and_a_mutinous_crew_take_the_ship() {
        let (mut app, ship, mate) = at_sea_with(Loyalty::Ambitious, 5.0);
        let before = app
            .world()
            .get::<Standing>(ship)
            .unwrap()
            .with(Nation::ENGLAND);
        // Hungry men stay mutinous.
        let provisions = app.world().resource::<Goods>().provisions;
        app.world_mut()
            .get_mut::<Hold>(ship)
            .unwrap()
            .owned
            .remove(provisions, f64::MAX);
        let taken = crate::test_support::run_until(&mut app, 15.0 * DAY as f32, |world| {
            world.get_entity(ship).is_err()
        });
        assert!(taken, "the crew never rose");
        app.update();
        let boat = player(&mut app);
        let world = app.world();
        assert_eq!(world.get::<ShipClass>(boat), Some(&ShipClass::boat()));
        assert!(world.get_entity(mate).is_err(), "he sailed off with her");
        let standing = |app: &App| {
            app.world()
                .get::<Standing>(boat)
                .expect("her name went with her")
                .with(Nation::ENGLAND)
        };
        assert_eq!(standing(&app), before, "no word of it yet");
        // Word gets around once she makes port.
        let port_royal = app
            .world_mut()
            .query_filtered::<Entity, With<Port>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .entity_mut(boat)
            .insert(Docked { port: port_royal });
        crate::test_support::run_seconds(&mut app, 10.0);
        assert!(standing(&app) < before);
    }

    #[test]
    fn a_mutiny_without_enough_followers_ends_with_its_leader_in_irons() {
        let (mut app, ship, mate) = at_sea_with(Loyalty::Ambitious, 4.0);
        for role in [
            Role::SailingMaster,
            Role::Gunner,
            Role::Bosun,
            Role::Surgeon,
            Role::Quartermaster,
        ] {
            app.world_mut().spawn((
                Officer {
                    role,
                    loyalty: Loyalty::Loyal,
                    confined: false,
                },
                Berth::Aboard(ship),
            ));
        }
        // Left alone, morale would take more than eight days to reach this.
        let put_down = crate::test_support::run_until(&mut app, 8.0 * DAY as f32, |world| {
            world
                .get::<Crew>(ship)
                .is_some_and(|crew| crew.morale >= AFTER_MUTINY_MORALE)
        });
        assert!(put_down, "no rising, or it succeeded");
        app.update();
        assert_eq!(player(&mut app), ship);
        assert!(app.world().get::<Officer>(mate).unwrap().confined);
    }

    #[test]
    fn a_turncoat_sells_his_captain_out_in_a_port_she_has_crossed() {
        let (mut app, ship, _) = at_sea_with(Loyalty::Turncoat, 50.0);
        app.world_mut()
            .get_mut::<Standing>(ship)
            .unwrap()
            .set(Nation::ENGLAND, -10.0);
        let kingston = app
            .world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == "Kingston")
            .unwrap()
            .0;
        // He has an even chance each time she docks.
        for _ in 0..20 {
            if app.world().get_entity(ship).is_err() {
                break;
            }
            app.world_mut().write_message(Arrived {
                ship,
                on_arrival: Arrival::Dock(kingston),
            });
            app.update();
        }
        app.update();
        let boat = player(&mut app);
        assert_ne!(boat, ship, "never betrayed");
        let world = app.world();
        assert_eq!(world.get::<Docked>(boat), Some(&Docked { port: kingston }));
        assert_eq!(world.get::<Crew>(boat).unwrap().count, HANDS_WHO_WAIT);
        assert!(world.get::<Hold>(boat).unwrap().capacity_tons < 10.0);
    }
}
