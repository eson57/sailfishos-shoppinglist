use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use actor_macro::{active_model, with_executor};
use chrono::{DateTime, Local};
use cxx_qt::CxxQtType;
use ractor::{ActorRef, call};

use shoppinglist_core::{
    actor::{ActorStatsSnapshot, FlattenCall},
    mesh::{
        debug::{DebugReport, NetworkDebug, ShoppingListDebug},
        synced_state::SyncedStateMsg,
    },
};

use crate::{
    actor::{ActorWiringModel, Actors, ModelRef},
    logging::{self, LogLine, LogSubscription},
};

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
        #[qproperty(*mut StatusRowsModel, status_rows, cxx_name = "statusRows", READ = get_status_rows, CONSTANT)]
        #[qproperty(*mut LogEntriesModel, log_entries, cxx_name = "logEntries", READ = get_log_entries, CONSTANT)]
        type DebugStatusModel = super::DebugStatusModelRust;

        fn get_registry(self: &DebugStatusModel) -> *mut ActorRegistry;
        unsafe fn set_registry(self: Pin<&mut DebugStatusModel>, registry: *mut ActorRegistry);
        fn get_ready(self: &DebugStatusModel) -> bool;
        fn get_status_rows(self: &DebugStatusModel) -> *mut StatusRowsModel;
        fn get_log_entries(self: &DebugStatusModel) -> *mut LogEntriesModel;
    }

    #[qenum(StatusRowsModel)]
    enum StatusRoles {
        IsHeader,
        Label,
        Value,
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qproperty(i32, count, READ, NOTIFY)]
        type StatusRowsModel = super::StatusRowsModelRust;

        #[cxx_override]
        fn data(self: &StatusRowsModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &StatusRowsModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &StatusRowsModel, parent: &QModelIndex) -> i32;
        #[cxx_override]
        #[cxx_name = "removeRows"]
        fn qt_remove_rows(
            self: Pin<&mut StatusRowsModel>,
            row: i32,
            count: i32,
            parent: &QModelIndex,
        ) -> bool;
        #[cxx_override]
        #[cxx_name = "moveRows"]
        fn qt_move_rows(
            self: Pin<&mut StatusRowsModel>,
            source_parent: &QModelIndex,
            source_row: i32,
            count: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;

        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut StatusRowsModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut StatusRowsModel>);
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(
            self: Pin<&mut StatusRowsModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut StatusRowsModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        fn begin_remove_rows(
            self: Pin<&mut StatusRowsModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        fn end_remove_rows(self: Pin<&mut StatusRowsModel>);
        #[inherit]
        #[cxx_name = "beginMoveRows"]
        fn begin_move_rows(
            self: Pin<&mut StatusRowsModel>,
            source_parent: &QModelIndex,
            source_first: i32,
            source_last: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;
        #[inherit]
        #[cxx_name = "endMoveRows"]
        fn end_move_rows(self: Pin<&mut StatusRowsModel>);
        #[inherit]
        fn index(
            self: &StatusRowsModel,
            row: i32,
            column: i32,
            parent: &QModelIndex,
        ) -> QModelIndex;
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut StatusRowsModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
    }

    #[qenum(LogEntriesModel)]
    enum LogRoles {
        Level,
        Text,
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qproperty(i32, count, READ, NOTIFY)]
        type LogEntriesModel = super::LogEntriesModelRust;

        #[cxx_override]
        fn data(self: &LogEntriesModel, index: &QModelIndex, role: i32) -> QVariant;
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &LogEntriesModel) -> QHash_i32_QByteArray;
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &LogEntriesModel, parent: &QModelIndex) -> i32;
        #[cxx_override]
        #[cxx_name = "removeRows"]
        fn qt_remove_rows(
            self: Pin<&mut LogEntriesModel>,
            row: i32,
            count: i32,
            parent: &QModelIndex,
        ) -> bool;
        #[cxx_override]
        #[cxx_name = "moveRows"]
        fn qt_move_rows(
            self: Pin<&mut LogEntriesModel>,
            source_parent: &QModelIndex,
            source_row: i32,
            count: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;

        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut LogEntriesModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut LogEntriesModel>);
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(
            self: Pin<&mut LogEntriesModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut LogEntriesModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        fn begin_remove_rows(
            self: Pin<&mut LogEntriesModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        fn end_remove_rows(self: Pin<&mut LogEntriesModel>);
        #[inherit]
        #[cxx_name = "beginMoveRows"]
        fn begin_move_rows(
            self: Pin<&mut LogEntriesModel>,
            source_parent: &QModelIndex,
            source_first: i32,
            source_last: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;
        #[inherit]
        #[cxx_name = "endMoveRows"]
        fn end_move_rows(self: Pin<&mut LogEntriesModel>);
        #[inherit]
        fn index(
            self: &LogEntriesModel,
            row: i32,
            column: i32,
            parent: &QModelIndex,
        ) -> QModelIndex;
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut LogEntriesModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
    }

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn new_status_rows_model() -> UniquePtr<StatusRowsModel>;
        #[cxx_name = "make_unique"]
        fn new_log_entries_model() -> UniquePtr<LogEntriesModel>;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qinvokable]
        fn refresh(self: Pin<&mut DebugStatusModel>);
    }
}

