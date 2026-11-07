use chrono::{DateTime, Utc};
use fracindex::Fracindex;
use iroh_docs::{DocTicket, NamespaceId};
use serde_with::DisplayFromStr;
use smallvec::SmallVec;
use uuid::Uuid;

use crate::mesh::shoppinglist::store::ShoppingList;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct GlobalState {}

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ShoppingListId {
    pub id: NamespaceId,
}

impl From<NamespaceId> for ShoppingListId {
    fn from(id: NamespaceId) -> Self {
        Self { id }
    }
}

impl std::fmt::Debug for ShoppingListId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

impl std::fmt::Display for ShoppingListId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

#[serde_with::serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct TrackedShoppingList {
    pub id: ShoppingListId,
    #[serde_as(as = DisplayFromStr)]
    pub ticket: DocTicket,
    pub shared: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct TrackedShoppingLists {
    pub shopping_lists: indexmap::IndexMap<ShoppingListId, TrackedShoppingList>,
}

#[derive(
    serde::Serialize,
    serde::Deserialize,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    bilrost::Message,
)]
pub struct ShoppingListItemId {
    high: u64,
    low: u64,
}

impl std::fmt::Debug for ShoppingListItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id().fmt(f)
    }
}

impl ShoppingListItemId {
    pub fn new(id: Uuid) -> Self {
        let full = id.as_u128();
        let high = (full >> 64) as u64;
        let low = full as u64;
        Self { high, low }
    }

    pub fn id(&self) -> Uuid {
        let full = ((self.high as u128) << 64) | (self.low as u128);
        Uuid::from_u128(full)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, bilrost::Message)]
pub struct ShoppingListItemIndex {
    #[bilrost(encoding(plainbytes))]
    inner: SmallVec<[u8; 16]>,
}

impl std::fmt::Debug for ShoppingListItemIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.index().fmt(f)
    }
}

impl ShoppingListItemIndex {
    pub fn new(index: Fracindex) -> Self {
        // Pray for me that rustc elides the allocation?
        Self {
            inner: index.to_bytes().into(),
        }
    }

    pub fn index(&self) -> Fracindex {
        Fracindex::from_bytes(&self.inner).expect("Shouldn't be possible, we should only be calling Fracindex::from_bytes on buffers created with FracIndex::to_bytes")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, bilrost::Message)]
pub struct ShoppingListItem {
    #[bilrost(0)]
    pub id: ShoppingListItemId,
    #[bilrost(4)]
    pub index: ShoppingListItemIndex,
    #[bilrost(1)]
    pub name: String,
    #[bilrost(2)]
    pub created_at: DateTime<Utc>,
    #[bilrost(3)]
    pub completed_at: Option<DateTime<Utc>>,
}

impl ShoppingListItem {
    pub fn incomplete_item_order_key(&self) -> impl Ord + use<> {
        (self.index.clone(), std::cmp::Reverse(self.created_at))
    }

    pub fn completed_item_order_key(&self) -> impl Ord + use<> {
        std::cmp::Reverse(self.completed_at)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShoppingListWithId {
    pub id: ShoppingListId,
    pub inner: ShoppingList,
}
