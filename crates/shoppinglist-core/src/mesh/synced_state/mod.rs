use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use adaptive_backoff::backoff::{Backoff as _, BackoffBuilder as _};
use indexmap::IndexMap;
use iroh_docs::DocTicket;
use n0_error::{Result, StdResultExt as _, anyerr};
use n0_future::{BufferedStreamExt, TryStreamExt};
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, SupervisionEvent, call};
use ractor_actors::streams::spawn_stream_pump;
use tokio::sync::broadcast;
use tokio_stream::{StreamExt as _, wrappers::BroadcastStream};

use crate::{
    actor::{
        ActorStats, FlattenCall, HasStats, Instrumented, ShouldMeasureMessage,
        debouncer::{DebouncerActor, DebouncerArgs, DebouncerMsg},
    },
    models::{ShoppingListId, ShoppingListWithId},
};

use super::debug::{DebugReport, NetworkDebug};
use super::local_state::LocalStateMsg;
use super::shoppinglist::{
    SHOPPING_LIST_GROUP, ShoppingListActor, ShoppingListArgs, ShoppingListMsg,
    commands::ShoppingListEvent,
};

fn data_subdir(root: &std::path::Path, name: &str) -> Result<PathBuf> {
    let path = root.join(name);
    std::fs::create_dir_all(&path).std_context(format!("Could not create {}", path.display()))?;
    Ok(path)
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SyncedStateEvent {
    // Not sure what events we need here, or even if we need more than just a 'I changed' anyway.
    // Currently any event is interpreted to mean 'refresh everything'
    Todo,
}

#[derive(Clone)]
pub struct DocsApi {
    endpoint: iroh::Endpoint,
    pub blobs: iroh_blobs::api::Store,
    gossip: iroh_gossip::Gossip,
    pub docs: iroh_docs::protocol::Docs,
    router: iroh::protocol::Router,
    pub author: iroh_docs::AuthorId,
}

impl DocsApi {
    pub async fn new(data_dir: Option<&Path>) -> Result<Self> {
        let endpoint = iroh::Endpoint::bind(iroh::endpoint::presets::N0).await?;
        let gossip = iroh_gossip::Gossip::builder().spawn(endpoint.clone());

        let (blobs, docs) = match data_dir {
            Some(root) => {
                tracing::info!("Using {root:?} as data storage directory");
                let blobs: iroh_blobs::api::Store =
                    iroh_blobs::store::fs::FsStore::load(data_subdir(root, "blobs")?)
                        .await?
                        .into();
                let docs = iroh_docs::protocol::Docs::persistent(data_subdir(root, "docs")?)
                    .spawn(endpoint.clone(), blobs.clone(), gossip.clone())
                    .await?;
                (blobs, docs)
            }
            None => {
                tracing::info!("Keeping shopping list data in memory");
                let blobs: iroh_blobs::api::Store = iroh_blobs::store::mem::MemStore::new().into();
                let docs = iroh_docs::protocol::Docs::memory()
                    .spawn(endpoint.clone(), blobs.clone(), gossip.clone())
                    .await?;
                (blobs, docs)
            }
        };

        let router = iroh::protocol::Router::builder(endpoint.clone())
            .accept(
                iroh_blobs::ALPN,
                iroh_blobs::BlobsProtocol::new(&blobs, None),
            )
            .accept(iroh_gossip::ALPN, gossip.clone())
            .accept(iroh_docs::ALPN, docs.clone())
            .spawn();

        let author = docs.author_default().await?;

        Ok(Self {
            endpoint,
            blobs,
            gossip,
            docs,
            router,
            author,
        })
    }

    pub fn network_debug(&self) -> NetworkDebug {
        let socket = &self.endpoint.metrics().socket;
        let gossip = self.gossip.metrics();
        let opened = socket.num_conns_opened.get();
        let closed = socket.num_conns_closed.get();
        NetworkDebug {
            connections_open: opened.saturating_sub(closed),
            connections_opened_total: opened,
            connections_closed_total: closed,
            connections_direct_total: socket.num_conns_direct.get(),
            relay_connections_failed: socket.relay_conns_failed.get(),
            gossip_msgs_sent: gossip.msgs_ctrl_sent.get() + gossip.msgs_data_sent.get(),
            gossip_msgs_recv: gossip.msgs_ctrl_recv.get() + gossip.msgs_data_recv.get(),
            gossip_bytes_sent: gossip.msgs_data_sent_size.get(),
            gossip_bytes_recv: gossip.msgs_data_recv_size.get(),
        }
    }

    pub async fn shutdown(self) {
        tracing::info!("Shutting down docs api");
        if let Err(err) = self.router.shutdown().await {
            tracing::error!("Could not shut down shopping list sync: {err}");
        }
        self.endpoint.close().await;
    }
}

pub struct SyncedStateActor;

pub struct SyncedStateArgs {
    pub local_db: ActorRef<LocalStateMsg>,
    pub data_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct ChildInfo {
    /// If this is none, it means the list actor is restarting.
    /// If we're being asked to open it, then we should retry immediately
    actor: Option<ActorRef<ShoppingListMsg>>,
    backoff: adaptive_backoff::backoff::ExponentialBackoff,
}

impl ChildInfo {
    fn new(actor: ActorRef<ShoppingListMsg>) -> Self {
        let backoff = adaptive_backoff::backoff::ExponentialBackoffBuilder::default()
            .min(Duration::from_millis(100))
            .max(Duration::from_secs(10))
            .build()
            .unwrap();
        Self {
            actor: Some(actor),
            backoff,
        }
    }
}

pub struct SyncedStateState {
    api: DocsApi,
    local_db: ActorRef<LocalStateMsg>,

    debouncer: ActorRef<DebouncerMsg<SyncedStateEvent>>,
    event_receiver: broadcast::Receiver<SyncedStateEvent>,

    shopping_list_actors: IndexMap<ShoppingListId, ChildInfo>,

    stats: ActorStats,
}

impl HasStats for SyncedStateState {
    fn stats_mut(&mut self) -> &mut ActorStats {
        &mut self.stats
    }
}

impl SyncedStateState {
    fn notify_changed(&self) {
        if let Err(err) = ractor::cast!(
            self.debouncer,
            DebouncerMsg::Enqueue(SyncedStateEvent::Todo)
        ) {
            tracing::warn!("Failed to send event: {err:?}");
        };
    }

    async fn spawn_shopping_list(
        &self,
        myself: &ActorRef<SyncedStateMsg>,
        ticket: Option<DocTicket>,
        allow_creation: bool,
    ) -> Result<ActorRef<ShoppingListMsg>> {
        let api = self.api.clone();

        let (actor, _handle) = Actor::spawn_linked(
            None,
            Instrumented(ShoppingListActor),
            ShoppingListArgs {
                local_state: self.local_db.clone(),
                api,
                ticket,
                allow_creation,
            },
            myself.get_cell(),
        )
        .await
        .map_err(|err| anyerr!("Could not start shopping list actor: {err}"))?;

        let shopping_list_events = call!(actor, ShoppingListMsg::SubscribeChanges).anyerr()?;

        let events = BroadcastStream::new(shopping_list_events).filter_map(|item| item.ok());
        if let Err(err) = spawn_stream_pump(
            events,
            myself.clone(),
            SyncedStateMsg::ShoppingListEvent,
            None,
        )
        .await
        {
            actor.stop(None);
            return Err(anyerr!("Could not forward shopping list events: {err}"));
        }

        Ok(actor)
    }

    /// Stops tracking a shopping list whose actor has stopped, returning its id.
    fn forget_child(&mut self, who: &ActorCell) -> Option<ShoppingListId> {
        let id = self.id_of(who)?;
        self.shopping_list_actors.shift_remove(&id);
        self.notify_changed();
        Some(id)
    }

    fn id_of(&self, who: &ActorCell) -> Option<ShoppingListId> {
        self.shopping_list_actors
            .iter()
            .filter_map(|(id, info)| info.actor.as_ref().map(|a| (*id, a)))
            .find(|(_id, actor)| actor.get_id() == who.get_id())
            .map(|(id, _)| id)
    }
}

impl ShouldMeasureMessage for SyncedStateMsg {
    fn should_measure(&self) -> bool {
        !matches!(self, Self::GetDebugReport(_))
    }
}

#[ractor::actor(
    message = pub enum SyncedStateMsg,
    state = SyncedStateState,
    arguments = SyncedStateArgs,
)]
impl SyncedStateActor {
    #[tracing::instrument(skip(self, myself, args))]
    async fn pre_start(
        &self,
        myself: ActorRef<SyncedStateMsg>,
        args: SyncedStateArgs,
    ) -> Result<SyncedStateState, ActorProcessingErr> {
        let api = DocsApi::new(args.data_dir.as_deref()).await?;

        let (event_sender, event_receiver) = broadcast::channel(10);
        let (debouncer, _) = Actor::spawn_linked(
            None,
            DebouncerActor::new(),
            DebouncerArgs {
                callback: Box::new(move |msg| {
                    let _ = event_sender.send(msg);
                    Ok(())
                }),
            },
            myself.get_cell(),
        )
        .await?;

        Ok(SyncedStateState {
            api,
            local_db: args.local_db,
            event_receiver,
            debouncer,
            shopping_list_actors: IndexMap::new(),
            stats: ActorStats::new("synced-state"),
        })
    }

