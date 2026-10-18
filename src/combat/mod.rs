//! Battle at sea: broadsides, damage, and what captains do under fire.
//!
//! Any ship can be ordered to engage another with an [`EngageOrder`]. The
//! ship attacked joins the battle and decides whether to fight or run.
//! Engaged ships without a route of their own are steered by their tactic,
//! and every broadside fires as it bears and is loaded, unless the ship's
//! [`FireControl`] is manual. Battered ships strike their colours; holed
//! ones sink.

pub mod gunnery;
pub mod rounds;
pub mod tactics;

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;

use crate::alert::{Alert, AlertKind};
use crate::crew::{Complement, Crew};
use crate::factions::{Colours, Deed, DeedKind};
use crate::map::Land;
use crate::math::{heading_to_direction, normalize_heading};
use crate::navigation::Route;
use crate::officers::Afterguard;
use crate::player::Player;
use crate::port::Docked;
use crate::sailing::physics::sail_speed;
use crate::sailing::{Heading, Helm, Hull, Laden, Rigging, SailHandling, Sails};
use crate::schedule::{FrameSet, SimSet};
use crate::wind::Wind;
use gunnery::{Broadside, Damage, GUN_RANGE, RELOAD, bearing_broadside, damage, hits};
use rounds::{Dice, Fighter, Move, Range, Round};
use tactics::{BOARDING_RANGE, LIE_ALONGSIDE, Strength, Tactic, course, defend};

/// Ships farther apart than this, meters, have broken off the action.
pub const DISENGAGE_DISTANCE: f32 = 3_000.0;
/// How far ahead a helmsman in battle looks for land, meters...
const LOOKOUT_AHEAD: f32 = 600.0;
/// ...and how much sea room she keeps, meters.
const SEA_ROOM: f32 = 40.0;
/// Turns tried either side of a course that runs onto land, in order.
const FEELERS: [f32; 6] = [30.0, 60.0, 90.0, 120.0, 150.0, 180.0];

/// The heading nearest `course` that keeps sea room for `ahead` meters from
/// `from`, or `course` itself if every way is blocked.
fn sea_room(land: &Land, from: Vec3, course: f32, ahead: f32) -> f32 {
    let clear = |heading: f32| {
        land.clearance(from, from + heading_to_direction(heading) * ahead) >= SEA_ROOM
    };
    if clear(course) {
        return course;
    }
    FEELERS
        .iter()
        .flat_map(|degrees| [degrees.to_radians(), -degrees.to_radians()])
        .map(|turn| normalize_heading(course + turn))
        .find(|&heading| clear(heading))
        .unwrap_or(course)
}

/// A ship's guns, and how soon each broadside is loaded.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[require(Timbers, FireControl)]
pub struct Battery {
    /// Guns mounted, both sides together.
    pub guns: u32,
    /// Seconds until each broadside is loaded. Zero is ready.
    pub port: f32,
    pub starboard: f32,
}

impl Battery {
    pub fn new(guns: u32) -> Self {
        Self {
            guns,
            port: 0.0,
            starboard: 0.0,
        }
    }

    pub fn per_side(&self) -> u32 {
        self.guns / 2
    }

    fn reload_mut(&mut self, side: Broadside) -> &mut f32 {
        match side {
            Broadside::Port => &mut self.port,
            Broadside::Starboard => &mut self.starboard,
        }
    }

    pub fn reload(&self, side: Broadside) -> f32 {
        match side {
            Broadside::Port => self.port,
            Broadside::Starboard => self.starboard,
        }
    }
}

/// What's left of her hull, from 1 (sound) to 0 (sunk).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Timbers(pub f32);

impl Default for Timbers {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Who decides when the guns fire.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FireControl {
    /// Each broadside fires as soon as it bears and is loaded.
    #[default]
    Auto,
    /// Broadsides fire only on the captain's word.
    Manual { order: Option<Broadside> },
}

/// The enemy a ship is fighting.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Engaged {
    pub enemy: Entity,
    /// Whether she started it. Aggressors pursue; the attacked choose.
    pub aggressor: bool,
}

