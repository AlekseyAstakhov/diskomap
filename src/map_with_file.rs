use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs::OpenOptions;
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

        Ok(MapWithFile {
            map,
            file_worker: FileWorker::new(file, cfg.write_error_callback.take()),
            indexes: Vec::new(),
            cfg: cfg,
        })
    }

    /// Inserts a key-value pair into the map. This function is used for updating too.
    /// Data will be written to RAM immediately, and to disk later in a separate thread.
    pub fn insert(&mut self, key: Key, value: Value) -> Result<Option<Value>, serde_json::Error> {
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

        // add operation to history file
        self.file_worker.write(line);

        // update in index
        for index in self.indexes.iter() {
            index.on_insert(key.clone(), value.clone(), old_value.clone());
        }

        Ok(old_value)
    }

    /// Returns a reference to the value corresponding to the key. No writing to the history file.
    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.map.get(key)
    }

    /// Remove value by key from the map in memory and asynchronously append operation to the file.
    pub fn remove(&mut self, key: &Key) -> Result<Option<Value>, serde_json::Error> {
        // remove from the map
        if let Some(old_value) = self.map.remove(&key) {
            // prepare data for write
            let mut line = file_line_of_remove(key, &mut self.cfg.integrity)?;

            // user callback
            if let Some(f) = &mut self.cfg.before_write_callback {
                if let Some(transformed_line) = f(&line) {
                    line = transformed_line;
                }
            }

            // add operation to history file
            self.file_worker.write(line);

            // remove from indexes
            for index in self.indexes.iter() {
                index.on_remove(&key, &old_value);
            }

            return Ok(Some(old_value.clone()));
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
