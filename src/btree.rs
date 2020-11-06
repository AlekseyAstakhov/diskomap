use crate::index::{IndexTrait, BtreeIndexError};
use crate::btree_index::BtreeIndex;
use crate::file_worker::FileWorker;
use crate::file_work::{load_from_file, ins_file_line, create_dirs_to_path_if_not_exist};
use crate::Integrity;
use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, RwLock};
use tempfile::tempdir;
use std::io::Write;

/// A map based on a B-Tree with the operations log file on the disk.
/// Used in a similar way as a BTreeMap, but store to file log of operations as insert and remove
/// for restoring actual data after restart application.
pub struct BTree<Key, Value> {
    /// Map in the RAM.
    map: BTreeMap<Key, Value>,
    /// Path to operations log file.
    file_path: String,
    // For append operations to the operations log file in background thread.
    file_worker: Option<FileWorker>,

    /// Created indexes.
    indexes: Vec<Box<dyn IndexTrait<Key, Value> + Send + Sync>>,
    /// Error handler of background thread. It's will call when error of writing to log file.
    on_background_error: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>,
    /// Mechanism of controlling the integrity of stored data in a log file.
    integrity: Arc<Mutex<Option<Integrity>>>,
}

impl<Key, Value: 'static> BTree<Key, Value>
where
    Key: Serialize + DeserializeOwned + Ord + Clone + Send + Sync + 'static,
    Value: Serialize + DeserializeOwned + Clone,
{
    /// Open/create map with 'operations_log_file'.
    /// If file is exist then load map from file.
    /// If file not is not exist then create new file.
    pub fn open_or_create(file_path: &str, integrity: Option<Integrity>) -> Result<Self, BTreeError> {
        create_dirs_to_path_if_not_exist(file_path)?;

        let mut file = OpenOptions::new().read(true).write(true).append(true).create(true).open(file_path)?;

        let integrity = Arc::new(Mutex::new(integrity));
        let mut locked_integrity = integrity.lock()?;

        file.lock_exclusive()?;

        // load current map from operations log file
        let map = match load_from_file(&mut file, &mut locked_integrity.deref_mut()) {
            Ok(map) => {
                map
            }
            Err(err) => {
                file.unlock()?;
                return Err(err);
            }
        };

        let on_background_error = Arc::new(Mutex::new(None));

        drop(locked_integrity);

        Ok(BTree {
            map,
            file_path: file_path.to_string(),
            file_worker: Some(FileWorker::new(file, on_background_error.clone())),
            indexes: Vec::new(),
            on_background_error: on_background_error,
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
        let integrity = self.integrity.clone();

        if let Some(file_worker) = &self.file_worker {
            file_worker.write_insert(key_val_json, integrity);
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
            let integrity = self.integrity.clone();

            if let Some(file_worker) = &self.file_worker {
                file_worker.write_remove(key_json, integrity);
            } else {
                unreachable!();
            }

            return Ok(Some(old_value.clone()));
        }

        Ok(None)
    }

    /// Remove history from log file.
    /// To reduce the size of the log file and speed up loading into RAM.
    /// If you don't need the entire history of all operations.
    /// All current data state will be presented as 'set' records.
    /// Locks 'Self::map' with shared read access while processing.
    /// If data is big it's take some time because writes all contents to a file.
    pub fn remove_history(&mut self, integrity: Option<Integrity>) -> Result<(), BTreeError> {
        let tempdir = tempdir()?;
        let tmp_file_path = tempdir.path().join(self.file_path.deref()).to_str().unwrap_or("").to_string();

        create_dirs_to_path_if_not_exist(&tmp_file_path)?;
        let mut tmp_file = OpenOptions::new().read(true).write(true).append(true).create(true).open(&tmp_file_path)?;
        let mut integrity = integrity;

        // here waiting for worker queue
        drop(self.file_worker.take());

        // write all to tmp file
        for (key, value) in self.map.iter() {
            let key_val_json = serde_json::to_string(&(&key, &value))?;
            let line = ins_file_line(&key_val_json, &mut integrity);
            tmp_file.write_all(line.as_bytes())?;
        }

        drop(tmp_file);

        let reaname_res = std::fs::rename(&tmp_file_path, &self.file_path);

        let reopened_file = OpenOptions::new().create(true).read(true).write(true).append(true)
            .open(self.file_path.deref())?;

        self.file_worker = Some(FileWorker::new(reopened_file, self.on_background_error.clone()));

        if let Err(err) = reaname_res {
            return Err(BTreeError::FileError(err));
        }

        *self.integrity.lock()? = integrity;

        Ok(())
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

    /// Returns cloned keys with values of sub-range of elements in the map. No writing to the operations log file.
    pub fn range<R>(&self, range: R) -> Result<Vec<(Key, Value)>, BTreeError>
        where
            R: std::ops::RangeBounds<Key>,
    {
        let mut key_values = vec![];
        let range = self.map.range(range);
        for (key, val) in range {
            key_values.push((key.clone(), val.clone()))
        }

        Ok(key_values)
    }

    /// Returns cloned keys of sub-range of elements in the map. No writing to the operations log file.
    pub fn range_keys<R>(&self, range: R) -> Vec<Key>
        where
            R: std::ops::RangeBounds<Key>,
    {
        self.map.range(range).map(|(key, _)| key.clone()).collect()
    }

    /// Returns cloned values of sub-range of elements in the map. No writing to the operations log file.
    pub fn range_values<R>(&self, range: R) -> Vec<Value>
        where
            R: std::ops::RangeBounds<Key>,
    {
        self.map.range(range).map(|(_, val)| val.clone()).collect()
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
    /// Error of working with file.
    FileError(std::io::Error),
    /// There is no expected checksum or hash in the log file line when integrity used.
    NoExpectedHash { line_num: usize, },
    /// Wrong Sha256 of log file line data when Sha256 blockchain integrity used.
    WrongSha256Chain { line_num: usize, },
    /// Wrong crc32 of log file line data when crc32 integrity used.
    WrongCrc32 { line_num: usize, },
    /// Json error with line number in operations log file.
    DeserializeJsonError { err: serde_json::Error, line_num: usize, },
    /// When line length in operations log file less then need.
    FileLineLengthLessThenMinimum { line_num: usize, },
    /// Line in operations log file no contains operation name as "ins" or "rem".
    NoLineDefinition { line_num: usize, },
    /// Json error with line number in operations log file.
    JsonSerializeError(serde_json::Error),
    /// Lock error.
    PoisonError,
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

// For op-?, "auto" type conversion.
impl<T> From<std::sync::PoisonError<T>> for BTreeError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        BTreeError::PoisonError
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