/// She has struck her colours: no more firing or sailing, waiting to be
/// boarded.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Struck;

/// The player's ship has gone down. Kept in the world so the game can say so.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sunk;

/// Order a ship to attack another.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngageOrder {
    pub ship: Entity,
    pub target: Entity,
}

/// A battle being fought by rounds rather than sailed out, and how it
/// stands.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Rounds {
    pub range: Range,
    /// The last move made, and what came of it.
    pub last: Option<(Move, Round)>,
}

impl Rounds {
    /// A battle by rounds starting with the enemy `distance` meters off.
    pub fn starting_at(distance: f32) -> Self {
        Self {
            range: Range::at(distance),
            last: None,
        }
    }
}

/// Order a ship fighting by [`Rounds`] to make her move.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundOrder {
    pub ship: Entity,
    pub play: Move,
}

/// A broadside was fired, for presentation.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct Volley {
    pub from: Vec3,
    pub to: Vec3,
}

/// Order a ship under manual fire control to fire a broadside.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FireOrder {
    pub ship: Entity,
    pub side: Broadside,
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EngageOrder>()
            .add_message::<FireOrder>()
            .add_message::<RoundOrder>()
            .add_message::<Volley>()
            .init_resource::<Dice>()
            // Orders work while paused, and their verdicts land at once.
            .add_systems(
                Update,
                (
                    (engage, queue_fire_orders, play_rounds).in_set(FrameSet::Orders),
                    strike_or_sink.after(FrameSet::Orders),
                ),
            )
            .add_systems(
                FixedUpdate,
                (
                    join_battle,
                    break_off,
                    rehoist_colours,
                    steer,
                    fire,
                    strike_or_sink,
                )
                    .chain()
                    .in_set(SimSet::Combat),
            );
    }
}

fn engage(
    mut orders: MessageReader<EngageOrder>,
    targets: Query<
        Option<&Colours>,
        (
            With<Battery>,
            Without<Docked>,
            Without<Struck>,
            Without<Sunk>,
        ),
    >,
    mut deeds: MessageWriter<Deed>,
    mut commands: Commands,
) {
    for order in orders.read() {
        let Ok(colours) = targets.get(order.target) else {
            continue;
        };
        if order.ship == order.target {
            continue;
        }
        if let Some(&Colours(nation)) = colours {
            deeds.write(Deed {
                doer: order.ship,
                kind: DeedKind::Attacked,
                nation,
            });
        }
        // Dropping her route hands the helm to her tactic.
        commands
            .entity(order.ship)
            .remove::<Route>()
            .try_insert(Engaged {
                enemy: order.target,
                aggressor: true,
            });
    }
}

fn queue_fire_orders(mut orders: MessageReader<FireOrder>, mut ships: Query<&mut FireControl>) {
    for order in orders.read() {
        if let Ok(mut control) = ships.get_mut(order.ship)
            && let FireControl::Manual { order: queued } = control.as_mut()
        {
            *queued = Some(order.side);
        }
    }
}

/// A ship under attack fights back. AI captains drop whatever voyage they
/// were on; the player keeps hers until she chooses otherwise.
fn join_battle(
    attackers: Query<(Entity, &Engaged)>,
    defenders: Query<Has<Player>, (With<Battery>, Without<Engaged>, Without<Struck>)>,
    mut alerts: MessageWriter<Alert>,
    mut commands: Commands,
) {
    for (attacker, engaged) in &attackers {
        let Ok(is_player) = defenders.get(engaged.enemy) else {
            continue;
        };
        let mut defender = commands.entity(engaged.enemy);
        defender.insert(Engaged {
            enemy: attacker,
            aggressor: false,
        });
        if !is_player {
            defender.remove::<Route>();
        }
        alerts.write(Alert {
            ship: engaged.enemy,
            kind: AlertKind::UnderAttack { by: attacker },
        });
    }
}

