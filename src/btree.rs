use crate::index::{IndexTrait, BtreeIndexError};
use crate::btree_index::BtreeIndex;
use crate::file_worker::FileWorker;
use crate::file_work::{
    load_from_file,
    file_line_of_insert,
    file_line_of_remove,
    create_dirs_to_path_if_not_exist,
    LoadFileError
};
use crate::Integrity;
use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::sync::{Arc, RwLock};

/// A map based on a B-Tree with the operations log file on the disk.
/// Used in a similar way as a BTreeMap, but store to file log of operations as insert and remove
/// for restoring actual data after restart application.
pub struct BTree<Key, Value> {
    /// Map in the RAM.
    map: BTreeMap<Key, Value>,
    // For append operations to the operations log file in background thread.
    file_worker: Option<FileWorker>,

    /// Created indexes.
    indexes: Vec<Box<dyn IndexTrait<Key, Value>>>,
    /// Mechanism of controlling the integrity of stored data in a log file.
    integrity: Option<Integrity>,
}

impl<Key, Value: 'static> BTree<Key, Value>
where
    Key: Serialize + DeserializeOwned + Ord + Clone + 'static,
    Value: Serialize + DeserializeOwned + Clone,
{
    /// Open/create map with 'operations_log_file'.
    /// If file is exist then load map from file.
    /// If file not is not exist then create new file.
    pub fn open_or_create(file_path: &str, mut integrity: Option<Integrity>) -> Result<Self, BTreeError> {
        create_dirs_to_path_if_not_exist(file_path)?;

        let mut file = OpenOptions::new().read(true).write(true).append(true).create(true).open(file_path)?;

        file.lock_exclusive()?;

        // load current map from operations log file
        let map = match load_from_file(&mut file, &mut integrity) {
            Ok(map) => {
                map
            }
            Err(err) => {
                file.unlock()?;
                return Err(BTreeError::LoadFileError(err));
            }
        };

        let on_background_error = None;

        Ok(BTree {
            map,
            file_worker: Some(FileWorker::new(file, on_background_error)),
            indexes: Vec::new(),
            integrity,
        })
    }

    /// Inserts a key-value pair into the map. This function is used for updating too.
    /// Data will be written to RAM immediately, and to disk later in a separate thread.
    pub fn insert(&mut self, key: Key, value: Value) -> Result<Option<Value>, BTreeError> {
        let old_value = self.map.insert(key.clone(), value.clone());

        // update in index
        for index in self.indexes.iter() {
            index.on_insert(key.clone(), value.clone(), old_value.clone())?;
        }

        // add operation to operations log file
        let key_val_json = serde_json::to_string(&(&key, &value))?;

        if let Some(file_worker) = &self.file_worker {
            let line = file_line_of_insert(&key_val_json, &mut self.integrity);
            file_worker.write(line);
        } else {
            unreachable!();
        }

        Ok(old_value)
    }

    /// Get value by key from the map in RAM. No writing to the operations log file.
    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.map.get(key)
    }

    /// Remove value by key from the map in memory and asynchronously append operation to the file.
    pub fn remove(&mut self, key: &Key) -> Result<Option<Value>, BTreeError> {
        if let Some(old_value) = self.map.remove(&key) {
            // remove from indexes
            for index in self.indexes.iter() {
                index.on_remove(&key, &old_value)?;
            }

            let key_json = serde_json::to_string(&key)?;

            if let Some(file_worker) = &self.file_worker {
                let line = file_line_of_remove(&key_json, &mut self.integrity);
                file_worker.write(line);
            } else {
                unreachable!();
            }

            return Ok(Some(old_value.clone()));
        }

        Ok(None)
    }

    /// Create custom index by value.
    /// 'make_index_key_callback' function is called during all operations of inserting,
    /// and deleting elements. In the function it is necessary to determine
    /// the value and type of the index key in any way related to the value of the 'BTree'.
    pub fn create_btree_index<IndexKey, F>(&mut self, make_index_key_callback: F)
        -> BtreeIndex<IndexKey, Key, Value>
    where
        IndexKey: Clone + Ord + Send + Sync + 'static,
        F: Fn(&Value) -> IndexKey + Send + Sync + 'static,
    {
        let mut index_map: BTreeMap<IndexKey, BTreeSet<Key>> = BTreeMap::new();

        for (key, val) in self.map.iter() {
            let index_key = make_index_key_callback(&val);
            match index_map.get_mut(&index_key) {
                Some(keys) => {
                    keys.insert(key.clone());
                }
                None => {
                    let mut set = BTreeSet::new();
                    set.insert(key.clone());
                    index_map.insert(index_key, set);
                }
            }
        }

        let index = BtreeIndex {
            inner: Arc::new(crate::btree_index::Inner {
                map: RwLock::new(index_map),
                make_index_key_callback: RwLock::new(Box::new(make_index_key_callback)),
            }),
        };

        self.indexes.push(Box::new(index.clone()));

        index
    }

    /// Returns reference to inner map.
    pub fn map(&self) -> &BTreeMap<Key, Value> {
        &self.map
    }

    /// Returns the number of elements in the map. No writing to the operations log file.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the map contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if the map in memory contains a value for the specified key.
    pub fn contains_key(&self, key: &Key) -> bool {
        self.map.contains_key(key)
    }

    /// Returns cloned keys of the map, in sorted order. No writing to the operations log file.
    pub fn cloned_keys(&self) -> Vec<Key> {
        self.map.keys().cloned().collect()
    }

    /// Returns cloned values of the map, in sorted order. No writing to the operations log file.
    pub fn cloned_values(&self) -> Vec<Value> {
        self.map.values().map(|val| val.clone()).collect()
    }
}

/// Errors when working with BTree.
#[derive(Debug)]
pub enum BTreeError {
    /// When load file.
    LoadFileError(LoadFileError),
    /// Error of working with file.
    FileError(std::io::Error),
    /// Json error with line number in operations log file.
    JsonSerializeError(serde_json::Error),
    /// Errors when working with the indexes.
    IndexError,
}

impl From<std::io::Error> for BTreeError {
    fn from(err: std::io::Error) -> Self {
        BTreeError::FileError(err)
    }
}

impl From<serde_json::error::Error> for BTreeError {
    fn from(err: serde_json::error::Error) -> Self {
        BTreeError::JsonSerializeError(err)
    }
}
impl From<BtreeIndexError> for BTreeError {
    fn from(_: BtreeIndexError) -> Self {
        BTreeError::IndexError
    }
}

impl std::fmt::Display for BTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for BTreeError {}
