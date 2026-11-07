use chrono::{DateTime, Utc};
use iroh_docs::{DocTicket, engine::LiveEvent};
use n0_error::{Result, StackResultExt, anyerr};
use n0_future::StreamExt as _;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, SupervisionEvent};
use ractor_actors::streams::spawn_stream_pump;
use tokio::sync::broadcast;

use super::debug::ShoppingListDebug;
use crate::models::{ShoppingListId, ShoppingListItem, ShoppingListItemId};
use crate::{
    actor::{
        ActorStats, FlattenCall, HasStats, ShouldMeasureMessage,
        debouncer::{DebouncerActor, DebouncerArgs, DebouncerMsg},
        tell,
    },
    models::ShoppingListItemIndex,
};

use self::{
    commands::{ShoppingListEvent, ShoppingListEventInner},
    store::{ShoppingList, ShoppingListDB},
};

use super::{local_state::LocalStateMsg, synced_state::DocsApi};

pub mod commands;
pub mod store;

pub const SHOPPING_LIST_GROUP: &str = "shopping-lists";

#[derive(Clone)]
struct LiveState {
    db: store::ShoppingListDB,

    doc: iroh_docs::api::Doc,
}

pub struct ShoppingListActor;

pub struct ShoppingListArgs {
    pub local_state: ActorRef<LocalStateMsg>,
    pub api: DocsApi,
    pub ticket: Option<DocTicket>,

    // TODO: probably consolidate ticket + allow_creation into a tri-state for new, open, import
    pub allow_creation: bool,
}

pub struct ShoppingListState {
    args: ShoppingListArgs,
    live_state: LiveState,
    stats: ActorStats,
    debouncer: ActorRef<DebouncerMsg<ShoppingListEvent>>,
    event_receiver: broadcast::Receiver<ShoppingListEvent>,
}

impl HasStats for ShoppingListState {
    fn stats_mut(&mut self) -> &mut ActorStats {
        &mut self.stats
    }
}

impl ShoppingListState {
    fn id(&self) -> ShoppingListId {
        self.live_state.doc.id().into()
    }

    fn emit(&self, id: ShoppingListId) {
        if let Err(err) = ractor::cast!(
            self.debouncer,
            DebouncerMsg::Enqueue(ShoppingListEvent {
                id,
                event: ShoppingListEventInner::Todo,
            })
        ) {
            tracing::warn!("Failed to send event: {err:?}");
        };
    }

    async fn mutation<F>(&self, operation: F) -> Result<ShoppingList>
    where
        F: AsyncFnOnce(&ShoppingListDB) -> Result<()> + Send + 'static,
    {
        operation(&self.live_state.db).await?;
        let id = self.id();
        self.emit(id);

        self.live_state.db.read_shopping_list().await
    }

    async fn share(&self) -> Result<String> {
        let doc = self.live_state.doc.clone();
        let id = self.id();

        let ticket = doc
            .share(
                iroh_docs::api::protocol::ShareMode::Write,
                iroh_docs::api::protocol::AddrInfoOptions::Relay,
            )
            .await?;
        let url = crate::link::format_sharing_link(&ticket);
        tell(
            &self.args.local_state,
            "mark shopping list shared",
            move |reply| LocalStateMsg::MarkShared(id, reply),
        );
        Ok(url)
    }

    async fn delete_self(&mut self) -> Result<()> {
        Ok(())
    }
}

impl ShouldMeasureMessage for ShoppingListMsg {
    fn should_measure(&self) -> bool {
        !matches!(self, Self::GetDebugStatus(_))
    }
}

