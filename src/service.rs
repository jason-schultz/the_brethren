//! Service: the groups a captain serves, her rank in each, and what
//! serving brings her. Kept on the ship she commands, beside her
//! [`Standing`]; for now only the player serves anyone.
//!
//! A captain in good standing with a group can join it at its office in
//! one of its nation's ports. She serves one nation at a time, and rises in
//! rank as her standing with the group grows. The navy pays her by the
//! day, the governor pays bounties on enemy ships under a letter of marque,
//! and the merchants pay her more for their contracts. Turning on her own
//! nation, or a crown's navy officer flying other colours, gets her struck
//! off.

use std::fmt;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::alert::{Alert, AlertKind};
use crate::cargo::Purse;
use crate::clock::WorldClock;
use crate::factions::{
    CaptainId, Colours, DeedKind, Group, GroupKind, Nation, Relations, Standing,
};
use crate::port::{Docked, Port, office};
use crate::schedule::{FrameSet, SimSet};
use crate::word::{DeedKnown, Happening, Ledger};

/// Standing with a group that earns each rank. Joining takes the first.
pub const RANKS: [f32; 3] = [20.0, 50.0, 80.0];
/// What a navy pays a day at each rank, pieces of eight.
const PAY_PER_DAY: [u32; RANKS.len()] = [5, 10, 20];
/// What a governor pays at each rank for every enemy ship taken or sunk
/// under a letter of marque.
const BOUNTY: [u32; RANKS.len()] = [100, 200, 400];
/// How much more the merchants pay one of their own for a contract, at
/// each rank.
const CONTRACT_BONUS: [f64; RANKS.len()] = [0.1, 0.2, 0.4];

/// A group the player serves, and her rank in it, from 0.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Post {
    pub group: Group,
    pub rank: usize,
}

impl Post {
    /// Her rank's title, as the factions data has it: "Lieutenant".
    pub fn title(self) -> &'static str {
        self.group.rank_title(self.rank)
    }

    /// What serving brings her, in words.
    pub fn perk(self) -> String {
        match self.group.kind {
            GroupKind::Navy => format!("pay of {} pieces of eight a day", PAY_PER_DAY[self.rank]),
            GroupKind::Governor => format!(
                "a letter of marque: {} pieces of eight for every ship of {}'s enemies taken or sunk under {} colours",
                BOUNTY[self.rank],
                self.group.nation,
                self.group.nation.adjective()
            ),
            GroupKind::Merchants => format!(
                "{:.0}% more for their contracts",
                CONTRACT_BONUS[self.rank] * 100.0
            ),
        }
    }
}

/// The groups a captain serves, all of one nation.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Service(pub Vec<Post>);

impl Service {
    /// The nation she serves, if any.
    pub fn nation(&self) -> Option<Nation> {
        self.0.first().map(|post| post.group.nation)
    }

    pub fn post(&self, group: Group) -> Option<Post> {
        self.0.iter().copied().find(|post| post.group == group)
    }

    /// Her post with `nation`'s group of `kind`.
    fn post_with(&self, nation: Nation, kind: GroupKind) -> Option<Post> {
        self.post(Group { nation, kind })
    }

    /// Strikes her off every group of `nation`'s she serves, if any, and
    /// says whether she was.
    fn strike_off(&mut self, nation: Nation) -> bool {
        let before = self.0.len();
        self.0.retain(|post| post.group.nation != nation);
        self.0.len() != before
    }
}

/// The rank `standing` with a group earns, if any.
pub fn earned_rank(standing: f32) -> Option<usize> {
    RANKS.iter().rposition(|&needed| standing >= needed)
}

/// Whether her post with `group` has her fly its nation's colours: a
/// crown's navy officers do, and the Brethren fly what they like.
fn keeps_colours(group: Group) -> bool {
    group.kind == GroupKind::Navy && group.nation.is_crown()
}

/// What the merchants of `group` add to one of their contracts' pay for
/// her, as a fraction.
pub fn contract_bonus(service: &Service, group: Group) -> f64 {
    service
        .post(group)
        .filter(|post| post.group.kind == GroupKind::Merchants)
        .map_or(0.0, |post| CONTRACT_BONUS[post.rank])
}

/// Why she can't join a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotJoined {
    /// She already serves it.
    Serving,
    /// She serves another nation.
    Serves(Nation),
    /// The group keeps no office in this port.
    NotHere,
    Wanted,
    /// She hasn't the standing with it.
    Unproven,
    /// A crown's navy takes only captains under its colours.
    Colours,
}

