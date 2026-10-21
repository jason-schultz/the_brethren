//! Escorts: merchants hiring an armed captain to see them safe to port.
//!
//! A merchant about to sail with a contract, from a port where the player
//! lies, asks for an escort if raiders would prey on her colours, and
//! waits a while for an answer. Once the player takes it, she sails when
//! the player does, and the player keeps [`Company`] with her. If she's
//! attacked, her escort hears of it at once. If she comes into her
//! destination with her escort in
//! sight, the escort is paid, and her merchants think the better of her.
//! If she's lost, burned by raiders or taken by anyone, her escort
//! included, the contract fails. Her merchants wait a few days for her
//! escort's account of the loss (see [`crate::word`]); given one, true or
//! not, they take her at her word. Without one, they think the worse.

use bevy::prelude::*;

use crate::alert::{Alert, AlertKind, SIGHTING_RANGE};
use crate::cargo::Purse;
use crate::clock::{DAY, HOUR, WorldClock};
use crate::combat::Engaged;
use crate::economy::Refusal;
use crate::factions::{Deed, DeedKind, Group, GroupKind, Nation, Relations, nations};
use crate::hunting::{Hunter, Mark, is_quarry};
use crate::navigation::{Company, KeepCompany};
use crate::port::Docked;
use crate::prizes::Prize;
use crate::schedule::{FrameSet, SimSet};
use crate::word::Ledger;

/// How long a merchant waits in port for a captain to take her escort.
pub const ESCORT_WAIT: f64 = 2.0 * HOUR;
/// Once it's taken, how long past her sailing time she waits for her
/// escort to put to sea.
pub const ESCORT_PATIENCE: f64 = 12.0 * HOUR;
/// The escort's fee is this share of her contract's...
const FEE_SHARE: f64 = 0.5;
/// ...but never less than this, in pieces of eight.
const LEAST_FEE: u32 = 20;
/// Her escort must be within this distance of her when she comes in,
/// meters: in sight.
pub const ESCORT_RANGE: f32 = SIGHTING_RANGE;
/// How long her merchants wait for an account of her loss, days.
pub const ACCOUNT_DAYS: f64 = 3.0;

/// A merchant's request for an escort to the port she's bound for, and
/// the captain who took it, if any.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Escort {
    /// The merchant to be guarded.
    pub ward: Entity,
    /// Her nation, whose merchants pay, kept for when she's lost.
    pub nation: Nation,
    /// Where she sails from, and where the request is made.
    pub origin: Entity,
    pub destination: Entity,
    /// Pieces of eight, paid when she comes in.
    pub fee: u32,
    pub escort: Option<Entity>,
    /// [`WorldClock`] time she was lost, if she was.
    pub lost_at: Option<f64>,
}

impl Escort {
    /// The group that pays for it: her nation's merchants.
    pub fn group(&self) -> Group {
        Group {
            nation: self.nation,
            kind: GroupKind::Merchants,
        }
    }
}

/// Order a docked ship to take on an escort at her port.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptEscort {
    pub ship: Entity,
    pub escort: Entity,
}

/// The fee for escorting a merchant whose contract pays `payment`.
pub fn escort_fee(payment: u32) -> u32 {
    ((payment as f64 * FEE_SHARE).round() as u32).max(LEAST_FEE)
}

/// Whether a merchant under `nation`'s colours wants an escort: whether
/// any lawless nation's raiders would prey on her.
pub fn wants_escort(nation: Nation, relations: &Relations) -> bool {
    let merchantman = Mark {
        colours: Some(nation),
        warship: false,
        attacking: None,
        standing: None,
        salvaging: None,
    };
    nations()
        .filter(|raiders| !raiders.is_crown())
        .any(|raiders| is_quarry(Hunter::Raider(raiders), &merchantman, relations))
}

/// Whether a merchant due to sail at `sails_at` waits on for the escort
/// she's taken on, which lies at `escort_lies` if in port: while it lies
/// where she does, `here`, until her patience runs out.
pub fn waits_for_escort(
    escort_lies: Option<Entity>,
    here: Entity,
    now: f64,
    sails_at: f64,
) -> bool {
    escort_lies == Some(here) && now < sails_at + ESCORT_PATIENCE
}

