use crate::models::ShoppingListId;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ShoppingListEvent {
    pub id: ShoppingListId,
    pub event: ShoppingListEventInner,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ShoppingListEventInner {
    // Not sure what events we need here, or even if we need more than just a 'I changed' anyway.
    // Currently any event is interpreted to mean 'refresh everything'
    Todo,
}
