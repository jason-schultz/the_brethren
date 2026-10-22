//! The port screen shown while the player's ship is docked: the town and
//! its locations. Changes the world only by sending orders.

mod model;

use bevy::ecs::query::QueryData;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use super::panel::{Shown, screen, show};
use crate::allegiance::Favour;
use crate::cargo::{Hold, Purse};
use crate::combat::Timbers;
use crate::crew::{Complement, Crew, ShoreLeave};
use crate::economy::jobs::{AcceptJob, Carrier, Job};
use crate::economy::market::{Market, Trade, Trader};
use crate::escorts::{AcceptEscort, Escort};
use crate::factions::{Colours, Standing};
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
use crate::ui::{Tales, ago, fate_text};
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

/// The player's ship lying in port, and her captain's record.
#[derive(QueryData)]
struct Ashore {
    entity: Entity,
    docked: &'static Docked,
    hold: &'static Hold,
    purse: &'static Purse,
    crew: &'static Crew,
    complement: &'static Complement,
    class: &'static ShipClass,
    timbers: &'static Timbers,
    rigging: &'static Rigging,
    colours: Option<&'static Colours>,
    standing: &'static Standing,
    service: &'static Service,
    knowledge: &'static Knowledge,
}

/// A prize, as the yard would value her.
#[derive(QueryData)]
struct Prized {
    entity: Entity,
    name: &'static Name,
    prize: &'static Prize,
    docked: &'static Docked,
    class: &'static ShipClass,
    timbers: &'static Timbers,
    rigging: &'static Rigging,
    hold: &'static Hold,
}

/// The exchange's business: contracts, and escorts asked for.
#[derive(SystemParam)]
struct ExchangeBoard<'w, 's> {
    jobs: Query<'w, 's, (Entity, &'static Job, Option<&'static Carrier>)>,
    escorts: Query<'w, 's, (Entity, &'static Escort)>,
}

/// Rebuilds the panel when what it should show has changed. The model is
/// built every frame while docked: it is small, and comparing it is far
/// simpler than tracking every value it depends on.
fn refresh(
    view: Res<View>,
    tales: Tales,
    player: Single<Ashore, With<Player>>,
    prizes: Query<Prized>,
    ports: Query<(&Name, &Port, &Favour, &Market)>,
    board: ExchangeBoard,
    ledger: Res<Ledger>,
    officers: Query<(Entity, &Name, &Officer, &Berth)>,
    panel: Single<(Entity, &mut Shown<Action>)>,
    mut commands: Commands,
) {
    let AshoreItem {
        entity: ship,
        docked,
        hold,
        purse,
        crew,
        complement,
        class,
        timbers,
        rigging,
        colours,
        standing,
        service,
        knowledge,
    } = player.into_inner();
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
        goods: &tales.goods,
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
        jobs: board.jobs
            .iter()
            // Jobs other ships carry aren't the player's business.
            .filter(|(_, _, carrier)| carrier.is_none_or(|carrier| carrier.0 == ship))
            .map(|(entity, job, carrier)| Listing {
                entity,
                job,
                destination: tales.name(job.destination),
                carried: carrier.is_some(),
            })
            .collect(),
        escorts: board
            .escorts
            .iter()
            .filter(|(_, escort)| match escort.escort {
                Some(guard) => guard == ship,
                None => escort.origin == docked.port,
            })
            .map(|(entity, escort)| EscortListing {
                entity,
                escort,
                ward: tales.name(escort.ward),
                destination: tales.name(escort.destination),
                cargo: board.jobs
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
                tales.tell(tale)
            })
            .collect(),
        accounts: ledger
            .accountable(ship)
            .map(|loss| AccountListing {
                lost: loss.ship,
                asked: format!(
                    "{} was lost {}, with you in those waters. How was she lost?",
                    loss.name,
                    ago(tales.clock.elapsed - loss.at)
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
                    &tales.goods,
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
            .filter(|prized| prized.prize.captor == ship && prized.docked.port == docked.port)
            .map(
                |PrizedItem {
                     entity,
                     name,
                     class,
                     timbers,
                     rigging,
                     hold,
                     ..
                 }| PrizeListing {
                    entity,
                    name: name.as_str(),
                    timbers: timbers.0,
                    rigging: rigging.0,
                    value: prize_value(&tales.goods, class, timbers.0, rigging.0, hold),
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

/// The orders a captain gives in port, one for each button that orders
/// something.
#[derive(SystemParam)]
struct PortOrders<'w> {
    trades: MessageWriter<'w, Trade>,
    accepts: MessageWriter<'w, AcceptJob>,
    escorts: MessageWriter<'w, AcceptEscort>,
    cast_off: MessageWriter<'w, CastOff>,
    repairs: MessageWriter<'w, RepairOrder>,
    sales: MessageWriter<'w, SellPrize>,
    commands: MessageWriter<'w, TakeCommand>,
    purchases: MessageWriter<'w, BuyShip>,
    shore_leave: MessageWriter<'w, ShoreLeave>,
    hires: MessageWriter<'w, OfficerOrder>,
    joins: MessageWriter<'w, Join>,
    pleas: MessageWriter<'w, SeekPardon>,
    accounts: MessageWriter<'w, GiveAccount>,
}

impl PortOrders<'_> {
    /// `ship`'s captain gives the order `action` stands for, if it's one.
    fn give(&mut self, ship: Entity, action: Action) {
        match action {
            // Going about town isn't an order.
            Action::Go(_) => {}
            Action::Trade {
                good,
                quantity,
                side,
            } => {
                self.trades.write(Trade {
                    ship,
                    good,
                    quantity,
                    side,
                });
            }
            Action::Accept(job) => {
                self.accepts.write(AcceptJob { ship, job });
            }
            Action::AcceptEscort(escort) => {
                self.escorts.write(AcceptEscort { ship, escort });
            }
            Action::CastOff => {
                self.cast_off.write(CastOff { ship });
            }
            Action::Repair => {
                self.repairs.write(RepairOrder { ship });
            }
            Action::SellPrize(prize) => {
                self.sales.write(SellPrize { prize });
            }
            Action::TakeCommand(prize) => {
                self.commands.write(TakeCommand { prize });
            }
            Action::BuyShip(class) => {
                self.purchases.write(BuyShip { ship, class });
            }
            Action::ShoreLeave => {
                self.shore_leave.write(ShoreLeave { ship });
            }
            Action::Hire(officer) => {
                self.hires.write(OfficerOrder {
                    ship,
                    officer,
                    act: OfficerAct::Hire,
                });
            }
            Action::Join(group) => {
                self.joins.write(Join {
                    captain: ship,
                    group,
                });
            }
            Action::Petition(nation) => {
                self.pleas.write(SeekPardon {
                    captain: ship,
                    nation,
                    plea: Plea::Petition,
                });
            }
            Action::GiveAccount { lost, says } => {
                self.accounts.write(GiveAccount { ship, lost, says });
            }
        }
    }
}

/// Turns a pressed button into an order, or a move around town.
fn act(
    activate: On<Activate>,
    actions: Query<&Action>,
    player: Single<Entity, With<Player>>,
    mut view: ResMut<View>,
    mut orders: PortOrders,
) {
    let Ok(&action) = actions.get(activate.entity) else {
        return;
    };
    match action {
        Action::Go(to) => *view = to,
        order => orders.give(*player, order),
    }
}
