use crate::index::IndexTrait;
use std::collections::{HashMap, BTreeSet};
use std::sync::{Arc, RwLock};
use std::hash::Hash;

/// The Btree index for getting indexes of the map by parts of value.
#[derive(Clone)]
pub struct HashMapIndex<IndexKey: Hash + Eq, BTreeKey, BTreeValue> {
    map: Arc<RwLock<HashMap<IndexKey, BTreeSet<BTreeKey>>>>,
    make_index_key_callback: fn(&BTreeValue) -> IndexKey,
}

impl<IndexKey: Hash + Eq, BTreeKey: Clone, BTreeValue> HashMapIndex<IndexKey, BTreeKey, BTreeValue> {
    /// BTreeMap keys by custom index. Empty vec if no so index.
    pub fn get(&self, key: &IndexKey) -> Vec<BTreeKey> {
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

    pub(crate) fn new(indexes: HashMap<IndexKey, BTreeSet<BTreeKey>>, make_index_key_callback: fn(&BTreeValue) -> IndexKey) -> Self {
        HashMapIndex {
            map: Arc::new(RwLock::new(indexes)),
            make_index_key_callback,
        }
    }
}

impl<IndexKey, BTreeKey, BTreeValue> IndexTrait<BTreeKey, BTreeValue> for HashMapIndex<IndexKey, BTreeKey, BTreeValue>
    where
        IndexKey: Hash + Eq,
        BTreeKey: Ord,
{
    /// Updates index when insert operation on 'BTree'.
    fn on_insert(&self, btree_key: BTreeKey, value: BTreeValue, old_value: Option<BTreeValue>) {
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

    /// Updates index when remove operation on 'BTree'.
    fn on_remove(&self, key: &BTreeKey, value: &BTreeValue) {
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