pub use qobject::{DebugStatusModel, LogEntriesModel, LogRoles, StatusRoles, StatusRowsModel};

#[derive(Clone, PartialEq, Eq)]
struct StatusRow {
    is_header: bool,
    label: String,
    value: String,
}

impl StatusRow {
    fn header(label: impl Into<String>) -> Self {
        Self {
            is_header: true,
            label: label.into(),
            value: String::new(),
        }
    }

    fn item(label: &str, value: impl ToString) -> Self {
        Self {
            is_header: false,
            label: label.to_owned(),
            value: value.to_string(),
        }
    }
}

list_model! {
    model: StatusRowsModel,
    rust: StatusRowsModelRust,
    roles: StatusRoles,
    row: StatusRow,
    new: qobject::new_status_rows_model,
    owned: OwnedStatusRows,
    columns: [
        (IsHeader, "isHeader", is_header),
        (Label, "label", label),
        (Value, "value", value),
    ],
}

#[derive(Clone, PartialEq, Eq)]
struct LogRow {
    level: String,
    text: String,
}

impl From<&LogLine> for LogRow {
    fn from(line: &LogLine) -> Self {
        Self {
            level: line.level.as_str().to_owned(),
            text: line.text.clone(),
        }
    }
}

list_model! {
    model: LogEntriesModel,
    rust: LogEntriesModelRust,
    roles: LogRoles,
    row: LogRow,
    new: qobject::new_log_entries_model,
    owned: OwnedLogRows,
    columns: [
        (Level, "level", level),
        (Text, "text", text),
    ],
}

#[active_model(qobject::DebugStatusModel)]
#[derive(Default)]
pub struct DebugStatusModelRust {
    synced: Option<ActorRef<SyncedStateMsg>>,
    refreshing: bool,

    status_rows: OwnedStatusRows,
    log_entries: OwnedLogRows,
}

impl DebugStatusModel {
    fn get_status_rows(&self) -> *mut StatusRowsModel {
        self.status_rows.as_ptr()
    }

    fn get_log_entries(&self) -> *mut LogEntriesModel {
        self.log_entries.as_ptr()
    }

    #[with_executor]
    fn refresh(mut self: Pin<&mut Self>) {
        let Some(synced) = self.synced.clone() else {
            return;
        };
        if self.refreshing {
            return;
        }
        self.as_mut().rust_mut().refreshing = true;

        let model = self.model_ref();
        tokio::task::spawn_local(async move {
            let report = call!(synced, SyncedStateMsg::GetDebugReport).flatten_call();
            model.with(|model| model.finish_refresh(report));
        });
    }

    fn finish_refresh(mut self: Pin<&mut Self>, report: n0_error::Result<DebugReport>) {
        self.as_mut().rust_mut().refreshing = false;

        match report {
            Ok(report) => self
                .as_mut()
                .rust_mut()
                .status_rows
                .reset_rows(rows_from_report(&report)),
            Err(err) => tracing::warn!(
                target: logging::UNCOLLECTED,
                "Could not refresh debug status: {err:?}"
            ),
        }
    }
}

impl ActorWiringModel for DebugStatusModel {
    fn connect_actors(mut self: Pin<&mut Self>, actors: Arc<Actors>) {
        let mut this = self.as_mut().rust_mut();
        this.synced = Some(actors.synced_state.clone());

        tokio::task::spawn_local(follow_logs(self.model_ref()));
        self.refresh();
    }
}

