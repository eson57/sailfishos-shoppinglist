use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use iroh_docs::store::Query;
use n0_error::{Result, StackResultExt, StdResultExt as _};
use n0_future::{BufferedStreamExt, StreamExt};

use crate::models::{ShoppingListItem, ShoppingListItemId, ShoppingListItemIndex};

const META_PREFIX: &str = "org.simmsb.shoppinglist/meta/";
const ITEM_PREFIX: &str = "org.simmsb.shoppinglist/item/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShoppingList {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub items: BTreeMap<ShoppingListItemId, ShoppingListItem>,
}

#[derive(Clone)]
pub struct ShoppingListDB {
    doc: iroh_docs::api::Doc,
    blobs: iroh_blobs::api::Store,
    author: iroh_docs::AuthorId,
}

impl ShoppingListDB {
    pub fn new(
        doc: iroh_docs::api::Doc,
        blobs: iroh_blobs::api::Store,
        author: iroh_docs::AuthorId,
    ) -> Self {
        Self { doc, blobs, author }
    }

    async fn delete_value<K: serde::Serialize>(&self, key_prefix: &str, key: &K) -> Result<()> {
        let key_buf: Vec<u8> = key_prefix.to_string().into_bytes();
        let key_buf = postcard::to_extend(key, key_buf).anyerr()?;

        self.doc.del(self.author, key_buf).await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn write_value<
        K: serde::Serialize + std::fmt::Debug,
        V: bilrost::Message + std::fmt::Debug,
    >(
        &self,
        key_prefix: &str,
        key: &K,
        value: &V,
    ) -> Result<()> {
        let key_buf: Vec<u8> = key_prefix.to_string().into_bytes();
        let key_buf = postcard::to_extend(key, key_buf).anyerr()?;

        let value = value.encode_to_vec();

        tracing::trace!("Writing value: {value:?}");

        self.doc.set_bytes(self.author, key_buf, value).await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn read_value<K: serde::Serialize + std::fmt::Debug, V: bilrost::OwnedMessage>(
        &self,
        key_prefix: &str,
        key: &K,
    ) -> Result<Option<V>> {
        let key_buf: Vec<u8> = key_prefix.to_string().into_bytes();
        let key_buf = postcard::to_extend(key, key_buf).anyerr()?;

        let q = Query::single_latest_per_key()
            .key_exact(key_buf)
            .include_empty();

        let Some(entry) = self.doc.get_one(q).await? else {
            return Ok(None);
        };
        entry.validate_empty().anyerr()?;

        if entry.content_len() == 0 {
            return Ok(None);
        }

        let data = self.blobs.get_bytes(entry.content_hash()).await.anyerr()?;

        tracing::trace!("Read value: {data:?}");

        Ok(Some(V::decode(data).anyerr()?))
    }

    async fn read_prefixed_values<V: bilrost::OwnedMessage>(
        &self,
        key_prefix: &str,
    ) -> Result<Vec<V>> {
        let q = Query::single_latest_per_key()
            .key_prefix(key_prefix.as_bytes())
            .include_empty();

        self.doc
            .get_many(q)
            .await?
            .map(async |entry| {
                let entry = entry?;

                entry.validate_empty().anyerr()?;

                if entry.content_len() == 0 {
                    return Ok(None);
                }

                // TODO: I think it's possible for us to be told about the docs
                // entry before the blob becomes available.
                //
                // Add a retry here maybe
                //
                // Seems that it's sort of okay, the actor will probably just retry
                let bytes = self.blobs.get_bytes(entry.content_hash()).await.anyerr()?;

                V::decode(bytes).anyerr().map(Some)
            })
            .buffered_unordered(10)
            .filter_map(|x| x.transpose())
            .try_collect()
            .await
    }

    pub async fn read_shopping_list(&self) -> Result<ShoppingList> {
        self.read_shoppinglist().await
    }

    pub async fn rename_item(&self, id: ShoppingListItemId, name: &str) -> Result<()> {
        let mut item = self.read_item(id).await?.context("Item doesn't exist")?;
        item.name = name.to_owned();

        self.insert_item(&item).await?;

        Ok(())
    }

    pub async fn set_completed(
        &self,
        id: ShoppingListItemId,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let mut item = self.read_item(id).await?.context("Item doesn't exist")?;
        item.completed_at = completed_at;

        self.insert_item(&item).await?;

        Ok(())
    }

    pub async fn set_index(
        &self,
        id: ShoppingListItemId,
        index: ShoppingListItemIndex,
    ) -> Result<()> {
        let mut item = self.read_item(id).await?.context("Item doesn't exist")?;
        item.index = index;

        self.insert_item(&item).await?;

        Ok(())
    }

    pub async fn restore_items(&self, items: &[ShoppingListItem]) -> Result<()> {
        for item in items {
            self.insert_item(item).await?;
        }

        Ok(())
    }

    pub async fn clear_completed(&self) -> Result<()> {
        let items = self
            .read_all_items()
            .await?
            .into_iter()
            .filter(|i| i.completed_at.is_some());

        for completed_item in items {
            self.delete_item(completed_item.id).await?;
        }

        Ok(())
    }

    pub async fn read_shoppinglist(&self) -> Result<ShoppingList> {
        let meta = self.read_meta().await?;
        let items = self.read_all_items().await?;

        Ok(ShoppingList {
            name: meta
                .as_ref()
                .map(|row| row.name.clone())
                .unwrap_or_default(),
            created_at: meta.as_ref().map(|row| row.created_at).unwrap_or_default(),
            items: items.into_iter().map(|v| (v.id, v)).collect(),
        })
    }

    pub async fn read_meta(&self) -> Result<Option<MetaRow>> {
        self.read_value::<_, MetaRow>(META_PREFIX, &()).await
    }

    pub async fn write_meta(&self, name: &str) -> Result<()> {
        let row = MetaRow {
            name: name.to_owned(),
            created_at: Utc::now(),
            version: 1,
        };

        self.write_value::<_, MetaRow>(META_PREFIX, &(), &row)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn insert_item(&self, item: &ShoppingListItem) -> Result<()> {
        self.write_value::<_, ShoppingListItem>(ITEM_PREFIX, &item.id, item)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn read_item(&self, id: ShoppingListItemId) -> Result<Option<ShoppingListItem>> {
        self.read_value::<_, ShoppingListItem>(ITEM_PREFIX, &id)
            .await
    }

    #[tracing::instrument(skip(self))]
    pub async fn read_all_items(&self) -> Result<Vec<ShoppingListItem>> {
        self.read_prefixed_values::<ShoppingListItem>(ITEM_PREFIX)
            .await
    }

    pub async fn delete_item(&self, id: ShoppingListItemId) -> Result<()> {
        self.delete_value(ITEM_PREFIX, &id).await?;

        Ok(())
    }
}

#[derive(Debug, bilrost::Message)]
pub struct MetaRow {
    pub name: String,
    pub created_at: DateTime<Utc>,

    // I don't really have a good provision for schema updates yet, this'll have to do for now
    pub version: u32,
}
