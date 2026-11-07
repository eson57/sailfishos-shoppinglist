use std::pin::Pin;
use std::sync::Arc;

use actor_macro::{active_model, with_executor};
use chrono::DateTime;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use itertools::Itertools;
use n0_error::{AnyError, StdResultExt as _};
use ractor::{ActorRef, call};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use shoppinglist_core::{
    actor::{FlattenCall, tell},
    link::parse_sharing_link,
    mesh::{shoppinglist::ShoppingListMsg, synced_state::SyncedStateMsg},
    models::{ShoppingListId, ShoppingListWithId},
};

use crate::actor::{ActorWiringModel, Actors, ModelRef};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;

        include!("sailfishos-shoppinglist/src/actor/registry.cxxqt.h");
        type ActorRegistry = crate::actor::ActorRegistry;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(*mut ActorRegistry, registry, READ = get_registry, WRITE = set_registry)]
        #[qproperty(bool, ready, READ = get_ready, NOTIFY)]
        #[qproperty(*mut HomeListModel, items, READ = get_items, CONSTANT)]
        type ShoppingListsModel = super::ShoppingListsModelRust;

        fn get_registry(self: &ShoppingListsModel) -> *mut ActorRegistry;
        unsafe fn set_registry(self: Pin<&mut ShoppingListsModel>, registry: *mut ActorRegistry);
        fn get_ready(self: &ShoppingListsModel) -> bool;
        fn get_items(self: &ShoppingListsModel) -> *mut HomeListModel;
    }

    #[qenum(HomeListModel)]
    enum HomeRoles {
        ShoppingListId,
        ShoppingListName,
        UpdatedAt,
        Preview,
    }

    unsafe extern "RustQt" {
        /// The rows shown on the home page.
        #[qobject]
        #[base = QAbstractListModel]
        #[qproperty(i32, count, READ, NOTIFY)]
        type HomeListModel = super::HomeListModelRust;

        #[cxx_override]
        fn data(self: &HomeListModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &HomeListModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &HomeListModel, parent: &QModelIndex) -> i32;
        #[cxx_override]
        #[cxx_name = "removeRows"]
        fn qt_remove_rows(
            self: Pin<&mut HomeListModel>,
            row: i32,
            count: i32,
            parent: &QModelIndex,
        ) -> bool;
        #[cxx_override]
        #[cxx_name = "moveRows"]
        fn qt_move_rows(
            self: Pin<&mut HomeListModel>,
            source_parent: &QModelIndex,
            source_row: i32,
            count: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;

        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut HomeListModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut HomeListModel>);
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(
            self: Pin<&mut HomeListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut HomeListModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        fn begin_remove_rows(
            self: Pin<&mut HomeListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        fn end_remove_rows(self: Pin<&mut HomeListModel>);
        #[inherit]
        #[cxx_name = "beginMoveRows"]
        fn begin_move_rows(
            self: Pin<&mut HomeListModel>,
            source_parent: &QModelIndex,
            source_first: i32,
            source_last: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;
        #[inherit]
        #[cxx_name = "endMoveRows"]
        fn end_move_rows(self: Pin<&mut HomeListModel>);
        #[inherit]
        fn index(self: &HomeListModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut HomeListModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
    }

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn new_home_list_model() -> UniquePtr<HomeListModel>;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qinvokable]
        fn create_shopping_list(self: Pin<&mut ShoppingListsModel>, name: &QString);

        #[qinvokable]
        fn remove_shopping_list(self: Pin<&mut ShoppingListsModel>, id: &QString);

        #[qinvokable]
        fn import_link(self: Pin<&mut ShoppingListsModel>, text: &QString);

        #[qsignal]
        fn open(self: Pin<&mut ShoppingListsModel>, shopping_list_id: QString);
    }
}

pub use qobject::{HomeListModel, HomeRoles, ShoppingListsModel};

#[derive(Clone, PartialEq, Eq)]
struct HomeRow {
    shopping_list_id: String,
    shopping_list_name: String,
    updated_at: DateTime<chrono::Local>,
    preview: String,
}

list_model! {
    model: HomeListModel,
    rust: HomeListModelRust,
    roles: HomeRoles,
    row: HomeRow,
    new: qobject::new_home_list_model,
    owned: OwnedHomeList,
    columns: [
        (ShoppingListId, "shoppingListId", shopping_list_id),
        (ShoppingListName, "shoppingListName", shopping_list_name),
        (UpdatedAt, "updatedAt", updated_at),
        (Preview, "preview", preview),
    ],
}

fn shopping_list_items_preview(list: &ShoppingListWithId) -> String {
    list.inner
        .items
        .values()
        .filter(|item| item.completed_at.is_none())
        .k_smallest_by_key(8, |x| x.incomplete_item_order_key())
        .map(|item| item.name.as_str())
        .join(", ")
}

fn shopping_list_latest_update(list: &ShoppingListWithId) -> DateTime<chrono::Utc> {
    list.inner
        .items
        .values()
        .filter_map(|item| {
            item.completed_at
                .iter()
                .copied()
                .chain(core::iter::once(item.created_at))
                .max()
        })
        .max()
        .unwrap_or(list.inner.created_at)
}

impl From<&ShoppingListWithId> for HomeRow {
    fn from(summary: &ShoppingListWithId) -> Self {
        Self {
            shopping_list_id: serde_json::to_string(&summary.id).unwrap(),
            shopping_list_name: summary.inner.name.clone(),
            updated_at: shopping_list_latest_update(summary).with_timezone(&chrono::Local),
            preview: shopping_list_items_preview(summary),
        }
    }
}

#[active_model(qobject::ShoppingListsModel)]
#[derive(Default)]
pub struct ShoppingListsModelRust {
    synced: Option<ActorRef<SyncedStateMsg>>,

    summaries: Vec<ShoppingListWithId>,

    items: OwnedHomeList,
}

impl ShoppingListsModel {
    fn get_items(&self) -> *mut HomeListModel {
        self.items.as_ptr()
    }

    fn synced(&self) -> ActorRef<SyncedStateMsg> {
        self.synced
            .clone()
            .expect("Synced state not connected to shopping lists model")
    }

    fn refresh(&self) {
        tokio::task::spawn_local(refresh(self.model_ref(), self.synced()));
    }

    fn set_summaries(self: Pin<&mut Self>, summaries: Vec<ShoppingListWithId>) {
        if summaries == self.summaries {
            return;
        }

        let mut this = self.rust_mut();
        this.summaries = summaries;
        let new_items: Vec<_> = this.summaries.iter().map(HomeRow::from).collect();
        this.items
            .patch_rows(new_items, |row| row.shopping_list_id.clone());
    }

    #[with_executor]
    fn create_shopping_list(self: Pin<&mut Self>, name: &QString) {
        let model = self.model_ref();
        let synced = self.synced();
        let name = name.to_string();

        tokio::task::spawn_local(async move {
            let result: Result<_, AnyError> = async {
                let shopping_list =
                    call!(synced, SyncedStateMsg::CreateShoppingList, name).flatten_call()?;
                call!(shopping_list, ShoppingListMsg::GetId).anyerr()
            }
            .await;

            match result {
                Ok(id) => {
                    model.with(|model| model.finish_open(id));
                }
                Err(err) => tracing::warn!("Failed to create shopping list: {err:?}"),
            }
        });
    }

    #[with_executor]
    fn remove_shopping_list(self: Pin<&mut Self>, id: &QString) {
        let id = match serde_json::from_str(id.to_string().trim()) {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!("Failed to decode shopping list id: {err:?}");
                return;
            }
        };
        tell(&self.synced(), "Removing shopping list", move |reply| {
            SyncedStateMsg::RemoveShoppingList(id, reply)
        });
    }

    #[with_executor]
    fn import_link(self: Pin<&mut Self>, text: &QString) {
        tracing::info!("Importing link: {text}");
        let ticket = match parse_sharing_link(text.to_string().trim()) {
            Ok(ticket) => ticket,
            Err(err) => {
                tracing::warn!("Failed to decode ticket: {err:?}");
                return;
            }
        };

        let model = self.model_ref();
        let synced = self.synced();

        tokio::task::spawn_local(async move {
            let id = ShoppingListId {
                id: ticket.capability.id(),
            };
            let result = call!(synced, |reply| SyncedStateMsg::OpenShoppingList {
                id,
                ticket: Some(ticket),
                reply,
            })
            .flatten_call();

            match result {
                Ok(_shopping_list) => {
                    model.with(|model| model.finish_open(id));
                }
                Err(err) => tracing::warn!("Failed to import shopping list: {err:?}"),
            }
        });
    }

    fn finish_open(mut self: Pin<&mut Self>, id: ShoppingListId) {
        let id_as_string = serde_json::to_string(&id).unwrap();
        self.as_mut().open(QString::from(&id_as_string));
        self.refresh();
    }
}

