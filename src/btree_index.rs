use crate::index::{IndexTrait, BtreeIndexError};
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
    pub(crate) make_index_key_callback: RwLock<Box<dyn Fn(&BTreeValue) -> IndexKey + Send + Sync + 'static>>,
}

impl<IndexKey: Ord, BTreeKey: Clone, BTreeValue> BtreeIndex<IndexKey, BTreeKey, BTreeValue> {
    /// BTreeMap keys by custom index. Empty vec if no so index.
    pub fn get(&self, key: &IndexKey) -> Result<Vec<BTreeKey>, BtreeIndexError> {
        let mut vec = vec![];
        let map = self.inner.map.read()?;
        if let Some(btree_keys) = map.get(key) {
            vec = (*btree_keys).iter().cloned().collect();
        }

        Ok(vec)
    }

    pub fn len(&self) -> Result<usize, BtreeIndexError> {
        Ok(self.inner.map.read()?.len())
    }


    pub fn is_empty(&self) -> Result<bool, BtreeIndexError> {
        Ok(self.len()? == 0)
    }
}

impl<IndexKey, BTreeKey, BTreeValue> IndexTrait<BTreeKey, BTreeValue> for BtreeIndex<IndexKey, BTreeKey, BTreeValue>
where
    IndexKey: Ord,
    BTreeKey: Ord,
{
    /// Updates index when insert operation on 'BTree'.
    fn on_insert(&self, btree_key: BTreeKey, value: BTreeValue, old_value: Option<BTreeValue>) -> Result<(), BtreeIndexError> {
        let mut map = self.inner.map.write()?;
        if let Some(old_value) = old_value {
            let old_value_index_key = self.inner.make_index_key_callback.read()?(&old_value);
            if let Some(keys) = map.get_mut(&old_value_index_key) {
                keys.remove(&btree_key);
            }
        }

        let index_key = self.inner.make_index_key_callback.read()?(&value);
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

        Ok(())
    }

    /// Updates index when remove operation on 'BTree'.
    fn on_remove(&self, key: &BTreeKey, value: &BTreeValue) -> Result<(), BtreeIndexError> {
        let index_key = self.inner.make_index_key_callback.read()?(&value);
        if let Some(keys) = self.inner.map.write()?.get_mut(&index_key) {
            keys.remove(key);
        }

        Ok(())
    }
}