/// Whether a ship docked at `port` can take on `escort`. Both the port
/// screen and the acceptance itself use this.
pub fn check_escort(escort: &Escort, port: Entity) -> Result<(), Refusal> {
    if port != escort.origin {
        Err(Refusal::NotHere)
    } else if escort.escort.is_some() {
        Err(Refusal::Taken)
    } else {
        Ok(())
    }
}

pub struct EscortsPlugin;

impl Plugin for EscortsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AcceptEscort>()
            // Taking on an escort is an order, so it works while paused.
            .add_systems(Update, accept_escorts.in_set(FrameSet::Orders))
            .add_systems(
                FixedUpdate,
                (warn_of_attacks, settle_escorts).in_set(SimSet::Economy),
            );
    }
}

/// Taking on an escort, she keeps company with her ward.
fn accept_escorts(
    mut orders: MessageReader<AcceptEscort>,
    ships: Query<&Docked>,
    mut escorts: Query<&mut Escort>,
    mut company: MessageWriter<KeepCompany>,
) {
    for order in orders.read() {
        let (Ok(docked), Ok(mut escort)) = (ships.get(order.ship), escorts.get_mut(order.escort))
        else {
            continue;
        };
        if check_escort(&escort, docked.port).is_ok() {
            escort.escort = Some(order.ship);
            company.write(KeepCompany {
                ship: order.ship,
                with: Some(escort.ward),
            });
        }
    }
}

/// A ward attacked calls her escort to her side.
fn warn_of_attacks(
    attacked: Query<(Entity, &Engaged), Added<Engaged>>,
    escorts: Query<&Escort>,
    mut alerts: MessageWriter<Alert>,
) {
    for (ward, engaged) in &attacked {
        let guard = escorts
            .iter()
            .find(|escort| escort.ward == ward)
            .and_then(|escort| escort.escort);
        if let Some(guard) = guard.filter(|&guard| guard != engaged.enemy) {
            alerts.write(Alert {
                ship: guard,
                kind: AlertKind::WardAttacked {
                    ward,
                    by: engaged.enemy,
                },
            });
        }
    }
}

