use crate::btree::IndexTrait;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct BtreeIndex<IndexKey: Ord, BTreeKey, BTreeValue> {
    pub(crate) inner: Arc<Inner<IndexKey, BTreeKey, BTreeValue>>,
}

pub struct Inner<IndexKey: Ord, BTreeKey, BTreeValue> {
    pub(crate) map: RwLock<BTreeMap<IndexKey, BTreeSet<BTreeKey>>>,
    pub(crate) make_index_key_callback: RwLock<Box<dyn Fn(&BTreeValue) -> IndexKey + Send + Sync + 'static>>,
}

impl<IndexKey: Ord, BTreeKey: Clone, BTreeValue> BtreeIndex<IndexKey, BTreeKey, BTreeValue> {
    /// BTreeMap keys by custom index. Empty vec if no so index.
    pub fn get(&self, key: &IndexKey) -> Vec<BTreeKey> {
        let mut vec = vec![];

        match self.inner.map.read() {
            Ok(map) => {
                if let Some(btree_keys) = map.get(key) {
                    vec = (*btree_keys).iter().cloned().collect();
                }
            }
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }

        vec
    }

    pub fn len(&self) -> usize {
        match self.inner.map.read() {
            Ok(map) => map.len(),
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }
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
    fn on_insert(&self, btree_key: BTreeKey, value: BTreeValue, old_value: Option<BTreeValue>) -> Result<(), IndexError> {
        let mut map = self.inner.map.write()?;
        if let Some(old_value) = old_value {
            let old_value_index_key = self.inner.make_index_key_callback.read()?(&old_value);
            match map.get_mut(&old_value_index_key) {
                Some(keys) => {
                    keys.remove(&btree_key);
                }
                None => {
                    unreachable!();
                }
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
    fn on_remove(&self, key: &BTreeKey, value: &BTreeValue) -> Result<(), IndexError> {
        let index_key = self.inner.make_index_key_callback.read()?(&value);
        if let Some(keys) = self.inner.map.write()?.get_mut(&index_key) {
            keys.remove(key);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum IndexError {
    PoisonError,
}

// For op-?, "auto" type conversion.
impl<T> From<std::sync::PoisonError<T>> for IndexError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        IndexError::PoisonError
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[derive(Clone, Serialize, Deserialize)]
    struct User {
        name: String,
        age: u8,
    }

    #[test]
    fn test() -> Result<(), Box<dyn std::error::Error>> {
        // new log file
        let log_file = tempdir()?.path().join("test.txt").to_str().unwrap().to_string();
        let map = crate::BTree::open_or_create(&log_file)?;
        let user_name_index = map.create_btree_index(|value: &User| value.name.clone());

        map.insert(0, User { name: "Mary".to_string(), age: 21 });
        map.insert(1, User { name: "John".to_string(), age: 37 });

        assert_eq!(user_name_index.get(&"John".to_string()), vec![1]);
        assert!(user_name_index.get(&"Masha".to_string()).is_empty());

        map.insert(3, User { name: "Masha".to_string(), age: 27 });
        map.insert(0, User { name: "Natasha".to_string(), age: 23 });

        assert_eq!(user_name_index.get(&"Natasha".to_string()), vec![0]);
        assert!(user_name_index.get(&"Mary".to_string()).is_empty());
        assert_eq!(user_name_index.get(&"Masha".to_string()), vec![3]);
        map.insert(5, User { name: "Natasha".to_string(), age: 25 });

        map.insert(1, User { name: "Bob".to_string(), age: 27 });
        assert!(user_name_index.get(&"John".to_string()).is_empty());

        map.remove(&1);
        assert!(user_name_index.get(&"Bob".to_string()).is_empty());

        map.insert(8, User { name: "Masha".to_string(), age: 23 });
        map.insert(12, User { name: "Masha".to_string(), age: 24 });
        assert_eq!(user_name_index.get(&"Masha".to_string()), vec![3, 8, 12]);

        map.insert(8, User { name: "Natasha".to_string(), age: 35 });
        assert_eq!(user_name_index.get(&"Masha".to_string()), vec![3, 12]);
        assert_eq!(user_name_index.get(&"Natasha".to_string()), vec![0, 5, 8]);

        Ok(())
    }
}