/// Ends engagements whose enemy is gone, sunk, in port, or out of reach.
/// Her quarry striking doesn't end it: there's boarding to come.
fn break_off(
    engaged: Query<(Entity, &Engaged, &Transform)>,
    enemies: Query<&Transform, (Without<Docked>, Without<Sunk>)>,
    mut commands: Commands,
) {
    for (ship, engagement, transform) in &engaged {
        let in_reach = enemies.get(engagement.enemy).is_ok_and(|enemy| {
            enemy.translation.distance(transform.translation) <= DISENGAGE_DISTANCE
        });
        if !in_reach {
            commands.entity(ship).remove::<Engaged>();
        }
    }
}

/// A struck ship nobody is engaging hoists her colours again and goes on
/// her way.
fn rehoist_colours(
    struck: Query<Entity, With<Struck>>,
    engaged: Query<&Engaged>,
    mut commands: Commands,
) {
    for ship in &struck {
        if !engaged.iter().any(|engaged| engaged.enemy == ship) {
            commands.entity(ship).remove::<Struck>();
        }
    }
}

fn strength(
    battery: &Battery,
    crew: &Crew,
    complement: &Complement,
    timbers: f32,
    rigging: f32,
) -> Strength {
    Strength {
        guns_per_side: battery.per_side(),
        crew: crew.count,
        min_crew: complement.min,
        timbers,
        rigging,
    }
}

/// Steers every engaged ship that has no route of her own by her tactic.
fn steer(
    mut ships: Query<
        (
            &Engaged,
            &Transform,
            &Heading,
            &Battery,
            &Crew,
            &Complement,
            &Timbers,
            &Rigging,
            &mut Helm,
        ),
        (Without<Route>, Without<Struck>),
    >,
    enemies: Query<(
        &Transform,
        &Heading,
        &Battery,
        &Crew,
        &Complement,
        &Timbers,
        &Rigging,
        Has<Struck>,
    )>,
    wind: Res<Wind>,
    land: Res<Land>,
) {
    for (engaged, transform, heading, battery, crew, complement, timbers, rigging, mut helm) in
        &mut ships
    {
        let Ok((
            enemy_at,
            enemy_heading,
            enemy_battery,
            enemy_crew,
            enemy_complement,
            enemy_timbers,
            enemy_rigging,
            enemy_struck,
        )) = enemies.get(engaged.enemy)
        else {
            continue;
        };
        let tactic = if enemy_struck {
            Tactic::Close
        } else if engaged.aggressor {
            Tactic::Pursue
        } else {
            defend(
                &strength(battery, crew, complement, timbers.0, rigging.0),
                &strength(
                    enemy_battery,
                    enemy_crew,
                    enemy_complement,
                    enemy_timbers.0,
                    enemy_rigging.0,
                ),
            )
        };
        let wanted = course(
            tactic,
            transform.translation,
            heading.0,
            enemy_at.translation,
            enemy_heading.0,
            wind.from,
        );
        let distance = transform.translation.distance(enemy_at.translation);
        // Closing to board, look only as far as she lies.
        let ahead = if tactic == Tactic::Close {
            distance.min(LOOKOUT_AHEAD)
        } else {
            LOOKOUT_AHEAD
        };
        helm.course = sea_room(&land, transform.translation, wanted, ahead);
        let alongside = distance <= LIE_ALONGSIDE;
        helm.sails = if tactic == Tactic::Close && alongside {
            Sails::Furled
        } else {
            Sails::Set
        };
    }
}