/// A request nobody took is withdrawn once she sails. A ward that comes
/// into her destination with her escort in sight pays it; one that's
/// lost fails it, and counts against her escort unless she accounts for
/// it in time. Either way, the escort's company with her is over.
fn settle_escorts(
    clock: Res<WorldClock>,
    ledger: Res<Ledger>,
    mut escorts: Query<(Entity, &mut Escort)>,
    wards: Query<(&Transform, Option<&Docked>), Without<Prize>>,
    guards: Query<(&Transform, Option<&Company>)>,
    mut purses: Query<&mut Purse>,
    mut deeds: MessageWriter<Deed>,
    mut alerts: MessageWriter<Alert>,
    mut commands: Commands,
) {
    for (entity, mut escort) in &mut escorts {
        let ward = wards.get(escort.ward).ok();
        let docked_at = ward
            .and_then(|(_, docked)| docked)
            .map(|docked| docked.port);
        let Some(guard) = escort.escort else {
            if docked_at != Some(escort.origin) {
                commands.entity(entity).despawn();
            }
            continue;
        };
        let Ok((guard_at, company)) = guards.get(guard) else {
            // Her escort lost her own ship: the contract went with it.
            commands.entity(entity).despawn();
            continue;
        };
        let parted = company == Some(&Company(escort.ward));
        let settled = |commands: &mut Commands| {
            commands.entity(entity).despawn();
            if parted {
                commands.entity(guard).remove::<Company>();
            }
        };
        let kind = match ward {
            None => {
                let Some(lost_at) = escort.lost_at else {
                    escort.lost_at = Some(clock.elapsed);
                    if parted {
                        commands.entity(guard).remove::<Company>();
                    }
                    alerts.write(Alert {
                        ship: guard,
                        kind: AlertKind::WardLost,
                    });
                    continue;
                };
                if ledger.has_account(guard, escort.ward) {
                    settled(&mut commands);
                    continue;
                }
                if clock.elapsed - lost_at < ACCOUNT_DAYS * DAY {
                    continue;
                }
                DeedKind::LostWard
            }
            Some((ward_at, _)) if docked_at == Some(escort.destination) => {
                if ward_at.translation.distance(guard_at.translation) > ESCORT_RANGE {
                    alerts.write(Alert {
                        ship: guard,
                        kind: AlertKind::WardAlone { ward: escort.ward },
                    });
                    settled(&mut commands);
                    continue;
                }
                if let Ok(mut purse) = purses.get_mut(guard) {
                    purse.0 += escort.fee;
                }
                alerts.write(Alert {
                    ship: guard,
                    kind: AlertKind::WardSafe {
                        ward: escort.ward,
                        fee: escort.fee,
                    },
                });
                DeedKind::Escorted
            }
            Some(_) => continue,
        };
        deeds.write(Deed {
            doer: guard,
            kind,
            group: escort.group(),
            about: Some(escort.ward),
        });
        settled(&mut commands);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{run_checked, sim_app};

    #[test]
    fn the_fee_is_half_her_contracts_and_never_trifling() {
        assert_eq!(escort_fee(300), 150);
        assert_eq!(escort_fee(10), LEAST_FEE);
    }

    #[test]
    fn merchants_the_raiders_prey_on_want_an_escort() {
        let mut relations = Relations::default();
        assert!(wants_escort(Nation::ENGLAND, &relations));
        assert!(wants_escort(Nation::SPAIN, &relations));
        relations.set(Nation::ENGLAND, Nation::PIRATES, 0.0);
        assert!(!wants_escort(Nation::ENGLAND, &relations));
    }

    #[test]
    fn she_waits_for_her_escort_only_while_it_lies_beside_her() {
        let [here, there] = [Entity::from_raw_u32(1), Entity::from_raw_u32(2)].map(Option::unwrap);
        assert!(waits_for_escort(Some(here), here, 10.0, 0.0));
        assert!(!waits_for_escort(Some(there), here, 10.0, 0.0), "it sailed");
        assert!(!waits_for_escort(None, here, 10.0, 0.0), "untaken");
        assert!(!waits_for_escort(Some(here), here, ESCORT_PATIENCE, 0.0));
    }

    struct Convoy {
        app: App,
        ward: Entity,
        guard: Entity,
        destination: Entity,
        escort: Entity,
    }

    fn convoy() -> Convoy {
        let mut app = sim_app();
        let [origin, destination] = [(); 2].map(|_| app.world_mut().spawn_empty().id());
        let ward = app
            .world_mut()
            .spawn((Transform::default(), Docked { port: origin }))
            .id();
        let guard = app
            .world_mut()
            .spawn((Transform::default(), Docked { port: origin }, Purse(0)))
            .id();
        let escort = app
            .world_mut()
            .spawn(Escort {
                ward,
                nation: Nation::ENGLAND,
                origin,
                destination,
                fee: 100,
                escort: None,
                lost_at: None,
            })
            .id();
        app.world_mut().write_message(AcceptEscort {
            ship: guard,
            escort,
        });
        app.update();
        Convoy {
            app,
            ward,
            guard,
            destination,
            escort,
        }
    }

    /// The alerts and deeds over a second.
    fn outcome(app: &mut App) -> (Vec<AlertKind>, Vec<DeedKind>) {
        let (mut alerts, mut deeds) = (Vec::new(), Vec::new());
        run_checked(app, 1.0, |world| {
            let messages = world.resource::<Messages<Alert>>();
            alerts.extend(messages.iter_current_update_messages().map(|a| a.kind));
            let messages = world.resource::<Messages<Deed>>();
            deeds.extend(messages.iter_current_update_messages().map(|d| d.kind));
        });
        (alerts, deeds)
    }

    #[test]
    fn seeing_her_in_pays_and_wins_her_merchants() {
        let mut c = convoy();
        assert_eq!(
            c.app.world().get::<Escort>(c.escort).unwrap().escort,
            Some(c.guard)
        );
        c.app.update();
        assert_eq!(
            c.app.world().get::<Company>(c.guard),
            Some(&Company(c.ward)),
            "she keeps company with her ward"
        );
        let port = Docked {
            port: c.destination,
        };
        c.app.world_mut().entity_mut(c.ward).insert(port);
        c.app.world_mut().entity_mut(c.guard).insert(port);
        let (alerts, deeds) = outcome(&mut c.app);
        assert_eq!(
            alerts,
            [AlertKind::WardSafe {
                ward: c.ward,
                fee: 100
            }]
        );
        assert_eq!(deeds, [DeedKind::Escorted]);
        assert_eq!(c.app.world().get::<Purse>(c.guard), Some(&Purse(100)));
        assert!(c.app.world().get_entity(c.escort).is_err(), "settled");
        assert_eq!(c.app.world().get::<Company>(c.guard), None, "parted");
    }

    #[test]
    fn coming_in_without_her_escort_pays_nothing() {
        let mut c = convoy();
        c.app.world_mut().entity_mut(c.ward).insert(Docked {
            port: c.destination,
        });
        c.app
            .world_mut()
            .entity_mut(c.guard)
            .remove::<Docked>()
            .insert(Transform::from_xyz(ESCORT_RANGE * 2.0, 0.0, 0.0));
        let (alerts, deeds) = outcome(&mut c.app);
        assert_eq!(alerts, [AlertKind::WardAlone { ward: c.ward }]);
        assert!(deeds.is_empty());
        assert_eq!(c.app.world().get::<Purse>(c.guard), Some(&Purse(0)));
    }

    fn wait_for_an_account(app: &mut App) {
        app.world_mut().resource_mut::<WorldClock>().elapsed += ACCOUNT_DAYS * DAY;
    }

    #[test]
    fn a_ward_lost_or_taken_fails_the_contract_unaccounted_for() {
        for taken in [false, true] {
            let mut c = convoy();
            if taken {
                c.app
                    .world_mut()
                    .entity_mut(c.ward)
                    .insert(Prize { captor: c.guard });
            } else {
                c.app.world_mut().despawn(c.ward);
            }
            let (alerts, deeds) = outcome(&mut c.app);
            assert_eq!(alerts, [AlertKind::WardLost], "taken: {taken}");
            assert!(deeds.is_empty(), "her merchants wait for an account");
            wait_for_an_account(&mut c.app);
            let (_, deeds) = outcome(&mut c.app);
            assert_eq!(deeds, [DeedKind::LostWard]);
            assert!(c.app.world().get_entity(c.escort).is_err(), "settled");
        }
    }

    #[test]
    fn an_account_of_her_loss_satisfies_her_merchants() {
        let mut c = convoy();
        c.app.world_mut().despawn(c.ward);
        outcome(&mut c.app);
        let event = crate::word::Happening::Account {
            teller: c.guard,
            teller_name: "Player Sloop".to_string(),
            captain: None,
            ship: c.ward,
            name: "Dove".to_string(),
            says: crate::word::Fate::Wrecked,
            blames: None,
        };
        c.app.world_mut().resource_mut::<Ledger>().record(
            0.0,
            crate::word::Place::Port(c.destination),
            event,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        wait_for_an_account(&mut c.app);
        let (_, deeds) = outcome(&mut c.app);
        assert!(deeds.is_empty(), "{deeds:?}");
        assert!(c.app.world().get_entity(c.escort).is_err(), "settled");
    }

    #[test]
    fn an_attack_on_her_ward_calls_the_escort() {
        let mut c = convoy();
        let raider = c.app.world_mut().spawn(Transform::default()).id();
        c.app.world_mut().entity_mut(c.ward).insert(Engaged {
            enemy: raider,
            aggressor: false,
        });
        let (alerts, _) = outcome(&mut c.app);
        let attacked = AlertKind::WardAttacked {
            ward: c.ward,
            by: raider,
        };
        assert_eq!(alerts, [attacked]);
        assert!(attacked.needs_decision(), "the game waits for her answer");
    }

    #[test]
    fn a_request_nobody_took_is_withdrawn_when_she_sails() {
        let mut app = sim_app();
        let origin = app.world_mut().spawn_empty().id();
        let ward = app.world_mut().spawn(Transform::default()).id();
        let escort = app
            .world_mut()
            .spawn(Escort {
                ward,
                nation: Nation::ENGLAND,
                origin,
                destination: origin,
                fee: 100,
                escort: None,
                lost_at: None,
            })
            .id();
        outcome(&mut app);
        assert!(app.world().get_entity(escort).is_err());
    }
}
