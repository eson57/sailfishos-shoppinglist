use crate::{actor::ActorStatsSnapshot, models::ShoppingListId};

#[derive(Debug, Clone, Default)]
pub struct NetworkDebug {
    pub connections_open: u64,
    pub connections_opened_total: u64,
    pub connections_closed_total: u64,
    pub connections_direct_total: u64,
    pub relay_connections_failed: u64,
    pub gossip_msgs_sent: u64,
    pub gossip_msgs_recv: u64,
    pub gossip_bytes_sent: u64,
    pub gossip_bytes_recv: u64,
}

#[derive(Debug, Clone)]
pub struct ShoppingListDebug {
    pub actor: ActorStatsSnapshot,
    pub id: ShoppingListId,
    pub entries: u64,
    pub content_bytes: u64,
    pub sync_peers: Option<usize>,
    pub sync_enabled: bool,
    pub subscribers: usize,
    pub handles: usize,
}

#[derive(Debug, Clone)]
pub struct DebugReport {
    pub local_state: ActorStatsSnapshot,
    pub synced_state: ActorStatsSnapshot,
    pub network: NetworkDebug,
    pub shopping_lists: Vec<ShoppingListDebug>,
}
