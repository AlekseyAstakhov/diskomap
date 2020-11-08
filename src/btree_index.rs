use crate::index::IndexTrait;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

/// The Btree index for getting indexes of the map by parts of value.
#[derive(Clone)]
pub struct BtreeIndex<IndexKey: Ord, BTreeKey, BTreeValue> {
    pub(crate) inner: Arc<Inner<IndexKey, BTreeKey, BTreeValue>>,
}

/// Inner data of 'BtreeIndex', need for clone all fields of 'BtreeIndex' together.
pub struct Inner<IndexKey: Ord, BTreeKey, BTreeValue> {
    pub(crate) map: RwLock<BTreeMap<IndexKey, BTreeSet<BTreeKey>>>,
    pub(crate) make_index_key_callback: fn(&BTreeValue) -> IndexKey,
}

impl<IndexKey: Ord, BTreeKey: Clone, BTreeValue> BtreeIndex<IndexKey, BTreeKey, BTreeValue> {
    /// BTreeMap keys by custom index. Empty vec if no so index.
    pub fn get(&self, key: &IndexKey) -> Vec<BTreeKey> {
        let mut vec = vec![];
        let map = self.inner.map.read().unwrap();
        if let Some(btree_keys) = map.get(key) {
            vec = (*btree_keys).iter().cloned().collect();
        }

        vec
    }

    pub fn len(&self) -> usize {
        self.inner.map.read().unwrap().len()
    }


    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<IndexKey, BTreeKey, BTreeValue> IndexTrait<BTreeKey, BTreeValue> for BtreeIndex<IndexKey, BTreeKey, BTreeValue>
where
    IndexKey: Ord,
    BTreeKey: Ord,
{
    /// Updates index when insert operation on 'BTree'.
    fn on_insert(&self, btree_key: BTreeKey, value: BTreeValue, old_value: Option<BTreeValue>) {
        let index_key = (self.inner.make_index_key_callback)(&value);
        let old_value_index_key = if let Some(old_value) = old_value {
            Some((self.inner.make_index_key_callback)(&old_value))
        } else {
            None
        };

        if let Ok(mut map) = self.inner.map.write() {
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
        let index_key = (self.inner.make_index_key_callback)(&value);

        if let Ok(mut map)= self.inner.map.write() {
            if let Some(keys) = map.get_mut(&index_key) {
                keys.remove(key);
            }
        } else {
            unreachable!(); // because there is no code that can cause panic during blocking
        }
    }
}