impl ActorWiringModel for ShoppingListsModel {
    fn connect_actors(mut self: Pin<&mut Self>, actors: Arc<Actors>) {
        let synced = actors.synced_state.clone();
        self.as_mut().rust_mut().synced = Some(synced.clone());

        tokio::task::spawn_local(follow_changes(self.model_ref(), synced));
        self.refresh();
    }
}

async fn follow_changes(model: ModelRef<ShoppingListsModel>, synced: ActorRef<SyncedStateMsg>) {
    let changes = match call!(synced, SyncedStateMsg::SubscribeChanges) {
        Ok(changes) => changes,
        Err(err) => {
            tracing::warn!("Failed to subscribe to synced state: {err:?}");
            return;
        }
    };

    let mut events = BroadcastStream::new(changes).filter_map(|x| x.ok());
    while let Some(event) = events.next().await {
        tracing::info!("received sync state event: {event:?}");

        if model.with(|_| ()).is_none() {
            tracing::debug!("Model got dropped, no longer following changes.");
            return;
        }
        refresh(model.clone(), synced.clone()).await;
    }
}

async fn refresh(model: ModelRef<ShoppingListsModel>, synced: ActorRef<SyncedStateMsg>) {
    tracing::debug!("refreshing");

    match call!(synced, SyncedStateMsg::ListShoppingLists).flatten_call() {
        Ok(summaries) => {
            model.with(|model| model.set_summaries(summaries));
        }
        Err(err) => tracing::warn!("Failed to refresh homepage for some reason: {err:?}"),
    }
}
