/// Trait of map what contains indexes.
/// Needed for create indexes with arbitrary storage map, such as 'BTreeMap', 'HashMap', etc.
pub trait MapTrait<Key, Value> {
    /// Returns a reference to the value corresponding to the key.
    fn get(&self, key: &Key) -> Option<&Value>;
    /// Returns a mutable reference to the value corresponding to the key.
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value>;
    /// Inserts a key-value pair into the map. If the map did not have this key present, `None` is returned.
    fn insert(&mut self, key: Key, value: Value);
    /// Removes a key from the map, returning the value at the key if the key was previously in the map.
    fn remove(&mut self, key: &Key);
}