    async fn post_start(
        &self,
        myself: ActorRef<SyncedStateMsg>,
        state: &mut SyncedStateState,
    ) -> Result<(), ActorProcessingErr> {
        let tracked_shopping_lists =
            call!(state.local_db, LocalStateMsg::GetTrackedShoppingLists).flatten_call()?;

        for tracked in tracked_shopping_lists.shopping_lists.values() {
            match state
                .spawn_shopping_list(&myself, Some(tracked.ticket.clone()), false)
                .await
            {
                Ok(actor) => {
                    state
                        .shopping_list_actors
                        .insert(tracked.id, ChildInfo::new(actor));
                }
                Err(err) => tracing::error!("Could not open shopping list {}: {err:?}", tracked.id),
            }
        }
        state.notify_changed();
        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<SyncedStateMsg>,
        state: &mut SyncedStateState,
    ) -> Result<(), ActorProcessingErr> {
        tracing::info!("Synced state actor shutting down");

        // Kill the shopping list actors before we stop ourselves
        for (_, info) in state.shopping_list_actors.drain(..) {
            let Some(actor) = info.actor else { continue };
            let _ = actor.get_cell().stop_and_wait(None, None).await;
        }

        state.api.clone().shutdown().await;
        Ok(())
    }

    /// Open a shopping list, this can be used to either import a shared shopping
    /// list, or open an existing.
    ///
    /// Used to both import shopping lists and open existing ones
    #[ractor::rpc(OpenShoppingList { id, ticket, reply })]
    #[tracing::instrument(skip(self, state))]
    async fn open_shopping_list(
        &self,
        myself: ActorRef<SyncedStateMsg>,
        id: ShoppingListId,
        ticket: Option<DocTicket>,
        state: &mut SyncedStateState,
    ) -> Result<ActorRef<ShoppingListMsg>> {
        if let Some(existing) = state
            .shopping_list_actors
            .get(&id)
            .and_then(|i| i.actor.as_ref())
        {
            return Ok(existing.clone());
        }

        let actor = state.spawn_shopping_list(&myself, ticket, false).await?;
        state
            .shopping_list_actors
            .insert(id, ChildInfo::new(actor.clone()));
        state.notify_changed();
        Ok(actor)
    }