/// Reloads every battery, and fires each loaded broadside that bears. Keen
/// crews and a gunner reload faster.
fn fire(
    time: Res<Time>,
    mut ships: Query<
        (
            Entity,
            &Engaged,
            &Transform,
            &Heading,
            &mut Battery,
            &mut FireControl,
        ),
        Without<Struck>,
    >,
    targets: Query<&Transform, Without<Struck>>,
    mut hulls: Query<(&mut Timbers, &mut Rigging, &mut Crew, Option<&Afterguard>)>,
    mut volleys: MessageWriter<Volley>,
) {
    let dt = time.delta_secs();
    let mut shots: Vec<(Entity, Damage)> = Vec::new();
    for (ship, engaged, transform, heading, mut battery, mut control) in &mut ships {
        for side in [Broadside::Port, Broadside::Starboard] {
            let reload = battery.reload_mut(side);
            *reload = (*reload - dt).max(0.0);
        }
        // A manual order is for this moment only: guns that don't bear
        // or aren't loaded don't fire, and the order lapses.
        let order = match control.as_mut() {
            FireControl::Auto => None,
            FireControl::Manual { order } => Some(order.take()),
        };
        let Ok(target) = targets.get(engaged.enemy) else {
            continue;
        };
        let Some(side) = bearing_broadside(heading.0, transform.translation, target.translation)
        else {
            continue;
        };
        let ordered = order.is_none_or(|order| order == Some(side));
        let distance = transform.translation.distance(target.translation);
        if !ordered || distance > GUN_RANGE || battery.reload(side) > 0.0 || battery.guns == 0 {
            continue;
        }
        let reload = hulls.get(ship).map_or(RELOAD, |(_, _, crew, afterguard)| {
            RELOAD * afterguard.map_or(1.0, Afterguard::reload) / crew.spirits().work_rate()
        });
        *battery.reload_mut(side) = reload;
        volleys.write(Volley {
            from: transform.translation,
            to: target.translation,
        });
        shots.push((engaged.enemy, damage(hits(battery.per_side(), distance))));
    }
    for (target, hit) in shots {
        if let Ok((mut timbers, mut rigging, mut crew, afterguard)) = hulls.get_mut(target) {
            suffer(hit, afterguard, &mut timbers, &mut rigging, &mut crew);
        }
    }
}

/// A ship takes `hit`. A surgeon aboard saves some of the wounded.
fn suffer(
    hit: Damage,
    afterguard: Option<&Afterguard>,
    timbers: &mut Timbers,
    rigging: &mut Rigging,
    crew: &mut Crew,
) {
    timbers.0 = (timbers.0 - hit.timbers).max(0.0);
    rigging.0 = (rigging.0 - hit.rigging).max(0.0);
    crew.take_casualties(hit.men * afterguard.map_or(1.0, Afterguard::losses));
}

/// A ship as a round sees her: her strength, and her best speed as she is.
fn fighter(
    (hull, handling, laden): (&Hull, &SailHandling, &Laden),
    (battery, complement): (&Battery, &Complement),
    (timbers, rigging, crew): (&Timbers, &Rigging, &Crew),
) -> Fighter {
    Fighter {
        strength: strength(battery, crew, complement, timbers.0, rigging.0),
        speed: sail_speed(hull.speed_mps, handling.0, rigging.0, laden.0),
    }
}

