use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

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

/// For the index that uses the BTreeMap.
pub struct BtreeMapWrapper<IndexKey, OwnerKey> {
    pub map: BTreeMap<IndexKey, OwnerKey>
}

/// std::collections::BTreeMap wrapper.
/// Need because i was not possible to implement the trait directly for std::collections::BTreeMap wrapper.
impl<IndexKey: Ord, OwnerKey> Default for BtreeMapWrapper<IndexKey, OwnerKey> {
    fn default() -> Self {
        BtreeMapWrapper { map: BTreeMap::new() }
    }
}

impl<Key: Ord, Value>  MapTrait<Key, Value>  for BtreeMapWrapper<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.map.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.map.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value) { self.map.insert(key, value); }
    fn remove(&mut self, key: &Key) { self.map.remove(key); }
}

/// std::collections::HashMap wrapper.
/// Need because i was not possible to implement the trait directly for std::collections::HashMap wrapper.
pub struct HashMapWrapper<IndexKey, OwnerKey> {
    map: HashMap<IndexKey, OwnerKey>
}

impl<IndexKey: Hash, OwnerKey> Default for HashMapWrapper<IndexKey, OwnerKey> {
    fn default() -> Self {
        HashMapWrapper { map: HashMap::new() }
    }
}

impl<Key: Hash + Eq, Value>  MapTrait<Key, Value>  for HashMapWrapper<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.map.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.map.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value) { self.map.insert(key, value); }
    fn remove(&mut self, key: &Key) { self.map.remove(key); }
}
