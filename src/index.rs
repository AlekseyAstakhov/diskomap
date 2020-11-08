/// Trait for update the index when the map containing it changes.
pub(crate) trait UpdateIndex<BTreeKey, BTreeValue> {
    /// Updates index when insert or update operation on map.
    fn on_insert(&self, key: BTreeKey, value: BTreeValue, old_value: Option<BTreeValue>);
    /// Updates index when remove operation on map.
    fn on_remove(&self, key: &BTreeKey, value: &BTreeValue);
}