/// This receives the last N logs from the subscriber and then receives each new
/// log after.
async fn follow_logs(model: ModelRef<DebugStatusModel>) {
    let LogSubscription {
        history,
        mut new_lines,
    } = logging::buffer().subscribe();

    let mut batch = history;
    loop {
        let rows: Vec<LogRow> = batch.iter().map(LogRow::from).collect();
        if model
            .with(|model| model.rust_mut().log_entries.prepend_rows(rows))
            .is_none()
        {
            return;
        }

        let Some(first) = new_lines.recv().await else {
            return;
        };

        batch = vec![first];

        while let Ok(line) = new_lines.try_recv() {
            batch.push(line);
        }
    }
}

fn rows_from_report(report: &DebugReport) -> Vec<StatusRow> {
    let mut rows = Vec::new();

    rows.push(StatusRow::header("Local state"));
    push_actor(&mut rows, &report.local_state);

    rows.push(StatusRow::header("Synced state"));
    push_actor(&mut rows, &report.synced_state);

    rows.push(StatusRow::header("Network"));
    push_network(&mut rows, &report.network);

    for shopping_list in &report.shopping_lists {
        push_shopping_list(&mut rows, shopping_list);
    }
    rows
}

fn push_actor(rows: &mut Vec<StatusRow>, actor: &ActorStatsSnapshot) {
    rows.push(StatusRow::item("Status", format!("{:?}", actor.status)));
    rows.push(StatusRow::item(
        "Started",
        DateTime::<Local>::from(actor.started_at).format("%H:%M:%S"),
    ));
    rows.push(StatusRow::item("Uptime", format_duration(actor.uptime)));
    rows.push(StatusRow::item(
        "Last message",
        actor
            .since_last_message
            .map_or("never".to_owned(), |since| {
                format!("{} ago", format_duration(since))
            }),
    ));
    rows.push(StatusRow::item("Messages handled", actor.messages));
}

fn push_network(rows: &mut Vec<StatusRow>, network: &NetworkDebug) {
    rows.push(StatusRow::item(
        "Open connections",
        network.connections_open,
    ));
    rows.push(StatusRow::item(
        "Connections opened",
        network.connections_opened_total,
    ));
    rows.push(StatusRow::item(
        "Connections closed",
        network.connections_closed_total,
    ));
    rows.push(StatusRow::item(
        "Direct connections",
        network.connections_direct_total,
    ));
    rows.push(StatusRow::item(
        "Failed relay connections",
        network.relay_connections_failed,
    ));
    rows.push(StatusRow::item(
        "Gossip sent",
        format!(
            "{} messages, {}",
            network.gossip_msgs_sent,
            format_bytes(network.gossip_bytes_sent)
        ),
    ));
    rows.push(StatusRow::item(
        "Gossip received",
        format!(
            "{} messages, {}",
            network.gossip_msgs_recv,
            format_bytes(network.gossip_bytes_recv)
        ),
    ));
}

fn push_shopping_list(rows: &mut Vec<StatusRow>, shopping_list: &ShoppingListDebug) {
    rows.push(StatusRow::header(format!(
        "Shopping list {}",
        shopping_list.id
    )));
    push_actor(rows, &shopping_list.actor);
    rows.push(StatusRow::item("Entries", shopping_list.entries));
    rows.push(StatusRow::item(
        "Content size",
        format_bytes(shopping_list.content_bytes),
    ));
    rows.push(StatusRow::item(
        "Sync peers",
        shopping_list
            .sync_peers
            .map_or("not syncing".to_owned(), |peers| peers.to_string()),
    ));
    rows.push(StatusRow::item("Sync enabled", shopping_list.sync_enabled));
    rows.push(StatusRow::item("Subscribers", shopping_list.subscribers));
    rows.push(StatusRow::item("Handles", shopping_list.handles));
}

fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    match secs {
        0 => format!("{} ms", duration.as_millis()),
        1..=59 => format!("{secs} s"),
        60..=3599 => format!("{} m {} s", secs / 60, secs % 60),
        _ => format!("{} h {} m", secs / 3600, secs % 3600 / 60),
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
