use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};
use crate::map_trait::MapTrait;
use std::marker::PhantomData;

/// The index for getting indexes of the owner map by parts of value.
pub struct Index<IndexKey, OwnerKey, OwnerValue, SelfMap>
where SelfMap: MapTrait<IndexKey, BTreeSet<OwnerKey>> {
    /// Indexes of owner map by index keys.
    map: Arc<RwLock<SelfMap>>,
    /// Make index callback.
    make_index_key_callback: fn(&OwnerValue) -> IndexKey,
    /// Need for avoid "unused parameter" compile error.
    _phantom: PhantomData<OwnerKey>,
}

impl<IndexKey, OwnerKey, OwnerValue, SelfMap> Index<IndexKey, OwnerKey, OwnerValue, SelfMap>
where
    OwnerKey: Ord + Clone,
    SelfMap: MapTrait<IndexKey, BTreeSet<OwnerKey>> {

    /// Owner keys by custom index. Empty vec if no so index.
    pub fn get(&self, key: &IndexKey) -> Vec<OwnerKey> {
        let mut vec = vec![];
        let map = self.map.read()
            .unwrap_or_else(|err| unreachable!(err)); // unreachable because no code with possible panic when this map locked

        if let Some(btree_keys) = map.get(key) {
            vec = (*btree_keys).iter().cloned().collect();
        }

        vec
    }

    /// Constructs new Index from custom map and make index callback.
    pub(crate) fn new(indexes: SelfMap, make_index_key_callback: fn(&OwnerValue) -> IndexKey) -> Self {
        Index {
            map: Arc::new(RwLock::new(indexes)),
            make_index_key_callback,
            _phantom: PhantomData,
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

impl<IndexKey, OwnerKey: Ord, OwnerValue, SelfMap> UpdateIndex<OwnerKey, OwnerValue> for Index<IndexKey, OwnerKey, OwnerValue, SelfMap>
where
    OwnerKey: Ord,
    SelfMap: MapTrait<IndexKey, BTreeSet<OwnerKey>> {

    /// Implementation of updating of index when insert operation on owner map.
    fn on_insert(&self, btree_key: OwnerKey, value: OwnerValue, old_value: Option<OwnerValue>) {
        let index_key = (self.make_index_key_callback)(&value);
        let old_value_index_key = if let Some(old_value) = old_value {
            Some((self.make_index_key_callback)(&old_value))
        } else {
            None
        };

        let mut map = self.map.write()
            .unwrap_or_else(|err| unreachable!(err)); // unreachable because no code with possible panic when this map locked

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
    }

    /// Implementation of updating of index when remove operation on owner map.
    fn on_remove(&self, key: &OwnerKey, value: &OwnerValue) {
        let index_key = (self.make_index_key_callback)(&value);

        let mut map = self.map.write()
            .unwrap_or_else(|err| unreachable!(err)); // unreachable because no code with possible panic when this map locked

        let mut need_remove_index = false;
        if let Some(keys) = map.get_mut(&index_key) {
            keys.remove(key);
            if keys.is_empty() {
                need_remove_index = true;
            }
        }
        if need_remove_index {
            map.remove(&index_key);
        }
    }
}

impl<IndexKey, OwnerKey, OwnerValue, SelfMap> Clone for Index<IndexKey, OwnerKey, OwnerValue, SelfMap>
    where SelfMap: MapTrait<IndexKey, BTreeSet<OwnerKey>> {

    /// Manually clone because #[derive(Clone)] can't work with PhantomData
    fn clone(&self) -> Self {
        Index {
            map: self.map.clone(),
            make_index_key_callback: self.make_index_key_callback,
            _phantom: PhantomData,
        }
    }
}
