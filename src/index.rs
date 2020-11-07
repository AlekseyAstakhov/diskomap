/// Custom index. 'BTree' contains this dyn traits and use when insert or delete elements for update indexes.
pub(crate) trait IndexTrait<BTreeKey, BTreeValue> {
    /// Updates index when insert or update operation on map.
    fn on_insert(&self, key: BTreeKey, value: BTreeValue, old_value: Option<BTreeValue>);
    /// Updates index when remove operation on map.
    fn on_remove(&self, key: &BTreeKey, value: &BTreeValue);
}