impl fmt::Display for NotJoined {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Serving => f.write_str("you serve them already"),
            Self::Serves(nation) => write!(f, "you serve {nation}; resign first"),
            Self::NotHere => f.write_str("they keep no office here"),
            Self::Wanted => f.write_str("you're wanted"),
            Self::Unproven => write!(f, "they take captains at {:+.0}", RANKS[0]),
            Self::Colours => f.write_str("hoist their colours first"),
        }
    }
}

/// Whether a captain, flying `colours`, can join `group` in `port`. Both
/// the port screen and the joining itself use this.
pub fn check_join(
    group: Group,
    port: &Port,
    standing: &Standing,
    service: &Service,
    colours: Option<Nation>,
) -> Result<(), NotJoined> {
    if service.post(group).is_some() {
        Err(NotJoined::Serving)
    } else if let Some(nation) = service.nation().filter(|&nation| nation != group.nation) {
        Err(NotJoined::Serves(nation))
    } else if port.flag != group.nation || office(&port.locations, group.kind).is_none() {
        Err(NotJoined::NotHere)
    } else if standing.wanted_by(group.nation) {
        Err(NotJoined::Wanted)
    } else if earned_rank(standing.of(group)).is_none() {
        Err(NotJoined::Unproven)
    } else if keeps_colours(group) && colours != Some(group.nation) {
        Err(NotJoined::Colours)
    } else {
        Ok(())
    }
}

/// Why she was struck off a nation's groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StruckOff {
    /// She struck a blow against her own nation.
    TurnedOnThem,
    /// Her nation wants her.
    Wanted,
    /// A navy officer hoisted other colours.
    Deserted,
}

/// Order a captain to join a group at its office where she lies.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Join {
    pub captain: Entity,
    pub group: Group,
}

/// Order a captain to leave a group.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resign {
    pub captain: Entity,
    pub group: Group,
}

pub struct ServicePlugin;

impl Plugin for ServicePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Join>()
            .add_message::<Resign>()
            .add_systems(
                Update,
                (join_and_resign.in_set(FrameSet::Orders), answer_for_deeds),
            )
            .add_systems(FixedUpdate, serve.in_set(SimSet::Resolve));
    }
}

fn join_and_resign(
    mut joins: MessageReader<Join>,
    mut resignations: MessageReader<Resign>,
    mut captains: Query<(Option<&Docked>, Option<&Colours>, &Standing, &mut Service)>,
    ports: Query<&Port>,
) {
    for &Join { captain, group } in joins.read() {
        let Ok((Some(docked), colours, standing, mut service)) = captains.get_mut(captain) else {
            continue;
        };
        let Ok(port) = ports.get(docked.port) else {
            continue;
        };
        let colours = colours.map(|colours| colours.0);
        if check_join(group, port, standing, &service, colours).is_ok() {
            let rank = earned_rank(standing.of(group)).unwrap_or(0);
            service.0.push(Post { group, rank });
        }
    }
    for &Resign { captain, group } in resignations.read() {
        if let Ok((.., mut service)) = captains.get_mut(captain) {
            service.0.retain(|post| post.group != group);
        }
    }
}

/// As a nation hears of a captain's deed: a group of it whose errand she
/// ran asks her to join; a member who struck at her own nation is struck
/// off; and a privateer is paid for an enemy ship she took or sank under
/// her nation's colours.
fn answer_for_deeds(
    mut known: MessageReader<DeedKnown>,
    ledger: Res<Ledger>,
    mut captains: Query<(Entity, &CaptainId, &mut Purse, &mut Service)>,
    relations: Res<Relations>,
    mut alerts: MessageWriter<Alert>,
) {
    for &DeedKnown {
        news,
        nation: heard,
        answered,
    } in known.read()
    {
        let Some(Happening::Deed(deed)) = ledger.get(news).map(|news| &news.event) else {
            continue;
        };
        let Some((ship, _, mut purse, mut service)) = captains
            .iter_mut()
            .find(|(_, id, ..)| Some(**id) == deed.captain)
        else {
            continue;
        };
        if deed.kind == DeedKind::Introduced && heard == deed.group.nation {
            alerts.write(Alert {
                ship,
                kind: AlertKind::Introduced { group: deed.group },
            });
        }
        let Some(nation) = service.nation().filter(|&nation| nation == heard) else {
            continue;
        };
        let victim = deed.group.nation;
        if victim == nation && deed.kind.is_blow() {
            service.strike_off(nation);
            alerts.write(Alert {
                ship,
                kind: AlertKind::StruckOff {
                    nation,
                    why: StruckOff::TurnedOnThem,
                },
            });
        } else if matches!(deed.kind, DeedKind::Captured | DeedKind::Sank)
            // A ship taken and then sunk is paid for once.
            && answered != Some(DeedKind::Captured)
            && relations.at_war(nation, victim)
            && deed.colours == Some(nation)
            && let Some(post) = service.post_with(nation, GroupKind::Governor)
        {
            let amount = BOUNTY[post.rank];
            purse.0 += amount;
            alerts.write(Alert {
                ship,
                kind: AlertKind::Bounty {
                    group: post.group,
                    amount,
                },
            });
        }
    }
}