    #[ractor::rpc(CreateShoppingList(name, reply))]
    #[tracing::instrument(skip(self, state))]
    async fn create_shopping_list(
        &self,
        myself: ActorRef<SyncedStateMsg>,
        name: String,
        state: &mut SyncedStateState,
    ) -> Result<ActorRef<ShoppingListMsg>> {
        let name = name.trim().to_string();
        let actor = state.spawn_shopping_list(&myself, None, true).await?;

        call!(actor, ShoppingListMsg::Rename, name).flatten_call()?;
        let id = call!(actor, ShoppingListMsg::GetId).anyerr()?;
        state
            .shopping_list_actors
            .insert(id, ChildInfo::new(actor.clone()));

        state.notify_changed();
        Ok(actor)
    }

    #[ractor::message(ShoppingListEvent(event))]
    fn shopping_list_event(&self, event: Option<ShoppingListEvent>, state: &SyncedStateState) {
        if let Some(event) = event {
            tracing::info!("Synced state notify changed: {event:?}");
            state.notify_changed();
        }
    }

    #[ractor::rpc(RemoveShoppingList(id, reply))]
    #[tracing::instrument(skip(self, state))]
    async fn remove_shopping_list(
        &self,
        id: ShoppingListId,
        state: &mut SyncedStateState,
    ) -> Result<()> {
        tracing::info!("Removing shopping list with id: {id}");

        let shopping_list_info = state.shopping_list_actors.shift_remove(&id);
        let local = state.local_db.clone();

        if let Some(actor) = shopping_list_info.and_then(|i| i.actor)
            && let Err(err) = call!(actor, ShoppingListMsg::DeleteSelf).flatten_call()
        {
            tracing::warn!("Failed to delete shopping list actor: {err:?}");
        }

        state.api.docs.drop_doc(id.id).await?;

        call!(local, LocalStateMsg::RemoveTrackedShoppingList, id).flatten_call()?;
        state.notify_changed();
        Ok(())
    }

    #[ractor::rpc(ListShoppingLists(reply))]
    #[tracing::instrument(skip(self, state))]
    async fn list_shopping_lists(
        &self,
        state: &SyncedStateState,
    ) -> Result<Vec<ShoppingListWithId>> {
        let shopping_lists = state.shopping_list_actors.clone();

        let shopping_list_states = n0_future::stream::iter(
            shopping_lists
                .into_iter()
                .filter_map(|(id, info)| info.actor.map(|a| (id, a)))
                .map(async |(id, actor)| {
                    let inner = call!(actor, ShoppingListMsg::GetState).flatten_call()?;
                    Ok::<_, n0_error::AnyError>(ShoppingListWithId { id, inner })
                }),
        )
        .buffered_ordered(8)
        .try_collect()
        .await?;

        Ok(shopping_list_states)
    }

