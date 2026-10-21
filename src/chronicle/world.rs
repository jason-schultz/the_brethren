//! The full chronicle's records of the world: news as it goes in the
//! ledger and as nations hear of it, the player's standing and money,
//! nations coming to new terms, ports going over, notes, and a snapshot of
//! how things stand once a game day and with every note.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::BufWriter;

use bevy::prelude::*;
use chronicle::{
    Captain, Fleet, GoodState, GroupStanding, Happening as Kind, LOGS, Pair, PortState, RUNS_KEPT,
    RUNS_MAX_BYTES, Record, Snapshot, prune, user_file,
};

use super::{Chronicle, Note, Noting, Roles, new_day, role};
use crate::cargo::{Hold, Purse};
use crate::clock::{DAY, WorldClock};
use crate::combat::Timbers;
use crate::crew::Crew;
use crate::economy::jobs::{Carrier, Job};
use crate::economy::market::{Market, unit_price};
use crate::factions::{
    CaptainId, Colours, Nation, Relations, Standing, Terms, groups, nations, pairs,
};
use crate::goods::Goods;
use crate::player::Player;
use crate::port::{Docked, Port};
use crate::sailing::Rigging;
use crate::ship_class::ShipClass;
use crate::word::{DeedKnown, Happening, Ledger, News, NewsId};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Last,
        (
            note_news,
            note_heard,
            note_terms,
            note_ports,
            note_money,
            // A note comes before the snapshot taken with it.
            (
                note_notes,
                // Both are asked every frame, to keep count of days and notes.
                snapshot.run_if(new_day.or_eager(on_message::<Note>)),
            )
                .chain(),
        )
            .in_set(Noting),
    );
}

/// Opens the log called `name` in the logs folder, and prunes the oldest.
pub(super) fn open_log(name: &str) -> Option<BufWriter<File>> {
    let folder = user_file(LOGS);
    let opened = fs::create_dir_all(&folder).and_then(|()| File::create(folder.join(name)));
    match opened {
        Ok(file) => {
            if let Err(error) = prune(&folder, RUNS_KEPT, RUNS_MAX_BYTES) {
                warn!("couldn't prune the old logs: {error}");
            }
            Some(BufWriter::new(file))
        }
        Err(error) => {
            warn!("couldn't open the log: {error}");
            None
        }
    }
}

/// Notes each piece of news as it goes in the ledger.
fn note_news(
    ledger: Res<Ledger>,
    player: Query<Entity, With<Player>>,
    mut next: Local<NewsId>,
    mut chronicle: ResMut<Chronicle>,
) {
    // A game loaded from an earlier save numbers its news from there on,
    // and what it brought with it was noted before.
    if ledger.next < *next {
        *next = ledger.next;
    }
    let player = player.single().ok();
    for news in ledger.news.iter().filter(|news| news.id >= *next) {
        chronicle.note(Record::News(news_record(news, player)));
    }
    *next = ledger.next;
}

fn news_record(news: &News, player: Option<Entity>) -> chronicle::News {
    let record = |happening, ship: &str, by: Entity| chronicle::News {
        id: news.id.0,
        happening,
        deed: None,
        ship: ship.to_string(),
        other: None,
        group: None,
        player: Some(by) == player,
    };
    match &news.event {
        Happening::Sailed { ship, name, .. } => record(Kind::Sailed, name, *ship),
        Happening::Overdue { ship, name, .. } => record(Kind::Overdue, name, *ship),
        Happening::Wrecked { ship, name, .. } => record(Kind::Wrecked, name, *ship),
        Happening::Deed(deed) => chronicle::News {
            deed: Some(format!("{:?}", deed.kind)),
            other: deed.victim.as_ref().map(|(_, name)| name.clone()),
            group: Some(deed.group.to_string()),
            ..record(Kind::Deed, &deed.doer_name, deed.doer)
        },
        Happening::Account {
            teller,
            teller_name,
            name,
            ..
        } => chronicle::News {
            other: Some(name.clone()),
            ..record(Kind::Account, teller_name, *teller)
        },
    }
}

/// Notes each nation hearing of a deed, how long word took, and what it
/// did to the player's standing if it was hers.
fn note_heard(
    mut known: MessageReader<DeedKnown>,
    ledger: Res<Ledger>,
    clock: Res<WorldClock>,
    player: Query<(&CaptainId, &Standing), With<Player>>,
    mut chronicle: ResMut<Chronicle>,
) {
    for heard in known.read() {
        let Some(news) = ledger.get(heard.news) else {
            continue;
        };
        chronicle.note(Record::Heard {
            news: heard.news.0,
            nation: heard.nation.to_string(),
            after_days: (clock.elapsed - news.at) / DAY,
        });
        let (Happening::Deed(deed), Ok((captain, standing))) = (&news.event, player.single())
        else {
            continue;
        };
        if deed.captain != Some(*captain) {
            continue;
        }
        for (group, by) in deed.standing_for(heard.nation, heard.answered) {
            chronicle.note(Record::Standing {
                group: group.to_string(),
                by,
                now: standing.of(group),
                deed: format!("{:?}", deed.kind),
            });
        }
    }
}