#[ractor::actor(
    message = pub enum ShoppingListMsg,
    state = ShoppingListState,
    arguments = ShoppingListArgs,
)]
impl ShoppingListActor {
    async fn pre_start(
        &self,
        myself: ActorRef<ShoppingListMsg>,
        args: ShoppingListArgs,
    ) -> Result<ShoppingListState, ActorProcessingErr> {
        ractor::pg::join(SHOPPING_LIST_GROUP.to_string(), vec![myself.get_cell()]);

        let docs = args.api.docs.clone();
        let blobs = args.api.blobs.clone();
        let author = args.api.author;

        let (doc, ticket, was_new) = match args.ticket.clone() {
            Some(ticket) => {
                let (doc, stream) = docs.import_and_subscribe(ticket.clone()).await?;
                let stream = stream.map(|x| x.context("doc event stream"));
                forward_live_events(stream, myself.clone()).await?;
                (doc, ticket, false)
            }
            None => {
                if !args.allow_creation {
                    return Err(anyerr!(
                        "Somehow tried to create a new shopping list when not opening a shopping list for creation"
                    ).into());
                }

                let doc = docs.create().await?;
                let stream = doc.subscribe().await?;
                let stream = stream.map(|x| x.context("doc event stream"));

                forward_live_events(stream, myself.clone()).await?;
                let ticket = doc
                    .share(
                        iroh_docs::api::protocol::ShareMode::Write,
                        iroh_docs::api::protocol::AddrInfoOptions::RelayAndAddresses,
                    )
                    .await?;

                (doc, ticket, true)
            }
        };

        doc.start_sync(vec![]).await?;

        let id: ShoppingListId = doc.id().into();

        // Tell the local state to track the ticket. This is a bit funky as
        // we'll use this to open the document the next time the application
        // starts up.
        ractor::call!(args.local_state, |reply| {
            LocalStateMsg::PutTrackedShoppingList {
                id,
                ticket,
                was_new,
                reply,
            }
        })
        .flatten_call()?;

        let db = ShoppingListDB::new(doc.clone(), blobs, author);
        let live_state = LiveState { doc, db };

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

        Ok(ShoppingListState {
            args,
            live_state,
            stats: ActorStats::new("shopping-list"),
            debouncer,
            event_receiver,
        })
    }

    async fn post_start(
        &self,
        _myself: ActorRef<ShoppingListMsg>,
        state: &mut ShoppingListState,
    ) -> Result<(), ActorProcessingErr> {
        tracing::info!("Starting shopping list actor");
        let id = ShoppingListId::from(state.live_state.doc.id());

        state.emit(id);
        tracing::info!("Started shopping list actor {id}");
        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<ShoppingListMsg>,
        state: &mut ShoppingListState,
    ) -> Result<(), ActorProcessingErr> {
        let id = ShoppingListId::from(state.live_state.doc.id());

        tracing::info!("Shopping list actor {id} shutting down");
        if let Err(err) = state.live_state.doc.close().await {
            tracing::error!("Shopping list actor {id} failed to shut down cleanly: {err:?}");
        }
        Ok(())
    }

    #[ractor::rpc(GetId(reply))]
    fn get_id(&self, state: &ShoppingListState) -> ShoppingListId {
        state.id()
    }

    #[ractor::rpc(GetDebugStatus(reply))]
    async fn get_debug_status(
        &self,
        myself: ActorRef<ShoppingListMsg>,
        state: &ShoppingListState,
    ) -> Result<ShoppingListDebug> {
        let doc = state.live_state.doc.clone();
        let actor = state.stats.snapshot(&myself.get_cell());

        let mut entries = 0u64;
        let mut content_bytes = 0u64;
        let stream = doc
            .get_many(iroh_docs::store::Query::all().include_empty())
            .await?;
        let mut stream = std::pin::pin!(stream);
        while let Some(entry) = stream.next().await {
            let entry = entry?;
            entries += 1;
            content_bytes += entry.content_len();
        }

        let status = doc.status().await?;
        let sync_peers = doc.get_sync_peers().await?.map(|peers| peers.len());

        Ok(ShoppingListDebug {
            actor,
            id: doc.id().into(),
            entries,
            content_bytes,
            sync_peers,
            sync_enabled: status.sync,
            subscribers: status.subscribers,
            handles: status.handles,
        })
    }

    #[ractor::rpc(GetState(reply))]
    async fn get_state(&self, state: &ShoppingListState) -> Result<ShoppingList> {
        state.live_state.db.read_shopping_list().await
    }

    #[ractor::rpc(AddItem(item, reply))]
    #[tracing::instrument(skip(self, state))]
    async fn add_item(
        &self,
        item: ShoppingListItem,
        state: &ShoppingListState,
    ) -> Result<ShoppingList> {
        state
            .mutation(async move |db| db.insert_item(&item).await)
            .await
    }

    #[ractor::rpc(UpdateItem { id, name, reply })]
    #[tracing::instrument(skip(self, state))]
    async fn update_item(
        &self,
        id: ShoppingListItemId,
        name: String,
        state: &ShoppingListState,
    ) -> Result<ShoppingList> {
        state
            .mutation(async move |db| db.rename_item(id, &name).await)
            .await
    }