    #[ractor::rpc(GetDebugReport(reply))]
    async fn get_debug_report(
        &self,
        myself: ActorRef<SyncedStateMsg>,
        state: &SyncedStateState,
    ) -> Result<DebugReport> {
        let network = state.api.network_debug();
        let synced_state = state.stats.snapshot(&myself.get_cell());
        let local_state = state.local_db.clone();
        let shopping_lists: Vec<ActorRef<ShoppingListMsg>> =
            ractor::pg::get_members(&SHOPPING_LIST_GROUP.to_string())
                .into_iter()
                .map(ActorRef::from)
                .collect();

        let local_state = call!(local_state, LocalStateMsg::GetDebugStatus).flatten_call()?;
        let mut shopping_list_status = Vec::new();
        for shopping_list in shopping_lists {
            match call!(shopping_list, ShoppingListMsg::GetDebugStatus).flatten_call() {
                Ok(status) => shopping_list_status.push(status),
                Err(err) => {
                    tracing::debug!("Could not get shopping list debug status: {err:?}")
                }
            }
        }
        Ok(DebugReport {
            local_state,
            synced_state,
            network,
            shopping_lists: shopping_list_status,
        })
    }

    #[ractor::rpc(SubscribeChanges(reply))]
    fn subscribe_changes(&self, state: &SyncedStateState) -> broadcast::Receiver<SyncedStateEvent> {
        state.event_receiver.resubscribe()
    }

    #[ractor::supervision(SupervisionEvent::ActorTerminated(who, _, _))]
    #[tracing::instrument(skip(self, state))]
    fn shopping_list_terminated(&self, who: ActorCell, state: &mut SyncedStateState) {
        if let Some(id) = state.forget_child(&who) {
            tracing::debug!("Shopping list {id} stopped");
        }
    }

    #[ractor::supervision(SupervisionEvent::ActorStarted(who))]
    #[tracing::instrument(skip(self, state))]
    fn shopping_list_started(&self, who: ActorCell, state: &mut SyncedStateState) {
        if let Some(info) = state
            .id_of(&who)
            .and_then(|id| state.shopping_list_actors.get_mut(&id))
        {
            info.backoff.reset();
        }
    }

    #[ractor::supervision(SupervisionEvent::ActorFailed(who, error))]
    #[tracing::instrument(skip(self, state))]
    async fn shopping_list_failed(
        &self,
        myself: ActorRef<SyncedStateMsg>,
        who: ActorCell,
        error: ActorProcessingErr,
        state: &mut SyncedStateState,
    ) {
        let Some(id) = state.id_of(&who) else { return };
        let Some(info) = state.shopping_list_actors.get_mut(&id) else {
            return;
        };

        // Clear the actor. If another task tries to start up this list again we
        // can try immediately, otherwise we try again after the retry period.
        info.actor = None;

        let wait_time = info.backoff.wait();

        tracing::warn!("Shopping list {id} failed, retrying in {wait_time:?}: {error:?}");
        tokio::spawn(async move {
            tokio::time::sleep(wait_time).await;
            let _ = myself.cast(SyncedStateMsg::RestartShoppingList { id });
        });
    }

    /// Reopens a shopping list that failed, unless it has been reopened or removed since.
    #[ractor::message(RestartShoppingList { id })]
    async fn restart_shopping_list(
        &self,
        myself: ActorRef<SyncedStateMsg>,
        id: ShoppingListId,
        state: &mut SyncedStateState,
    ) {
        if state
            .shopping_list_actors
            .get(&id)
            .is_some_and(|i| i.actor.is_some())
        {
            return;
        }

        let tracked = call!(state.local_db, LocalStateMsg::GetTrackedShoppingLists).flatten_call();

        let Some(tracked) = tracked.ok().and_then(|tracked_shopping_lists| {
            tracked_shopping_lists.shopping_lists.get(&id).cloned()
        }) else {
            tracing::warn!("Shopping list {id} is no longer tracked, not restarting");
            state.shopping_list_actors.swap_remove(&id);
            return;
        };

        match state
            .spawn_shopping_list(&myself, Some(tracked.ticket), false)
            .await
        {
            Ok(actor) => {
                state.shopping_list_actors.insert(id, ChildInfo::new(actor));
                state.notify_changed();
            }
            Err(err) => tracing::error!("Could not restart shopping list {id}: {err:?}"),
        }
    }
}