/// Notes every pair of nations coming to new terms.
fn note_terms(
    relations: Res<Relations>,
    mut was: Local<Option<Vec<Terms>>>,
    mut chronicle: ResMut<Chronicle>,
) {
    let now: Vec<Terms> = pairs().map(|[a, b]| relations.terms(a, b)).collect();
    if let Some(was) = was.as_ref() {
        for (([a, b], before), after) in pairs().zip(was).zip(&now) {
            if before != after {
                chronicle.note(Record::Terms {
                    nations: [a.to_string(), b.to_string()],
                    terms: after.name().to_string(),
                    value: relations.between(a, b),
                });
            }
        }
    }
    *was = Some(now);
}

/// Notes ports going over from one nation to another.
fn note_ports(
    ports: Query<(Entity, &Name, &Port), Changed<Port>>,
    mut flags: Local<HashMap<Entity, Nation>>,
    mut chronicle: ResMut<Chronicle>,
) {
    for (entity, name, port) in &ports {
        if let Some(was) = flags.insert(entity, port.flag)
            && was != port.flag
        {
            chronicle.note(Record::WentOver {
                port: name.to_string(),
                from: was.to_string(),
                to: port.flag.to_string(),
            });
        }
    }
}

/// Notes the player's purse as it changes.
fn note_money(
    player: Single<&Purse, With<Player>>,
    mut was: Local<Option<u32>>,
    mut chronicle: ResMut<Chronicle>,
) {
    let purse = player.0;
    if let Some(was) = *was
        && was != purse
    {
        chronicle.note(Record::Money {
            by: i64::from(purse) - i64::from(was),
            purse,
        });
    }
    *was = Some(purse);
}

fn note_notes(mut notes: MessageReader<Note>, mut chronicle: ResMut<Chronicle>) {
    for Note(text) in notes.read() {
        chronicle.note(Record::Note { text: text.clone() });
    }
}

type PlayerShip<'a> = (
    &'a Name,
    &'a ShipClass,
    &'a Purse,
    (&'a Timbers, &'a Rigging, &'a Crew, &'a Hold),
    Option<&'a Docked>,
    Option<&'a Colours>,
    &'a Standing,
);

/// Notes how things stand.
fn snapshot(
    player: Query<PlayerShip, With<Player>>,
    ships: Query<(Option<&Colours>, Roles), With<ShipClass>>,
    ports: Query<(&Name, &Port, &Market)>,
    open_jobs: Query<(), (With<Job>, Without<Carrier>)>,
    entities: Query<()>,
    names: Query<&Name>,
    relations: Res<Relations>,
    ledger: Res<Ledger>,
    goods: Res<Goods>,
    mut chronicle: ResMut<Chronicle>,
) {
    let player = player.single().ok().map(
        |(name, class, purse, (timbers, rigging, crew, hold), docked, colours, standing)| Captain {
            ship: name.to_string(),
            class: class.name.to_string(),
            purse: purse.0,
            timbers: timbers.0,
            rigging: rigging.0,
            crew: crew.count,
            morale: crew.morale,
            cargo_tons: hold.tons_used(&goods),
            docked: docked
                .and_then(|docked| names.get(docked.port).ok())
                .map(Name::to_string),
            colours: colours.map(|colours| colours.0.to_string()),
            standing: groups()
                .map(|group| GroupStanding {
                    group: group.to_string(),
                    value: standing.of(group),
                })
                .collect(),
            wanted: nations()
                .filter(|&nation| standing.wanted_by(nation))
                .map(|nation| nation.to_string())
                .collect(),
        },
    );
    let mut fleets = BTreeMap::new();
    for (colours, roles) in &ships {
        *fleets
            .entry((colours.map(|colours| colours.0), role(roles)))
            .or_insert(0) += 1;
    }
    let mut ports: Vec<PortState> = ports
        .iter()
        .map(|(name, port, market)| PortState {
            port: name.to_string(),
            flag: port.flag.to_string(),
            goods: market
                .entries
                .iter()
                .map(|entry| {
                    let good = goods.get(entry.good);
                    GoodState {
                        good: good.name.clone(),
                        stock: entry.stock,
                        target: entry.target,
                        price: unit_price(good.base_price, entry.stock, entry.target),
                        needed: entry.is_needed(),
                    }
                })
                .collect(),
        })
        .collect();
    ports.sort_by(|a, b| a.port.cmp(&b.port));
    chronicle.note(Record::Snapshot(Box::new(Snapshot {
        player,
        relations: pairs()
            .map(|[a, b]| Pair {
                nations: [a.to_string(), b.to_string()],
                value: relations.between(a, b),
            })
            .collect(),
        fleets: fleets
            .into_iter()
            .map(|((nation, role), ships)| Fleet {
                nation: nation.map(|nation| nation.to_string()),
                role,
                ships,
            })
            .collect(),
        ports,
        open_jobs: open_jobs.iter().count() as u32,
        news: ledger.news.len() as u32,
        entities: entities.iter().count() as u32,
        memory_kb: memory_kb(),
    })));
}

