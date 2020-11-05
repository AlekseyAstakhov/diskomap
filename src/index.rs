/// Custom index. 'BTree' contains this dyn traits and use when insert or delete elements for update indexes.
pub(crate) trait IndexTrait<BTreeKey, BTreeValue> {
    /// Updates index when insert or update operation on map.
    fn on_insert(&self, key: BTreeKey, value: BTreeValue, old_value: Option<BTreeValue>) -> Result<(), BtreeIndexError>;
    /// Updates index when remove operation on map.
    fn on_remove(&self, key: &BTreeKey, value: &BTreeValue) -> Result<(), BtreeIndexError>;
}

#[derive(Debug)]
pub enum BtreeIndexError {
    PoisonError,
}

// For op-?, "auto" type conversion.
impl<T> From<std::sync::PoisonError<T>> for BtreeIndexError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        BtreeIndexError::PoisonError
    }
}

impl std::fmt::Display for BtreeIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for BtreeIndexError {}