/// Plays each ordered round. The enemy answers with her tactic; a struck
/// enemy just waits to be boarded. Coming alongside lays the ships
/// alongside in the world too, so boarding works as at sea. Drawing out of
/// reach ends the battle.
fn play_rounds(
    mut orders: MessageReader<RoundOrder>,
    mut dice: ResMut<Dice>,
    engaged: Query<&Engaged>,
    struck: Query<(), With<Struck>>,
    mut rounds: Query<&mut Rounds>,
    mut ships: Query<(
        &mut Transform,
        &mut Heading,
        (&Hull, &SailHandling, &Laden),
        (&Battery, &Complement),
        (&mut Timbers, &mut Rigging, &mut Crew),
        Option<&Afterguard>,
    )>,
    mut commands: Commands,
) {
    for order in orders.read() {
        let Ok(&Engaged { enemy, .. }) = engaged.get(order.ship) else {
            continue;
        };
        let Ok(mut state) = rounds.get_mut(order.ship) else {
            continue;
        };
        let Ok([us, them]) = ships.get_many_mut([order.ship, enemy]) else {
            continue;
        };
        let (mut our_at, mut our_heading, our_sailing, our_guns, mut our_hull, our_officers) = us;
        let (their_at, their_heading, their_sailing, their_guns, mut their_hull, their_officers) =
            them;
        let round = if struck.contains(enemy) {
            // She's given up: closing lays us alongside, and no one fires.
            Round {
                range: Some(if order.play == Move::Close {
                    Range::Alongside
                } else {
                    state.range
                }),
                taken: Damage::default(),
                dealt: Damage::default(),
            }
        } else {
            let ours = fighter(
                our_sailing,
                our_guns,
                (&our_hull.0, &our_hull.1, &our_hull.2),
            );
            let theirs = fighter(
                their_sailing,
                their_guns,
                (&their_hull.0, &their_hull.1, &their_hull.2),
            );
            let their_tactic = if engaged.get(enemy).is_ok_and(|e| e.aggressor) {
                Tactic::Pursue
            } else {
                defend(&theirs.strength, &ours.strength)
            };
            rounds::play(
                state.range,
                order.play,
                their_tactic,
                &ours,
                &theirs,
                &mut dice,
            )
        };
        let (timbers, rigging, crew) = &mut our_hull;
        suffer(round.taken, our_officers, timbers, rigging, crew);
        let (timbers, rigging, crew) = &mut their_hull;
        suffer(round.dealt, their_officers, timbers, rigging, crew);
        state.last = Some((order.play, round));
        match round.range {
            None => {
                commands.entity(order.ship).remove::<(Engaged, Rounds)>();
                commands.entity(enemy).remove::<Engaged>();
            }
            Some(range) => {
                state.range = range;
                if range == Range::Alongside {
                    // Lie alongside her, on her beam, on the same heading.
                    let beam = heading_to_direction(their_heading.0 + FRAC_PI_2);
                    our_at.translation = their_at.translation + beam * BOARDING_RANGE / 2.0;
                    our_heading.0 = their_heading.0;
                }
            }
        }
    }
}

