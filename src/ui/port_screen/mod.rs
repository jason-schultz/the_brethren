//! The port screen shown while the player's ship is docked: the town and
//! its locations. Changes the world only by sending orders.

mod model;

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::panel::{Shown, screen, show};
use crate::allegiance::Favour;
use crate::cargo::{Hold, Purse};
use crate::clock::WorldClock;
use crate::combat::Timbers;
use crate::crew::{Complement, Crew, ShoreLeave};
use crate::economy::jobs::{AcceptJob, Carrier, Job};
use crate::economy::market::{Market, Trade, Trader};
use crate::escorts::{AcceptEscort, Escort};
use crate::factions::{Colours, Standing};
use crate::goods::Goods;
use crate::officers::{Berth, Officer, OfficerAct, OfficerOrder, Quarterdeck, posts};
use crate::pardons::{Plea, SeekPardon};
use crate::player::Player;
use crate::port::{CastOff, Docked, Port};
use crate::prizes::Prize;
use crate::sailing::Rigging;
use crate::schedule::FrameSet;
use crate::service::{Join, Service};
use crate::ship_class::ShipClass;
use crate::shipyard::{
    BuyShip, PartExchange, RepairOrder, SellPrize, TakeCommand, check_purchase, prize_value,
    repair_cost,
};
use crate::ui::{ago, fate_text, nearest_port, tale_text};
use crate::word::{GiveAccount, Knowledge, Ledger};
use model::{
    AccountListing, Action, Candidate, Condition, Context, EscortListing, Listing, Offer,
    PrizeListing, View,
};

/// The screen's root node.
#[derive(Component)]
struct PortScreen;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<View>()
        .add_observer(open_port_screen)
        .add_observer(close_port_screen)
        // After orders, so the screen shows their results the same frame.
        .add_systems(Update, refresh.after(FrameSet::Orders));
}

fn open_port_screen(
    add: On<Add<Docked>>,
    players: Query<(), With<Player>>,
    mut view: ResMut<View>,
    mut commands: Commands,
) {
    if !players.contains(add.entity) {
        return;
    }
    *view = View::Town;
    commands.spawn(screen::<Action>(PortScreen));
}

fn close_port_screen(
    remove: On<Remove<Docked>>,
    players: Query<(), With<Player>>,
    screens: Query<Entity, With<PortScreen>>,
    mut commands: Commands,
) {
    if players.contains(remove.entity) {
        for screen in &screens {
            commands.entity(screen).despawn();
        }
    }
}

/// Rebuilds the panel when what it should show has changed. The model is
/// built every frame while docked: it is small, and comparing it is far
/// simpler than tracking every value it depends on.
fn refresh(
    view: Res<View>,
    goods: Res<Goods>,
    player: Single<
        (
            (Entity, &Docked, &Hold, &Purse, &Crew, &Complement),
            (&ShipClass, &Timbers, &Rigging, Option<&Colours>),
            (&Standing, &Service, &Knowledge),
        ),
        With<Player>,
    >,
    prizes: Query<(
        Entity,
        &Name,
        &Prize,
        &Docked,
        &ShipClass,
        &Timbers,
        &Rigging,
        &Hold,
    )>,
    ports: Query<(&Name, &Port, &Favour, &Market)>,
    jobs: Query<(Entity, &Job, Option<&Carrier>)>,
    (escorts, ledger, clock): (Query<(Entity, &Escort)>, Res<Ledger>, Res<WorldClock>),
    anchorages: Query<(&Name, &Transform), With<Port>>,
    officers: Query<(Entity, &Name, &Officer, &Berth)>,
    names: Query<&Name>,
    panel: Single<(Entity, &mut Shown<Action>)>,
    mut commands: Commands,
) {
    let (
        (ship, docked, hold, purse, crew, complement),
        (class, timbers, rigging, colours),
        (standing, service, knowledge),
    ) = *player;
    let (entity, mut shown) = panel.into_inner();
    let Ok((port_name, port, favour, market)) = ports.get(docked.port) else {
        return;
    };
    let posts = posts(
        ship,
        officers
            .iter()
            .map(|(_, _, officer, berth)| (officer, berth)),
    );
    let context = Context {
        goods: &goods,
        port_entity: docked.port,
        port_name: port_name.as_str(),
        port,
        favour,
        market,
        trader: Trader {
            hold,
            purse: *purse,
            crew,
            complement,
        },
        jobs: jobs
            .iter()
            // Jobs other ships carry aren't the player's business.
            .filter(|(_, _, carrier)| carrier.is_none_or(|carrier| carrier.0 == ship))
            .map(|(entity, job, carrier)| Listing {
                entity,
                job,
                destination: names.get(job.destination).map_or("?", Name::as_str),
                carried: carrier.is_some(),
            })
            .collect(),
        escorts: escorts
            .iter()
            .filter(|(_, escort)| match escort.escort {
                Some(guard) => guard == ship,
                None => escort.origin == docked.port,
            })
            .map(|(entity, escort)| EscortListing {
                entity,
                escort,
                ward: names.get(escort.ward).map_or("?", Name::as_str),
                destination: names.get(escort.destination).map_or("?", Name::as_str),
                cargo: jobs
                    .iter()
                    .find(|(_, job, carrier)| {
                        carrier.is_some_and(|carrier| carrier.0 == escort.ward)
                            && job.destination == escort.destination
                    })
                    .map(|(_, job, _)| (job.good, job.quantity)),
                taken: escort.escort.is_some(),
            })
            .collect(),
        // What the tavern talks of, as it told her.
        word: ledger
            .news
            .iter()
            .filter(|news| news.known_at.contains(&docked.port))
            .filter_map(|news| knowledge.of(news.id))
            .map(|tale| {
                tale_text(
                    &goods,
                    tale,
                    clock.elapsed,
                    |entity| names.get(entity).map_or("?", Name::as_str).to_string(),
                    |at| nearest_port(&anchorages, at),
                )
            })
            .collect(),
        accounts: ledger
            .accountable(ship)
            .map(|loss| AccountListing {
                lost: loss.ship,
                asked: format!(
                    "{} was lost {}, with you in those waters. How was she lost?",
                    loss.name,
                    ago(clock.elapsed - loss.at)
                ),
                choices: loss
                    .accounts(ship)
                    .into_iter()
                    .map(|says| {
                        let blames = says.by().and_then(|&by| loss.name_of(by)).unwrap_or("?");
                        let mut label = fate_text(says, blames);
                        if let Some(first) = label.get_mut(0..1) {
                            first.make_ascii_uppercase();
                        }
                        (says, label)
                    })
                    .collect(),
            })
            .collect(),
        standing,
        service,
        colours: colours.map(|colours| colours.0),
        condition: Condition {
            timbers: timbers.0,
            rigging: rigging.0,
            repair_cost: repair_cost(class, timbers.0, rigging.0),
        },
        offers: ShipClass::for_sale()
            .filter(|offered| offered != class)
            .map(|offered| Offer {
                class: offered,
                cost: check_purchase(
                    &goods,
                    port,
                    &offered,
                    PartExchange {
                        class,
                        timbers: timbers.0,
                        rigging: rigging.0,
                        hold,
                        crew,
                    },
                    *purse,
                ),
            })
            .collect(),
        prizes: prizes
            .iter()
            .filter(|(_, _, prize, prize_docked, ..)| {
                prize.captor == ship && prize_docked.port == docked.port
            })
            .map(
                |(entity, name, _, _, class, timbers, rigging, hold)| PrizeListing {
                    entity,
                    name: name.as_str(),
                    timbers: timbers.0,
                    rigging: rigging.0,
                    value: prize_value(&goods, class, timbers.0, rigging.0, hold),
                },
            )
            .collect(),
        deck: Quarterdeck {
            ship,
            docked: Some(docked.port),
            purse: *purse,
            posts: &posts,
        },
        candidates: officers
            .iter()
            .filter(|(_, _, _, berth)| {
                matches!(berth, Berth::ForHire { port, .. } if *port == docked.port)
            })
            .map(|(entity, name, officer, berth)| Candidate {
                entity,
                name: name.as_str(),
                officer,
                berth: *berth,
            })
            .collect(),
    };
    let content = model::build(*view, &context);
    show(&mut commands, (entity, &mut shown), content, act);
}