/// The game's resident memory, kilobytes, as `ps` tells it.
#[cfg(unix)]
fn memory_kb() -> Option<u64> {
    let pid = std::process::id().to_string();
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()?;
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

/// Windows has no `ps`, and there's no way to ask without `unsafe`.
#[cfg(not(unix))]
fn memory_kb() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chronicle::Run;
    use crate::chronicle::tests::{noted, noting_app};
    use crate::factions::{DeedKind, Group, GroupKind};
    use crate::word::{DeedNews, Place};

    fn snapshots(records: &[Record]) -> Vec<&Snapshot> {
        records
            .iter()
            .filter_map(|record| match record {
                Record::Snapshot(snapshot) => Some(snapshot.as_ref()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_snapshot_is_taken_once_a_day_and_with_every_note() {
        let mut app = noting_app(Run::Soak);
        app.update();
        assert_eq!(snapshots(&noted(&mut app)).len(), 1, "one at the start");
        app.update();
        assert!(snapshots(&noted(&mut app)).is_empty());

        app.world_mut().write_message(Note("too slow".into()));
        app.update();
        let records = noted(&mut app);
        assert_eq!(
            records[0],
            Record::Note {
                text: "too slow".into()
            }
        );
        assert_eq!(snapshots(&records).len(), 1);

        app.world_mut().resource_mut::<WorldClock>().elapsed += DAY;
        app.update();
        let records = noted(&mut app);
        let [snapshot] = snapshots(&records)[..] else {
            panic!("no snapshot the next day: {records:?}");
        };
        assert_eq!(snapshot.relations.len(), pairs().count());
    }

    #[test]
    fn the_players_money_is_noted_as_it_changes() {
        let mut app = noting_app(Run::Soak);
        let player = app.world_mut().spawn((Player, Purse(500))).id();
        app.update();
        noted(&mut app);
        app.world_mut().get_mut::<Purse>(player).unwrap().0 = 380;
        app.update();
        assert!(noted(&mut app).contains(&Record::Money {
            by: -120,
            purse: 380
        }));
    }

    #[test]
    fn word_of_the_players_deed_reaching_a_nation_notes_the_standing_it_moved() {
        let mut app = noting_app(Run::Soak);
        let merchants = Group {
            nation: Nation::ENGLAND,
            kind: GroupKind::Merchants,
        };
        let player = app
            .world_mut()
            .spawn((Player, CaptainId(7), Standing::default()))
            .id();
        let deed = DeedNews {
            kind: DeedKind::Captured,
            group: merchants,
            doer: player,
            doer_name: "Swallow".into(),
            captain: Some(CaptainId(7)),
            colours: Some(Nation::PIRATES),
            victim: Some((Entity::PLACEHOLDER, "Dove".into())),
            standing: vec![(merchants, -20.0)],
        };
        let id = app.world_mut().resource_mut::<Ledger>().record(
            0.0,
            Place::Sea(Vec3::ZERO),
            Happening::Deed(deed),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        app.world_mut().resource_mut::<WorldClock>().elapsed = 2.0 * DAY;
        app.update();
        let news = noted(&mut app);
        assert!(news.iter().any(|record| matches!(
            record,
            Record::News(chronicle::News { deed: Some(deed), other: Some(victim), player: true, .. })
                if deed == "Captured" && victim == "Dove"
        )));

        app.world_mut().write_message(DeedKnown {
            news: id,
            nation: Nation::ENGLAND,
            answered: Some(DeedKind::Attacked),
        });
        app.update();
        let records = noted(&mut app);
        assert!(records.iter().any(|record| matches!(
            record,
            Record::Heard { news, nation, after_days }
                if *news == id.0
                    && *nation == Nation::ENGLAND.to_string()
                    && (after_days - 2.0).abs() < 0.01
        )));
        let owed = 1.0 - DeedKind::Attacked.standing() / DeedKind::Captured.standing();
        assert!(records.iter().any(|record| matches!(
            record,
            Record::Standing { group, by, .. }
                if *group == merchants.to_string() && (*by + 20.0 * owed).abs() < 1e-4
        )));
    }
}