/// Holed ships sink and beaten AI captains strike. The player never
/// strikes on her own; she fights until she sinks or breaks off.
fn strike_or_sink(
    ships: Query<(
        Entity,
        &Engaged,
        &Battery,
        &Crew,
        &Complement,
        &Timbers,
        &Rigging,
        Has<Player>,
        Option<&Colours>,
    )>,
    mut helms: Query<&mut Helm>,
    mut alerts: MessageWriter<Alert>,
    mut deeds: MessageWriter<Deed>,
    mut commands: Commands,
) {
    for (ship, engaged, battery, crew, complement, timbers, rigging, is_player, colours) in &ships {
        let foe = engaged.enemy;
        if timbers.0 <= 0.0 {
            if let Some(&Colours(nation)) = colours {
                deeds.write(Deed {
                    doer: foe,
                    kind: DeedKind::Sank,
                    nation,
                });
            }
            alerts.write(Alert {
                ship: foe,
                kind: AlertKind::EnemySank { other: ship },
            });
            if is_player {
                alerts.write(Alert {
                    ship,
                    kind: AlertKind::Sunk,
                });
                commands
                    .entity(ship)
                    .remove::<(Engaged, Route)>()
                    .insert(Sunk);
                if let Ok(mut helm) = helms.get_mut(ship) {
                    helm.sails = Sails::Furled;
                }
            } else {
                commands.entity(ship).despawn();
            }
        } else if !is_player && strength(battery, crew, complement, timbers.0, rigging.0).beaten() {
            alerts.write(Alert {
                ship: foe,
                kind: AlertKind::EnemyStruck { other: ship },
            });
            commands
                .entity(ship)
                .remove::<(Engaged, Route)>()
                .insert(Struck);
            if let Ok(mut helm) = helms.get_mut(ship) {
                helm.sails = Sails::Furled;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Hold;
    use crate::goods::Goods;
    use crate::math::heading_to_direction;
    use crate::ship_class::ShipClass;
    use crate::test_support::{run_checked, run_seconds, run_until, sim_app};
    use crate::time_scale::MAX_STEP;
    use rounds::{Move, Range};

    fn battle_app() -> App {
        let mut app = sim_app();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        app
    }

    /// A sloop at `at` heading `heading` with `crew` men and `guns`, her
    /// hold `laden` full of rum.
    fn sloop(app: &mut App, at: Vec3, heading: f32, crew: u32, guns: u32, laden: f64) -> Entity {
        let goods = app.world().resource::<Goods>().clone();
        let class = ShipClass::SLOOP;
        let mut hold: Hold = class.empty_hold();
        hold.owned
            .add(goods.id("rum").unwrap(), class.hold_tons * laden);
        app.world_mut()
            .spawn((
                class.build(crew, guns, hold),
                Heading(heading),
                Helm {
                    course: heading,
                    sails: Sails::Set,
                },
                Transform::from_translation(at),
            ))
            .id()
    }

    fn engage_order(app: &mut App, ship: Entity, target: Entity) {
        app.world_mut().write_message(EngageOrder { ship, target });
    }

    /// The wind's from the east-north-east, so a merchant to the west of a
    /// raider runs downwind, away from her.
    fn raider_and_merchant(app: &mut App) -> (Entity, Entity) {
        let raider = sloop(app, Vec3::ZERO, 0.0, 40, 8, 0.0);
        let west = heading_to_direction(270_f32.to_radians()) * 900.0;
        let merchant = sloop(app, west, 0.0, 10, 4, 1.0);
        (raider, merchant)
    }

    #[test]
    fn a_raider_runs_down_a_laden_merchant_until_she_strikes() {
        let mut app = battle_app();
        let (raider, merchant) = raider_and_merchant(&mut app);
        engage_order(&mut app, raider, merchant);
        let struck = run_until(&mut app, 3.0 * 3600.0, |world| {
            world.get::<Struck>(merchant).is_some()
        });
        assert!(struck, "she never struck");
        let world = app.world();
        assert_eq!(
            world.get::<Engaged>(raider).map(|e| e.enemy),
            Some(merchant),
            "still engaged, to board her"
        );
        assert_eq!(world.get::<Helm>(merchant).unwrap().sails, Sails::Furled);
        assert!(
            world.get::<Timbers>(raider).unwrap().0 > 0.5,
            "the raider came off well"
        );
    }

    #[test]
    fn the_attacked_ship_fights_back() {
        let mut app = battle_app();
        // Evenly matched, so the defender stands and fights.
        let attacker = sloop(&mut app, Vec3::ZERO, 0.0, 12, 8, 0.0);
        let defender = sloop(&mut app, Vec3::new(300.0, 0.0, 0.0), 0.0, 12, 8, 0.0);
        engage_order(&mut app, attacker, defender);
        run_seconds(&mut app, 600.0);
        let world = app.world();
        assert_eq!(
            world.get::<Engaged>(defender),
            Some(&Engaged {
                enemy: attacker,
                aggressor: false
            })
        );
        assert!(
            world.get::<Timbers>(attacker).unwrap().0 < 1.0,
            "never fired back"
        );
    }

    #[test]
    fn manual_guns_hold_their_fire_until_ordered() {
        let mut app = battle_app();
        let attacker = sloop(&mut app, Vec3::ZERO, 0.0, 40, 8, 0.0);
        // Dead in the water, abeam to starboard, and toothless.
        let target = sloop(&mut app, Vec3::new(200.0, 0.0, 0.0), 0.0, 10, 0, 0.0);
        app.world_mut().get_mut::<Helm>(target).unwrap().sails = Sails::Furled;
        app.world_mut()
            .entity_mut(attacker)
            .insert(FireControl::Manual { order: None });
        engage_order(&mut app, attacker, target);
        run_seconds(&mut app, 120.0);
        assert_eq!(app.world().get::<Timbers>(target), Some(&Timbers(1.0)));

        let bearing = |world: &World| {
            let heading = world.get::<Heading>(attacker).unwrap().0;
            let from = world.get::<Transform>(attacker).unwrap().translation;
            let to = world.get::<Transform>(target).unwrap().translation;
            (from.distance(to) < GUN_RANGE)
                .then(|| bearing_broadside(heading, from, to))
                .flatten()
        };
        assert!(run_until(&mut app, 600.0, |world| bearing(world).is_some()));
        let side = bearing(app.world()).unwrap();
        app.world_mut().write_message(FireOrder {
            ship: attacker,
            side,
        });
        // Queued this frame, fired on the next fixed step.
        app.update();
        app.update();
        assert!(
            app.world().get::<Timbers>(target).unwrap().0 < 1.0,
            "the order went unheeded"
        );
    }

    #[test]
    fn guns_hold_fire_out_of_range() {
        let mut app = battle_app();
        let attacker = sloop(&mut app, Vec3::ZERO, 0.0, 40, 8, 0.0);
        // Abeam to starboard, beyond gun range, and dead in the water.
        let target = sloop(
            &mut app,
            Vec3::new(GUN_RANGE * 2.0, 0.0, 0.0),
            0.0,
            10,
            0,
            0.0,
        );
        for ship in [attacker, target] {
            app.world_mut().get_mut::<Helm>(ship).unwrap().sails = Sails::Furled;
        }
        app.world_mut().entity_mut(attacker).insert(Engaged {
            enemy: target,
            aggressor: false,
        });
        run_seconds(&mut app, 10.0);
        let battery = app.world().get::<Battery>(attacker).unwrap();
        assert_eq!(
            (battery.port, battery.starboard),
            (0.0, 0.0),
            "wasted a broadside"
        );
    }

    #[test]
    fn ships_that_draw_apart_break_off() {
        let mut app = battle_app();
        let (raider, merchant) = raider_and_merchant(&mut app);
        engage_order(&mut app, raider, merchant);
        app.update();
        app.world_mut()
            .get_mut::<Transform>(merchant)
            .unwrap()
            .translation = Vec3::new(-DISENGAGE_DISTANCE * 2.0, 0.0, 0.0);
        run_seconds(&mut app, 10.0);
        assert!(app.world().get::<Engaged>(raider).is_none());
        assert!(app.world().get::<Engaged>(merchant).is_none());
    }

    #[test]
    fn a_struck_ship_left_alone_hoists_her_colours_again() {
        let mut app = battle_app();
        let (_, merchant) = raider_and_merchant(&mut app);
        app.world_mut().entity_mut(merchant).insert(Struck);
        run_seconds(&mut app, 10.0);
        assert!(app.world().get::<Struck>(merchant).is_none());
    }

    #[test]
    fn sea_room_turns_away_from_land_ahead_and_keeps_a_clear_course() {
        use crate::map::LandMass;
        use crate::map::data::Terrain;
        // A wall of land 300 m north, running east and west.
        let land = Land {
            masses: vec![LandMass::new(
                "Wall",
                Terrain::Sand,
                vec![
                    Vec2::new(-5000.0, -300.0),
                    Vec2::new(5000.0, -300.0),
                    Vec2::new(5000.0, -1000.0),
                    Vec2::new(-5000.0, -1000.0),
                ],
                None,
            )],
        };
        let east = 90_f32.to_radians();
        assert_eq!(
            sea_room(&land, Vec3::ZERO, east, LOOKOUT_AHEAD),
            east,
            "already clear"
        );
        let north = sea_room(&land, Vec3::ZERO, 0.0, LOOKOUT_AHEAD);
        let ahead = heading_to_direction(north) * LOOKOUT_AHEAD;
        assert!(
            land.clearance(Vec3::ZERO, ahead) >= SEA_ROOM,
            "turned to {}°",
            north.to_degrees()
        );
        assert!(north.to_degrees() > 0.0, "turned away from dead ahead");
    }

    #[test]
    fn a_fleeing_ship_does_not_run_onto_the_coast() {
        use crate::map::LandMass;
        use crate::map::data::Terrain;
        let mut app = battle_app();
        // The merchant runs west, away from the raider, onto a coast 800 m off.
        app.insert_resource(Land {
            masses: vec![LandMass::new(
                "Coast",
                Terrain::Sand,
                vec![
                    Vec2::new(-1700.0, -5000.0),
                    Vec2::new(-1700.0, 5000.0),
                    Vec2::new(-3000.0, 5000.0),
                    Vec2::new(-3000.0, -5000.0),
                ],
                None,
            )],
        });
        let (raider, merchant) = raider_and_merchant(&mut app);
        engage_order(&mut app, raider, merchant);
        run_checked(&mut app, 1800.0, |world| {
            if let Ok(at) = world
                .get_entity(merchant)
                .map(|e| e.get::<Transform>().unwrap().translation)
            {
                assert!(
                    !world.get::<crate::sailing::Aground>(merchant).unwrap().0,
                    "ran aground at {at}"
                );
            }
        });
    }

    #[test]
    fn a_holed_ship_sinks() {
        let mut app = battle_app();
        let (raider, merchant) = raider_and_merchant(&mut app);
        engage_order(&mut app, raider, merchant);
        app.update();
        app.world_mut().get_mut::<Timbers>(merchant).unwrap().0 = 0.0;
        run_seconds(&mut app, 10.0);
        assert!(app.world().get_entity(merchant).is_err(), "still afloat");
        assert!(app.world().get::<Engaged>(raider).is_none());
    }

    /// A raider fighting a merchant by rounds with the world paused, as the
    /// battle screen does.
    fn by_rounds() -> (App, Entity, Entity) {
        let mut app = battle_app();
        let (raider, merchant) = raider_and_merchant(&mut app);
        app.world_mut().entity_mut(raider).insert((
            Engaged {
                enemy: merchant,
                aggressor: true,
            },
            Rounds::starting_at(900.0),
        ));
        app.world_mut().entity_mut(merchant).insert(Engaged {
            enemy: raider,
            aggressor: false,
        });
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        (app, raider, merchant)
    }

    fn play(app: &mut App, ship: Entity, play: Move) {
        app.world_mut().write_message(RoundOrder { ship, play });
        app.update();
    }

    #[test]
    fn rounds_are_fought_while_the_world_waits_until_she_strikes() {
        let (mut app, raider, merchant) = by_rounds();
        // Within gunshot of a slow merchant, so this is about the rounds and
        // not the race: a runner can win that, and get away.
        app.world_mut().get_mut::<Rigging>(merchant).unwrap().0 = 0.3;
        app.world_mut().get_mut::<Rounds>(raider).unwrap().range = Range::Gunshot;
        let clock = app.world().resource::<crate::clock::WorldClock>().elapsed;
        for _ in 0..200 {
            if app.world().get::<Struck>(merchant).is_some() {
                break;
            }
            let range = app.world().get::<Rounds>(raider).unwrap().range;
            let next = if range == Range::Gunshot {
                Move::Rake
            } else {
                Move::Close
            };
            play(&mut app, raider, next);
        }
        let world = app.world();
        assert!(world.get::<Struck>(merchant).is_some(), "she never struck");
        assert!(
            world.get::<Timbers>(merchant).unwrap().0 < 1.0
                || world.get::<Rigging>(merchant).unwrap().0 < 1.0
        );
        assert_eq!(
            world.resource::<crate::clock::WorldClock>().elapsed,
            clock,
            "the world waited"
        );
    }

    #[test]
    fn closing_on_a_struck_ship_lays_her_alongside_to_board() {
        let (mut app, raider, merchant) = by_rounds();
        app.world_mut().entity_mut(merchant).insert(Struck);
        play(&mut app, raider, Move::Close);
        let world = app.world();
        assert_eq!(world.get::<Rounds>(raider).unwrap().range, Range::Alongside);
        let gap = world
            .get::<Transform>(raider)
            .unwrap()
            .translation
            .distance(world.get::<Transform>(merchant).unwrap().translation);
        assert!(gap <= BOARDING_RANGE, "{gap} m apart");
    }

    #[test]
    fn breaking_off_from_long_range_ends_the_battle() {
        let (mut app, raider, merchant) = by_rounds();
        // The merchant runs too, so nothing stops us drawing apart.
        for _ in 0..3 {
            play(&mut app, raider, Move::BreakOff);
        }
        assert!(app.world().get::<Engaged>(raider).is_none());
        assert!(app.world().get::<Rounds>(raider).is_none());
        assert!(app.world().get::<Engaged>(merchant).is_none());
    }
}
