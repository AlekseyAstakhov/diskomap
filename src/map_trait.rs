use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

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
    /// Iterate over all elements and call callback for each.
    fn for_each(&self, f: impl FnMut(&Key, &Value));
}

impl<Key: Ord, Value>  MapTrait<Key, Value> for BTreeMap<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value) -> Option<Value> { self.insert(key, value) }
    fn remove(&mut self, key: &Key) -> Option<Value> { self.remove(key) }
    fn for_each(&self, mut f: impl FnMut(&Key, &Value)) { for (key, val) in self.iter() { f(key, val) } }
}

impl<Key: Hash + Eq, Value>  MapTrait<Key, Value>  for HashMap<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value)  -> Option<Value> { self.insert(key, value) }
    fn remove(&mut self, key: &Key) -> Option<Value> { self.remove(key) }
    fn for_each(&self, mut f: impl FnMut(&Key, &Value)) { for (key, val) in self.iter() { f(key, val) } }
}
