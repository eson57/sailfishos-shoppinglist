use std::path::PathBuf;

use indexmap::IndexMap;
use iroh_docs::DocTicket;
use n0_error::{Result, StdResultExt as _};
use ractor::{ActorProcessingErr, ActorRef};

use crate::actor::{ActorStats, ActorStatsSnapshot, HasStats, ShouldMeasureMessage};
use crate::models::{self, GlobalState, ShoppingListId, TrackedShoppingList, TrackedShoppingLists};

const CONFIG_FILE: &str = "local-config.ron";

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AppState {
    global_state: models::GlobalState,
    tracked_shopping_lists: models::TrackedShoppingLists,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            tracked_shopping_lists: models::TrackedShoppingLists {
                shopping_lists: IndexMap::new(),
            },
            global_state: models::GlobalState {},
        }
    }
}

pub struct LocalState {
    app: AppState,
    config_dir: Option<PathBuf>,
    stats: ActorStats,
}

impl LocalState {
    fn persist(&self) -> Result<()> {
        let Some(config_dir) = &self.config_dir else {
            return Ok(());
        };
        confy::store_path(config_dir.join(CONFIG_FILE), &self.app).anyerr()
    }
}

impl HasStats for LocalState {
    fn stats_mut(&mut self) -> &mut ActorStats {
        &mut self.stats
    }
}

/// Actor which handles reading and writing the local config.
pub struct LocalStateActor;

pub struct LocalStateArgs {
    app: AppState,
    config_dir: Option<PathBuf>,
}

pub fn format_ron_error(input: &str, err: &ron::error::SpannedError) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let start = err.span.start;
    let end = err.span.end;
    let lines: Vec<&str> = input.lines().collect();
    let gutter = end.line.to_string().len().max(4);

    let _ = writeln!(
        out,
        "RON error at {}:{}: {}",
        start.line, start.col, err.code
    );

    for line_num in start.line..=end.line {
        let Some(&line_text) = lines.get(line_num.saturating_sub(1)) else {
            break;
        };

        let line_chars = line_text.chars().count();
        let col_start = if line_num == start.line {
            start.col.saturating_sub(1).min(line_chars)
        } else {
            0
        };
        let col_end = if line_num == end.line {
            end.col.saturating_sub(1).min(line_chars)
        } else {
            line_chars
        };

        let caret_len = col_end.saturating_sub(col_start).max(1);
        let padding: String = line_text
            .chars()
            .take(col_start)
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .collect();

        let _ = writeln!(out, "{:>width$} | {}", line_num, line_text, width = gutter);
        let _ = writeln!(
            out,
            "{:>width$} | {}{}",
            "",
            padding,
            "^".repeat(caret_len),
            width = gutter
        );
    }

    out
}

impl LocalStateArgs {
    pub fn load(config_dir: Option<PathBuf>) -> Result<Self> {
        let app = match &config_dir {
            Some(dir) => {
                tracing::info!("Using {dir:?} as local config directory");
                confy::load_path::<AppState>(dir.join(CONFIG_FILE)).map_err(|e| match e {
                    confy::ConfyError::BadRonData(ron_err) => {
                        let file = std::fs::read_to_string(dir.join(CONFIG_FILE)).unwrap();
                        let err_msg = format_ron_error(&file, &ron_err);

                        n0_error::anyerr!(ron_err).context(err_msg)
                    }
                    e => n0_error::anyerr!(e, "Failed to load config"),
                })?
            }
            None => AppState::default(),
        };
        Ok(Self { app, config_dir })
    }
}

impl ShouldMeasureMessage for LocalStateMsg {
    fn should_measure(&self) -> bool {
        !matches!(self, Self::GetDebugStatus(_))
    }
}

#[ractor::actor(message = pub enum LocalStateMsg, state = LocalState, arguments = LocalStateArgs)]
impl LocalStateActor {
    async fn pre_start(
        &self,
        _myself: ActorRef<LocalStateMsg>,
        args: LocalStateArgs,
    ) -> Result<LocalState, ActorProcessingErr> {
        Ok(LocalState {
            app: args.app,
            config_dir: args.config_dir,
            stats: ActorStats::new("local-state"),
        })
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<LocalStateMsg>,
        state: &mut LocalState,
    ) -> Result<(), ActorProcessingErr> {
        if let Err(err) = state.persist() {
            tracing::warn!("Could not store local shopping lists: {err}");
        }
        Ok(())
    }

    #[ractor::rpc(GetDebugStatus(reply))]
    fn get_debug_status(
        &self,
        myself: ActorRef<LocalStateMsg>,
        state: &LocalState,
    ) -> Result<ActorStatsSnapshot> {
        Ok(state.stats.snapshot(&myself.get_cell()))
    }

    #[ractor::rpc(GetGlobalState(reply))]
    fn get_global_state(&self, state: &LocalState) -> Result<GlobalState> {
        Ok(state.app.global_state.clone())
    }

    #[ractor::rpc(GetTrackedShoppingLists(reply))]
    fn get_tracked_shopping_lists(&self, state: &LocalState) -> Result<TrackedShoppingLists> {
        Ok(state.app.tracked_shopping_lists.clone())
    }

    #[ractor::rpc(PutTrackedShoppingList { id, ticket, was_new, reply })]
    fn put_tracked_shopping_list(
        &self,
        id: ShoppingListId,
        ticket: DocTicket,
        was_new: bool,
        state: &mut LocalState,
    ) -> Result<()> {
        if let Some(existing) = state.app.tracked_shopping_lists.shopping_lists.get_mut(&id) {
            existing.ticket = ticket;
        } else {
            state.app.tracked_shopping_lists.shopping_lists.insert(
                id,
                TrackedShoppingList {
                    id,
                    ticket,
                    shared: was_new,
                },
            );
        }
        state.persist()
    }

    #[ractor::rpc(MarkShared(id, reply))]
    fn mark_shared(&self, id: ShoppingListId, state: &mut LocalState) -> Result<()> {
        if let Some(existing) = state.app.tracked_shopping_lists.shopping_lists.get_mut(&id) {
            existing.shared = true;
        }
        state.persist()
    }

    #[ractor::rpc(RemoveTrackedShoppingList(id, reply))]
    fn remove_tracked_shopping_list(
        &self,
        id: ShoppingListId,
        state: &mut LocalState,
    ) -> Result<()> {
        tracing::debug!("Removing local tracking state for: {id:?}");
        state
            .app
            .tracked_shopping_lists
            .shopping_lists
            .shift_remove(&id);
        state.persist()
    }
}
