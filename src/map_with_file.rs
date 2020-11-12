use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs::{OpenOptions, File};
use std::hash::Hash;
use crate::index::{UpdateIndex, Index};
use crate::file_worker::FileWorker;
use crate::file_work::{
    map_from_file,
    file_line_of_insert,
    file_line_of_remove,
    create_dirs_to_path_if_not_exist,
    LoadFileError
};
use crate::map_trait::MapTrait;
use crate::cfg::Cfg;
use std::sync::{Arc, Mutex};
use std::io::Write;

/// Map with storing all changes history to the file.
/// Restores own state from the file when creating.
/// Based on std::collections::BTreeMap.
pub type BTreeMap<Key, Value> = MapWithFile<Key, Value, std::collections::BTreeMap<Key, Value>>;

/// Map with storing all changes history to the file.
/// Restores own state from the file when creating.
/// Based on std::collections::HashMap.
pub type HashMap<Key, Value> = MapWithFile<Key, Value, std::collections::HashMap<Key, Value>>;

/// Map container with storing all changes history to the file.
/// Restores own state from the file when creating.
pub struct MapWithFile<Key, Value, Map>
where Map: MapTrait<Key, Value>  {
    /// Map in the RAM.
    map: Map,
    // For append operations to the history file in background thread.
    file_worker: FileWorker,
    /// Created indexes.
    indexes: Vec<Box<dyn UpdateIndex<Key, Value>>>,
    /// Config.
    cfg: Cfg,
    /// Opened and exclusive locked history file of operations on map.
    file: Arc<Mutex<File>>,
}

