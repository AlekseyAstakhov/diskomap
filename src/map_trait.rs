/// Trait of map what contains indexes.
/// Needed for create indexes with arbitrary storage map, such as 'BTreeMap', 'HashMap', etc.
pub trait MapTrait<Key, Value> {
    fn get(&self, key: &Key) -> Option<&Value>;
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value>;
    fn insert(&mut self, key: Key, value: Value);
    fn remove(&mut self, key: &Key);
}
