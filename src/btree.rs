use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::hash::Hash;
use crate::index::{UpdateIndex, Index, BtreeIndexMap, HashIndexMap};
use crate::file_worker::FileWorker;
use crate::Integrity;
use crate::file_work::{
    load_from_file,
    file_line_of_insert,
    file_line_of_remove,
    create_dirs_to_path_if_not_exist,
    LoadFileError
};
use crate::map_trait::MapTrait;

/// A map based on a B-Tree with the operations log file on the disk.
/// Used in a similar way as a BTreeMap, but store to file log of operations as insert and remove
/// for restoring actual data after restart application.
pub struct BTree<Key, Value> {
    /// Map in the RAM.
    map: BTreeMap<Key, Value>,
    // For append operations to the operations log file in background thread.
    file_worker: FileWorker,

    /// Created indexes.
    indexes: Vec<Box<dyn UpdateIndex<Key, Value>>>,
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
    pub fn open_or_create(file_path: &str, mut integrity: Option<Integrity>) -> Result<Self, LoadFileError> {
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
                return Err(err);
            }
        };

        let on_background_error = None;

        Ok(BTree {
            map,
            file_worker: FileWorker::new(file, on_background_error),
            indexes: Vec::new(),
            integrity,
        })
    }

    /// Inserts a key-value pair into the map. This function is used for updating too.
    /// Data will be written to RAM immediately, and to disk later in a separate thread.
    pub fn insert(&mut self, key: Key, value: Value) -> Result<Option<Value>, serde_json::Error> {
        let old_value = self.map.insert(key.clone(), value.clone());

        // update in index
        for index in self.indexes.iter() {
            index.on_insert(key.clone(), value.clone(), old_value.clone());
        }

        // add operation to operations log file
        let line = file_line_of_insert(&key, &value, &mut self.integrity)?;
        self.file_worker.write(line);

        Ok(old_value)
    }

    /// Get value by key from the map in RAM. No writing to the operations log file.
    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.map.get(key)
    }

    /// Remove value by key from the map in memory and asynchronously append operation to the file.
    pub fn remove(&mut self, key: &Key) -> Result<Option<Value>, serde_json::Error> {
        if let Some(old_value) = self.map.remove(&key) {
            // remove from indexes
            for index in self.indexes.iter() {
                index.on_remove(&key, &old_value);
            }

            let line = file_line_of_remove(key, &mut self.integrity)?;
            self.file_worker.write(line);

            return Ok(Some(old_value.clone()));
        }

        Ok(None)
    }

    /// Create index by value based on std::collections::BTreeMap.
    /// 'make_index_key_callback' function is called during all operations of inserting,
    /// and deleting elements. In the function it is necessary to determine
    /// the value and type of the index key in any way related to the value of the 'BTree'.
    pub fn create_btree_index<IndexKey>(&mut self, make_index_key_callback: fn(&Value) -> IndexKey)
        -> Index<IndexKey, Key, Value>
        where
            IndexKey: Clone + Ord + 'static
    {
        self.create_index::<IndexKey, BtreeIndexMap<IndexKey, BTreeSet<Key>>>(make_index_key_callback)
    }

    /// Create index by value based on std::collections::HashMap.
    /// 'make_index_key_callback' function is called during all operations of inserting,
    /// and deleting elements. In the function it is necessary to determine
    /// the value and type of the index key in any way related to the value of the 'BTree'.
    pub fn create_hashmap_index<IndexKey>(&mut self, make_index_key_callback: fn(&Value) -> IndexKey)
        -> Index<IndexKey, Key, Value>
    where
        IndexKey: Clone + Hash + Eq + 'static,
    {
        self.create_index::<IndexKey, HashIndexMap<IndexKey, BTreeSet<Key>>>(make_index_key_callback)
    }

    /// Create index by value.
    /// 'make_index_key_callback' function is called during all operations of inserting,
    /// and deleting elements. In the function it is necessary to determine
    /// the value and type of the index key in any way related to the value of the 'BTree'.
    pub fn create_index<IndexKey, Map>(&mut self, make_index_key_callback: fn(&Value) -> IndexKey)
                                          -> Index<IndexKey, Key, Value>
        where
            IndexKey: Clone + Eq + 'static,
            Map: MapTrait<IndexKey, BTreeSet<Key>> + Default + Sized + 'static,
    {
        let mut index_map = Map::default();

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

        let index = Index::new(index_map, make_index_key_callback);
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
}
