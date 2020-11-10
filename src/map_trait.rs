use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::ops::Deref;

/// Trait of map.
/// Needed for generalize maps, such as 'BTreeMap', 'HashMap', and use custom maps.
pub trait MapTrait<Key, Value> {
    /// Returns a reference to the value corresponding to the key.
    fn get(&self, key: &Key) -> Option<&Value>;
    /// Returns a mutable reference to the value corresponding to the key.
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value>;
    /// Inserts a key-value pair into the map and return old value. If the map did not have this key present, `None` is returned.
    fn insert(&mut self, key: Key, value: Value) -> Option<Value>;
    /// Removes a key from the map, returning the value at the key if the key was previously in the map.
    fn remove(&mut self, key: &Key) -> Option<Value>;
}

/// std::collections::BTreeMap wrapper.
/// Need because i was not possible to implement the trait directly for std::collections::BTreeMap wrapper.
pub struct BtreeMapWrapper<Key, Value> {
    map: BTreeMap<Key, Value>
}

impl<Key: Ord, Value> Default for BtreeMapWrapper<Key, Value> {
    fn default() -> Self {
        BtreeMapWrapper { map: BTreeMap::new() }
    }
}

impl<Key: Ord, Value>  MapTrait<Key, Value>  for BtreeMapWrapper<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.map.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.map.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value) -> Option<Value> { self.map.insert(key, value) }
    fn remove(&mut self, key: &Key) -> Option<Value> { self.map.remove(key) }
}

impl<Key, Value> Deref for BtreeMapWrapper<Key, Value> {
    type Target = BTreeMap<Key, Value>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

/// std::collections::HashMap wrapper.
/// Need because i was not possible to implement the trait directly for std::collections::HashMap wrapper.
pub struct HashMapWrapper<Key, Value> {
    map: HashMap<Key, Value>
}

impl<Key: Hash, Value> Default for HashMapWrapper<Key, Value> {
    fn default() -> Self {
        HashMapWrapper { map: HashMap::new() }
    }
}

impl<Key: Hash + Eq, Value>  MapTrait<Key, Value>  for HashMapWrapper<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.map.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.map.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value)  -> Option<Value> { self.map.insert(key, value) }
    fn remove(&mut self, key: &Key) -> Option<Value> { self.map.remove(key) }
}

impl<Key, Value> Deref for HashMapWrapper<Key, Value> {
    type Target = HashMap<Key, Value>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<Key: Ord, Value>  MapTrait<Key, Value>  for BTreeMap<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value) -> Option<Value> { self.insert(key, value) }
    fn remove(&mut self, key: &Key) -> Option<Value> { self.remove(key) }
}