impl<Key, Value: 'static, Map> MapWithFile<Key, Value, Map>
where
    Key: Serialize + DeserializeOwned + Ord + Clone + 'static,
    Value: Serialize + DeserializeOwned + Clone,
    Map: MapTrait<Key, Value> + Default {

    /// Open/create map with history file 'file_path'.
    /// If file is exist then load map from file.
    /// If file not is not exist then create new file.
    pub fn open_or_create(file_path: &str, mut cfg: Cfg) -> Result<Self, LoadFileError> {
        create_dirs_to_path_if_not_exist(file_path)?;

        let mut file = OpenOptions::new().read(true).write(true).append(true).create(true).open(file_path)?;
        file.lock_exclusive()?;

        // load current map from history file
        let map = map_from_file::<Map, Key, Value, _>(&mut file, &mut cfg.integrity, cfg.after_read_callback.take())?;

        let file = Arc::new(Mutex::new(file));

        Ok(MapWithFile {
            map,
            file_worker: FileWorker::new(file.clone(), cfg.write_error_callback.take()),
            indexes: Vec::new(),
            cfg: cfg,
            file
        })
    }

    /// Inserts a key-value pair into the map.
    /// Data will be written to RAM immediately, and to disk later in a separate thread.
    pub fn insert(&mut self, key: Key, value: Value) -> Result<Option<Value>, serde_json::Error> {
        let (line, old_val) = self.insert_inner(key, value)?;
        // add operation to history file
        self.file_worker.write(line);

        Ok(old_val)
    }

    /// Inserts a key-value pair into the map with returning write to file result.
    /// Writing is performed synchronously in current thread.
    /// Writing is performed synchronously in current thread and out of file worker order queue.
    pub fn insert_sync(&mut self, key: Key, value: Value) -> Result<Option<Value>, SyncInsertRemoveError> {
        let (line, old_val) = self.insert_inner(key, value)?;
        // add operation to history file
        let mut file = self.file.lock()
            .unwrap_or_else(|err| unreachable!(err)); // unreachable because no code with possible panic under lock of this file

        file.write(line.as_bytes())?;

        Ok(old_val)
    }

    /// Insert to the map, prepare data for writing to the file and update indexes. Common for 'insert' and 'insert_sync'.
    fn insert_inner(&mut self, key: Key, value: Value) -> Result<(String, Option<Value>), serde_json::Error> {
        // insert to the map
        let old_value = self.map.insert(key.clone(), value.clone());
        // prepare data for write
        let mut line = file_line_of_insert(&key, &value, &mut self.cfg.integrity)?;

        // user callback
        if let Some(f) = &mut self.cfg.before_write_callback {
            if let Some(transformed_line) = f(&line) {
                line = transformed_line;
            }
        }

        // update in index
        for index in self.indexes.iter() {
            index.on_insert(key.clone(), value.clone(), old_value.clone());
        }

        Ok((line, old_value))
    }

    /// Returns a reference to the value corresponding to the key. No writing to the history file.
    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.map.get(key)
    }

    /// Remove value by key from the map in memory and asynchronously append operation to the file.
    pub fn remove(&mut self, key: &Key) -> Result<Option<Value>, serde_json::Error> {
        if let Some((line, old_val)) = self.remove_inner(key)? {
            // add operation to history file
            self.file_worker.write(line);

            return Ok(Some(old_val));
        }

        Ok(None)
    }

    /// Remove value by key from the map with returning write to history file result.
    /// Writing is performed synchronously in current thread and out of file worker order queue.
    pub fn remove_sync(&mut self, key: &Key) -> Result<Option<Value>, SyncInsertRemoveError> {
        if let Some((line, old_val)) = self.remove_inner(&key)? {
            // add operation to history file
            let mut file = self.file.lock()
                .unwrap_or_else(|err| unreachable!(err)); // unreachable because no code with possible panic under lock of this file

            file.write(line.as_bytes())?;

            return Ok(Some(old_val));
        }

        Ok(None)
    }

    /// Remove from the map, prepare data for writing to the file and update indexes. Common for 'remove' and 'remove_sync'.
    fn remove_inner(&mut self, key: &Key) -> Result<Option<(String, Value)>, serde_json::Error> {
        // prepare data for write
        let mut line = file_line_of_remove(key, &mut self.cfg.integrity)?;

        // remove from the map
        if let Some(old_value) = self.map.remove(&key) {
            // user callback
            if let Some(f) = &mut self.cfg.before_write_callback {
                if let Some(transformed_line) = f(&line) {
                    line = transformed_line;
                }
            }

            // remove from indexes
            for index in self.indexes.iter() {
                index.on_remove(&key, &old_value);
            }

            return Ok(Some((line, old_value)));
        }

        Ok(None)
    }

    /// Create index by value based on std::collections::BTreeMap.
    /// 'make_index_key_callback' function is called during all operations of inserting,
    /// and deleting elements. In the function it is necessary to determine
    /// the value and type of the index key in any way related to the value of the 'BTree'.
    pub fn create_btree_index<IndexKey>(&mut self, make_index_key_callback: fn(&Value) -> IndexKey)
        -> Index<IndexKey, Key, Value, std::collections::BTreeMap<IndexKey, BTreeSet<Key>>>
    where IndexKey: Clone + Ord + 'static {
        self.create_index::<IndexKey, std::collections::BTreeMap<IndexKey, BTreeSet<Key>>>(make_index_key_callback)
    }

    /// Create index by value based on std::collections::HashMap.
    /// 'make_index_key_callback' function is called during all operations of inserting,
    /// and deleting elements. In the function it is necessary to determine
    /// the value and type of the index key in any way related to the value of the 'BTree'.
    pub fn create_hashmap_index<IndexKey>(&mut self, make_index_key_callback: fn(&Value) -> IndexKey)
        -> Index<IndexKey, Key, Value, std::collections::HashMap<IndexKey, BTreeSet<Key>>>
    where IndexKey: Clone + Hash + Eq + 'static {
        self.create_index::<IndexKey, std::collections::HashMap<IndexKey, BTreeSet<Key>>>(make_index_key_callback)
    }

    /// Create index by value.
    /// 'make_index_key_callback' function is called during all operations of inserting,
    /// and deleting elements. In the function it is necessary to determine
    /// the value and type of the index key in any way related to the value of the 'BTree'.
    pub fn create_index<IndexKey, MapOfIndex>(&mut self, make_index_key_callback: fn(&Value) -> IndexKey)
        -> Index<IndexKey, Key, Value, MapOfIndex>
    where
        IndexKey: Clone + Eq + 'static,
        MapOfIndex: MapTrait<IndexKey, BTreeSet<Key>> + Default + Sized + 'static,
    {
        let mut index_map = MapOfIndex::default();

        self.map.for_each(|key, val| {
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
        });

        let index = Index::new(index_map, make_index_key_callback);
        self.indexes.push(Box::new(index.clone()));

        index
    }

    /// Returns a reference to an used map.
    pub fn map(&self) -> &Map {
        &self.map
    }
}

#[derive(Debug)]
pub enum SyncInsertRemoveError {
    SerializeError(serde_json::Error),
    WriteError(std::io::Error),
}

impl From<serde_json::Error > for SyncInsertRemoveError {
    fn from(err: serde_json::Error) -> Self {
        SyncInsertRemoveError::SerializeError(err)
    }
}

impl From<std::io::Error> for SyncInsertRemoveError {
    fn from(err: std::io::Error) -> Self {
        SyncInsertRemoveError::WriteError(err)
    }
}

impl std::error::Error for SyncInsertRemoveError {}

impl std::fmt::Display for SyncInsertRemoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
