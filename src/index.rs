use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, RwLock};
use std::hash::Hash;

/// The index for getting indexes of the owner map by parts of value.
#[derive(Clone)]
pub struct Index<IndexKey, OwnerKey, OwnerValue>  {
    map: Arc<RwLock<dyn IndexMap<IndexKey, BTreeSet<OwnerKey>>>>,
    make_index_key_callback: fn(&OwnerValue) -> IndexKey,
}

impl<IndexKey, OwnerKey: Ord + Clone, OwnerValue> Index<IndexKey, OwnerKey, OwnerValue> {
    /// Owner keys by custom index. Empty vec if no so index.
    pub fn get(&self, key: &IndexKey) -> Vec<OwnerKey> {
        let mut vec = vec![];
        if let Ok(map) = self.map.read() {
            if let Some(btree_keys) = map.get(key) {
                vec = (*btree_keys).iter().cloned().collect();
            }
        } else {
            unreachable!(); // because there is no code that can cause panic during blocking
        }

        vec
    }

    /// Constructs new Index from custom map and make index callback.
    pub(crate) fn new<Map>(indexes: Map, make_index_key_callback: fn(&OwnerValue) -> IndexKey) -> Self
        where Map: IndexMap<IndexKey, BTreeSet<OwnerKey>> + 'static
    {
        Index {
            map: Arc::new(RwLock::new(indexes)),
            make_index_key_callback,
        }
    }
}

impl<IndexKey, OwnerKey: Ord, OwnerValue> UpdateIndex<OwnerKey, OwnerValue> for Index<IndexKey, OwnerKey, OwnerValue> {
    /// Implementation of updating of index when insert operation on owner map.
    fn on_insert(&self, btree_key: OwnerKey, value: OwnerValue, old_value: Option<OwnerValue>) {
        let index_key = (self.make_index_key_callback)(&value);
        let old_value_index_key = if let Some(old_value) = old_value {
            Some((self.make_index_key_callback)(&old_value))
        } else {
            None
        };

        if let Ok(mut map) = self.map.write() {
            if let Some(old_value_index_key) = old_value_index_key {
                if let Some(keys) = map.get_mut(&old_value_index_key) {
                    keys.remove(&btree_key);
                }
            }

            match map.get_mut(&index_key) {
                Some(keys) => {
                    keys.insert(btree_key);
                }
                None => {
                    let mut set = BTreeSet::new();
                    set.insert(btree_key);
                    map.insert(index_key, set);
                }
            }
        } else {
            unreachable!(); // because there is no code that can cause panic during blocking
        }
    }

    /// Implementation of updating of index when remove operation on owner map.
    fn on_remove(&self, key: &OwnerKey, value: &OwnerValue) {
        let index_key = (self.make_index_key_callback)(&value);

        if let Ok(mut map)= self.map.write() {
            if let Some(keys) = map.get_mut(&index_key) {
                keys.remove(key);
            }
        } else {
            unreachable!(); // because there is no code that can cause panic during blocking
        }
    }
}

/// Trait for update the index when the owner map content changes.
pub(crate) trait UpdateIndex<OwnerKey, OwnerValue> {
    /// Updates index when insert or update operation on map.
    fn on_insert(&self, key: OwnerKey, value: OwnerValue, old_value: Option<OwnerValue>);
    /// Updates index when remove operation on map.
    fn on_remove(&self, key: &OwnerKey, value: &OwnerValue);
}

/// Trait of map what contains indexes.
/// Needed for create indexes with arbitrary storage map, such as 'BTreeMap', 'HashMap', etc.
pub trait IndexMap<Key, Value> {
    fn get(&self, key: &Key) -> Option<&Value>;
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value>;
    fn insert(&mut self, key: Key, value: Value);
}

/// For the index that uses the BTreeMap.
pub struct BtreeIndexMap<IndexKey, OwnerKey> {
    pub map: BTreeMap<IndexKey, OwnerKey>
}

impl<IndexKey: Ord, OwnerKey> Default for BtreeIndexMap<IndexKey, OwnerKey> {
    fn default() -> Self {
        BtreeIndexMap { map: BTreeMap::new() }
    }
}

impl<Key: Ord, Value>  IndexMap<Key, Value>  for BtreeIndexMap<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.map.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.map.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value) { self.map.insert(key, value); }
}

/// For the index that uses the HashMap.
pub struct HashIndexMap<IndexKey, OwnerKey> {
    map: HashMap<IndexKey, OwnerKey>
}

impl<IndexKey: Hash, OwnerKey> Default for HashIndexMap<IndexKey, OwnerKey> {
    fn default() -> Self {
        HashIndexMap { map: HashMap::new() }
    }
}

impl<Key: Hash + Eq, Value>  IndexMap<Key, Value>  for HashIndexMap<Key, Value>  {
    fn get(&self, key: &Key) -> Option<&Value> { self.map.get(key) }
    fn get_mut(&mut self, key: &Key) -> Option<&mut Value> { self.map.get_mut(key) }
    fn insert(&mut self, key: Key, value: Value) { self.map.insert(key, value); }
}