/// Every step, for every captain in service: a member her nation wants,
/// or a navy officer under other colours, is struck off; standing earns
/// promotion; and each new day the navy pays.
fn serve(
    time: Res<Time>,
    clock: Res<WorldClock>,
    mut captains: Query<(
        Entity,
        Option<&Colours>,
        &mut Purse,
        &Standing,
        &mut Service,
    )>,
    mut alerts: MessageWriter<Alert>,
) {
    for (ship, colours, mut purse, standing, mut service) in &mut captains {
        let Some(nation) = service.nation() else {
            continue;
        };
        let mut alert = |kind| {
            alerts.write(Alert { ship, kind });
        };
        if standing.wanted_by(nation) {
            service.strike_off(nation);
            alert(AlertKind::StruckOff {
                nation,
                why: StruckOff::Wanted,
            });
            continue;
        }
        let deserted = |post: &Post| keeps_colours(post.group) && colours != Some(&Colours(nation));
        if service.0.iter().any(deserted) {
            service.0.retain(|post| !deserted(post));
            alert(AlertKind::StruckOff {
                nation,
                why: StruckOff::Deserted,
            });
        }
        for post in &mut service.0 {
            let earned = earned_rank(standing.of(post.group)).unwrap_or(0);
            if earned > post.rank {
                post.rank = earned;
                alert(AlertKind::Promoted { post: *post });
            }
        }
        if let Some(post) = service.post_with(nation, GroupKind::Navy) {
            purse.0 += PAY_PER_DAY[post.rank] * clock.days_begun(time.delta_secs_f64());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::DAY;
    use crate::factions::Deed;
    use crate::port::Location;
    use crate::test_support::{run_seconds, sim_app};
    use crate::time_scale::MAX_STEP;
    use crate::word::Knowledge;

    const ROYAL_NAVY: Group = Group {
        nation: Nation::ENGLAND,
        kind: GroupKind::Navy,
    };
    const JAMAICA_MERCHANTS: Group = Group {
        nation: Nation::ENGLAND,
        kind: GroupKind::Merchants,
    };

    fn port_royal() -> Port {
        Port {
            flag: Nation::ENGLAND,
            town: Vec3::ZERO,
            locations: vec![
                Location::Tavern,
                Location::GovernorsHouse,
                Location::MerchantsExchange,
            ],
        }
    }

    #[test]
    fn rank_comes_with_standing() {
        assert_eq!(earned_rank(10.0), None);
        assert_eq!(earned_rank(RANKS[0]), Some(0));
        assert_eq!(earned_rank(65.0), Some(1));
        assert_eq!(earned_rank(100.0), Some(2));
    }

    #[test]
    fn a_navy_takes_proven_captains_under_its_colours() {
        let port = port_royal();
        let mut standing = Standing::default();
        let service = Service::default();
        let england = Some(Nation::ENGLAND);
        assert_eq!(
            check_join(ROYAL_NAVY, &port, &standing, &service, england),
            Err(NotJoined::Unproven)
        );
        standing.set_group(ROYAL_NAVY, 25.0);
        assert_eq!(
            check_join(
                ROYAL_NAVY,
                &port,
                &standing,
                &service,
                Some(Nation::PIRATES)
            ),
            Err(NotJoined::Colours)
        );
        assert_eq!(
            check_join(ROYAL_NAVY, &port, &standing, &service, england),
            Ok(())
        );
        let spanish = Port {
            flag: Nation::SPAIN,
            ..port_royal()
        };
        assert_eq!(
            check_join(ROYAL_NAVY, &spanish, &standing, &service, england),
            Err(NotJoined::NotHere)
        );
    }

    #[test]
    fn she_serves_one_nation_at_a_time() {
        let port = Port {
            flag: Nation::PIRATES,
            locations: vec![Location::Tavern],
            ..port_royal()
        };
        let mut standing = Standing::default();
        let flying_gang = Nation::PIRATES.groups()[0];
        standing.set_group(flying_gang, 30.0);
        let service = Service(vec![Post {
            group: JAMAICA_MERCHANTS,
            rank: 0,
        }]);
        assert_eq!(
            check_join(flying_gang, &port, &standing, &service, None),
            Err(NotJoined::Serves(Nation::ENGLAND))
        );
        assert_eq!(
            check_join(flying_gang, &port, &standing, &Service::default(), None),
            Ok(()),
            "the Brethren don't mind the colours, and meet in the tavern"
        );
    }

    #[test]
    fn the_merchants_pay_their_own_more() {
        let service = Service(vec![Post {
            group: JAMAICA_MERCHANTS,
            rank: 1,
        }]);
        assert_eq!(
            contract_bonus(&service, JAMAICA_MERCHANTS),
            CONTRACT_BONUS[1]
        );
        assert_eq!(contract_bonus(&service, Nation::SPAIN.groups()[2]), 0.0);
        assert_eq!(contract_bonus(&Service::default(), JAMAICA_MERCHANTS), 0.0);
    }

    /// A lieutenant in the Royal Navy, under English colours, lying in
    /// Port Royal, where whatever she does is known at once.
    fn officer(app: &mut App) -> Entity {
        let mut standing = Standing::default();
        standing.set_group(ROYAL_NAVY, RANKS[0]);
        let service = Service(vec![Post {
            group: ROYAL_NAVY,
            rank: 0,
        }]);
        let port = app
            .world_mut()
            .spawn((port_royal(), Transform::default()))
            .id();
        app.world_mut()
            .spawn((
                Name::new("Sloop"),
                Colours(Nation::ENGLAND),
                Purse(0),
                (CaptainId(7), standing, service, Knowledge::default()),
                Transform::default(),
                Docked { port },
            ))
            .id()
    }

    fn service(app: &App, captain: Entity) -> &Service {
        app.world().get::<Service>(captain).unwrap()
    }

    #[test]
    fn the_navy_pays_by_the_day_and_promotes_on_merit() {
        let mut app = sim_app();
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .set_timestep_seconds(MAX_STEP);
        let ship = officer(&mut app);
        // Three new days begin in three and a half.
        run_seconds(&mut app, 3.5 * DAY as f32);
        let pay = app.world().get::<Purse>(ship).unwrap().0;
        assert_eq!(pay, 3 * PAY_PER_DAY[0]);
        app.world_mut()
            .get_mut::<Standing>(ship)
            .unwrap()
            .set_group(ROYAL_NAVY, RANKS[1]);
        app.update();
        let rank = service(&app, ship).post(ROYAL_NAVY).map(|post| post.rank);
        assert_eq!(rank, Some(1));
    }

    #[test]
    fn a_navy_officer_under_other_colours_is_struck_off() {
        let mut app = sim_app();
        let ship = officer(&mut app);
        app.world_mut()
            .entity_mut(ship)
            .insert(Colours(Nation::PIRATES));
        run_seconds(&mut app, 1.0);
        assert_eq!(service(&app, ship).nation(), None);
    }

    #[test]
    fn striking_at_her_own_nation_gets_her_struck_off() {
        let mut app = sim_app();
        let ship = officer(&mut app);
        app.world_mut().write_message(Deed {
            doer: ship,
            kind: DeedKind::Attacked,
            group: JAMAICA_MERCHANTS,
            about: None,
        });
        run_seconds(&mut app, 1.0);
        assert_eq!(service(&app, ship).nation(), None);
    }

    #[test]
    fn a_privateer_is_paid_for_enemy_prizes_under_her_colours() {
        let mut app = sim_app();
        let ship = officer(&mut app);
        let governor = Group {
            nation: Nation::ENGLAND,
            kind: GroupKind::Governor,
        };
        app.world_mut().entity_mut(ship).insert(Service(vec![Post {
            group: governor,
            rank: 0,
        }]));
        app.world_mut()
            .get_mut::<Standing>(ship)
            .unwrap()
            .set_group(governor, RANKS[0]);
        app.world_mut()
            .resource_mut::<Relations>()
            .set(Nation::ENGLAND, Nation::SPAIN, -90.0);
        let cuban_merchants = Nation::SPAIN.groups()[2];
        app.world_mut().write_message(Deed {
            doer: ship,
            kind: DeedKind::Captured,
            group: cuban_merchants,
            about: None,
        });
        run_seconds(&mut app, 1.0);
        assert_eq!(app.world().get::<Purse>(ship), Some(&Purse(BOUNTY[0])));
    }
}