/// Turns a pressed button into an order, or a move around town.
fn act(
    activate: On<Activate>,
    actions: Query<&Action>,
    player: Single<Entity, With<Player>>,
    mut view: ResMut<View>,
    mut trades: MessageWriter<Trade>,
    (mut accepts, mut escorts): (MessageWriter<AcceptJob>, MessageWriter<AcceptEscort>),
    mut cast_off: MessageWriter<CastOff>,
    mut repairs: MessageWriter<RepairOrder>,
    mut sales: MessageWriter<SellPrize>,
    (mut commands, mut purchases): (MessageWriter<TakeCommand>, MessageWriter<BuyShip>),
    mut shore_leave: MessageWriter<ShoreLeave>,
    mut hires: MessageWriter<OfficerOrder>,
    mut joins: MessageWriter<Join>,
    (mut pleas, mut accounts): (MessageWriter<SeekPardon>, MessageWriter<GiveAccount>),
) {
    let Ok(&action) = actions.get(activate.entity) else {
        return;
    };
    let ship = *player;
    match action {
        Action::Go(to) => *view = to,
        Action::Trade {
            good,
            quantity,
            side,
        } => {
            trades.write(Trade {
                ship,
                good,
                quantity,
                side,
            });
        }
        Action::Accept(job) => {
            accepts.write(AcceptJob { ship, job });
        }
        Action::AcceptEscort(escort) => {
            escorts.write(AcceptEscort { ship, escort });
        }
        Action::CastOff => {
            cast_off.write(CastOff { ship });
        }
        Action::Repair => {
            repairs.write(RepairOrder { ship });
        }
        Action::SellPrize(prize) => {
            sales.write(SellPrize { prize });
        }
        Action::TakeCommand(prize) => {
            commands.write(TakeCommand { prize });
        }
        Action::BuyShip(class) => {
            purchases.write(BuyShip { ship, class });
        }
        Action::ShoreLeave => {
            shore_leave.write(ShoreLeave { ship });
        }
        Action::Hire(officer) => {
            hires.write(OfficerOrder {
                ship,
                officer,
                act: OfficerAct::Hire,
            });
        }
        Action::Join(group) => {
            joins.write(Join {
                captain: ship,
                group,
            });
        }
        Action::Petition(nation) => {
            pleas.write(SeekPardon {
                captain: ship,
                nation,
                plea: Plea::Petition,
            });
        }
        Action::GiveAccount { lost, says } => {
            accounts.write(GiveAccount { ship, lost, says });
        }
    }
}