    #[ractor::rpc(SetIndex { id, new_index, reply })]
    #[tracing::instrument(skip(self, state))]
    async fn set_index(
        &self,
        id: ShoppingListItemId,
        new_index: ShoppingListItemIndex,
        state: &ShoppingListState,
    ) -> Result<ShoppingList> {
        state
            .mutation(async move |db| db.set_index(id, new_index).await)
            .await
    }

    #[ractor::rpc(SetCompleted { id, completed_at, reply })]
    #[tracing::instrument(skip(self, state))]
    async fn set_completed(
        &self,
        id: ShoppingListItemId,
        completed_at: Option<DateTime<Utc>>,
        state: &ShoppingListState,
    ) -> Result<ShoppingList> {
        state
            .mutation(async move |db| db.set_completed(id, completed_at).await)
            .await
    }

    #[ractor::rpc(DeleteItem(id, reply))]
    #[tracing::instrument(skip(self, state))]
    async fn delete_item(
        &self,
        id: ShoppingListItemId,
        state: &ShoppingListState,
    ) -> Result<ShoppingList> {
        state
            .mutation(async move |db| db.delete_item(id).await)
            .await
    }

    #[ractor::rpc(RestoreItems(items, reply))]
    #[tracing::instrument(skip(self, state))]
    async fn restore_items(
        &self,
        items: Vec<ShoppingListItem>,
        state: &ShoppingListState,
    ) -> Result<ShoppingList> {
        state
            .mutation(async move |db| db.restore_items(&items).await)
            .await
    }

    #[ractor::rpc(ClearCompleted(reply))]
    #[tracing::instrument(skip(self, state))]
    async fn clear_completed(&self, state: &ShoppingListState) -> Result<ShoppingList> {
        state
            .mutation(async move |db| db.clear_completed().await)
            .await
    }

    #[ractor::rpc(Rename(name, reply))]
    #[tracing::instrument(skip(self, state))]
    async fn rename(&self, name: String, state: &ShoppingListState) -> Result<ShoppingList> {
        state
            .mutation(async move |db| db.write_meta(&name).await)
            .await
    }

    #[ractor::rpc(Share(reply))]
    #[tracing::instrument(skip(self, state))]
    async fn share(&self, state: &ShoppingListState) -> Result<String> {
        state.share().await
    }

    #[ractor::rpc(DeleteSelf(reply))]
    #[tracing::instrument(skip(self, state))]
    async fn delete_self(
        &self,
        myself: ActorRef<ShoppingListMsg>,
        state: &mut ShoppingListState,
    ) -> Result<()> {
        let result = state.delete_self().await;
        myself.stop(None);
        result
    }

    #[ractor::rpc(SubscribeChanges(reply))]
    fn subscribe_changes(
        &self,
        state: &ShoppingListState,
    ) -> broadcast::Receiver<ShoppingListEvent> {
        state.event_receiver.resubscribe()
    }

    #[ractor::message(LiveEvent(event))]
    fn live_event(
        &self,
        event: Option<Result<LiveEvent>>,
        state: &ShoppingListState,
    ) -> Result<(), ActorProcessingErr> {
        let Some(event) = event else {
            tracing::debug!("Doc event stream ended");
            return Ok(());
        };
        event?;
        state.emit(state.id());
        Ok(())
    }

    #[ractor::supervision(SupervisionEvent::ActorTerminated(_, _, _))]
    fn pump_terminated(&self) {}

    #[ractor::supervision(SupervisionEvent::ActorFailed(who, error))]
    fn pump_failed(&self, who: ActorCell, error: ActorProcessingErr) {
        // We only receive events for the doc event forwarding sub-actor
        tracing::warn!("Doc event forwarding {} failed: {error:?}", who.get_id());
    }
}

/// Used to forward the iroh document event stream to the actor
async fn forward_live_events(
    stream: impl n0_future::Stream<Item = Result<LiveEvent>> + Send + 'static,
    myself: ActorRef<ShoppingListMsg>,
) -> Result<()> {
    spawn_stream_pump(stream, myself, ShoppingListMsg::LiveEvent, None)
        .await
        .map_err(|err| anyerr!("Could not forward doc events: {err}"))?;
    Ok(())
}
