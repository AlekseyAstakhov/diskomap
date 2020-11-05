use crate::btree_index::BtreeIndex;
use crate::btree_index::BtreeIndexError;
use crate::file_worker::FileWorker;
use crate::file_work::{load_from_file, write_insert_to_file, create_dirs_to_path_if_not_exist};
use crate::Integrity;
use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, RwLock};
use tempfile::tempdir;

/// A map based on a B-Tree with the operations log file on the disk.
/// Used in a similar way as a BTreeMap, but store to file log of operations as insert and remove
/// for restoring actual data after restart application.
/// Thread safe and clone-shareable.
#[derive(Clone)]
pub struct BTree<Key, Value> {
    /// Inner data this struct, need for Arc all fields together.
    inner: Arc<Inner<Key, Value>>,
}

/// Inner data of 'BTree', need for clone all fields of 'BTree' together.
struct Inner<Key, Value> {
    /// Map in the RAM.
    map: RwLock<BTreeMap<Key, RwLock<Value>>>,
    /// Path to operations log file.
    file_path: String,
    // For append operations to the operations log file in background thread.
    file_worker: Mutex<FileWorker>,

    /// Created indexes.
    indexes: RwLock<Vec<Box<dyn IndexTrait<Key, Value> + Send + Sync>>>,
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
        file.lock_exclusive()?;

        let integrity = Arc::new(Mutex::new(integrity));

        // load current map from operations log file
        let map = match load_from_file(&mut file, &mut integrity.lock()?.deref_mut()) {
            Ok(map) => {
                map
            }
            Err(err) => {
                file.unlock()?;
                return Err(err);
            }
        };

        let on_background_error = Arc::new(Mutex::new(None));

        Ok(BTree {
            inner: Arc::new(Inner {
                map: RwLock::new(map),
                file_path: file_path.to_string(),
                file_worker: Mutex::new(FileWorker::new(file, on_background_error.clone())),
                indexes: RwLock::new(Vec::new()),
                on_background_error: on_background_error,
                integrity,
            }),
        })
    }

    /// Inserts a key-value pair into the map. This function is used for updating too.
    /// Data will be written to RAM immediately, and to disk later in a separate thread.
    pub fn insert(&self, key: Key, value: Value) -> Result<Option<Value>, BTreeError> {
        let updated_value = match self.inner.map.read()?.get(&key) {
            // if the value exists, then try to update it
            Some(cur_value) => {
                let mut cur_value = cur_value.write()?;
                let old_value = (*cur_value).clone();
                *cur_value = value.clone();
                Some(old_value)
            }
            None => {
                None
            }
        };

        let old_value = match updated_value {
            Some(updated_value) => Some(updated_value),
            None => {
                // if the value not exists, then inset new
                let old = self.inner.map.write()?.insert(key.clone(), RwLock::new(value.clone()));
                match old {
                    Some(old) => Some(old.read()?.clone()),
                    None => None,
                }
            }
        };

        // update in index
        for index in self.inner.indexes.read()?.iter() {
            index.on_insert(key.clone(), value.clone(), old_value.clone())?;
        }

        // add operation to operations log file
        let key_val_json = serde_json::to_string(&(&key, &value))?;
        let integrity = self.inner.integrity.clone();
        self.inner.file_worker.lock()?.write_insert(key_val_json, integrity).unwrap();

        Ok(old_value)
    }

    /// Get value by key from the map in RAM. No writing to the operations log file.
    pub fn get(&self, key: &Key) -> Result<Option<Value>, BTreeError> {
        let map = self.inner.map.read()?;
        if let Some(val_rw) = map.get(key) {
            return Ok(Some(val_rw.read()?.clone()));
        }

        Ok(None)
    }

    /// Remove value by key from the map in memory and asynchronously append operation to the file.
    pub fn remove(&self, key: &Key) -> Result<Option<Value>, BTreeError> {
        if let Some(old_value) = self.inner.map.write()?.remove(&key) {
            let value = old_value.read()?;

            // remove from indexes
            for index in self.inner.indexes.read()?.iter() {
                index.on_remove(&key, &value)?;
            }

            let key_json = serde_json::to_string(&key)?;
            let integrity = self.inner.integrity.clone();
            self.inner.file_worker.lock()?.write_remove(key_json, integrity).unwrap();

            return Ok(Some(value.clone()));
        }

        Ok(None)
    }

    /// Returns `true` if the map in memory contains a value for the specified key.
    pub fn contains_key(&self, key: &Key) -> Result<bool, BTreeError> {
        Ok(self.inner.map.read()?.contains_key(key))
    }

    /// Returns cloned keys with values of sub-range of elements in the map. No writing to the operations log file.
    pub fn range<R>(&self, range: R) -> Result<Vec<(Key, Value)>, BTreeError>
    where
        R: std::ops::RangeBounds<Key>,
    {
        let mut key_values = vec![];
        let map = self.inner.map.read()?;
        let range = map.range(range);
        for (key, val) in range {
            key_values.push((key.clone(), val.read()?.clone()))
        }

        Ok(key_values)
    }

    /// Returns cloned keys of sub-range of elements in the map. No writing to the operations log file.
    pub fn range_keys<R>(&self, range: R) -> Result<Vec<Key>, BTreeError>
    where
        R: std::ops::RangeBounds<Key>,
    {
        Ok(self.inner.map.read()?.range(range).map(|(key, _)| key.clone()).collect())
    }

    /// Returns cloned values of sub-range of elements in the map. No writing to the operations log file.
    pub fn range_values<R>(&self, range: R) -> Result<Vec<Value>, BTreeError>
    where
        R: std::ops::RangeBounds<Key>,
    {
        let mut values = vec![];
        let map = self.inner.map.read()?;
        let range = map.range(range);
        for (_, val) in range {
            values.push(val.read()?.clone())
        }

        Ok(values)
    }

    /// Returns the number of elements in the map. No writing to the operations log file.
    pub fn len(&self) -> Result<usize, BTreeError> {
        Ok(self.inner.map.read()?.len())
    }

    /// Returns `true` if the map contains no elements.
    pub fn is_empty(&self) -> Result<bool, BTreeError> {
        Ok(self.len()? == 0)
    }

    /// Remove history from log file.
    /// To reduce the size of the log file and speed up loading into RAM.
    /// If you don't need the entire history of all operations.
    /// All current data state will be presented as 'set' records.
    /// Locks 'Self::map' with shared read access while processing.
    /// If data is big it's take some time because writes all contents to a file.
    pub fn remove_history(&self, integrity: Option<Integrity>) -> Result<(), BTreeError> {
        let map = self.inner.map.read()?;
        let tempdir = tempdir()?;
        let tmp_file_path = tempdir.path().join(self.inner.file_path.deref()).to_str().unwrap_or("").to_string();

        create_dirs_to_path_if_not_exist(&tmp_file_path)?;
        let mut tmp_file = OpenOptions::new().read(true).write(true).append(true).create(true).open(&tmp_file_path)?;
        let mut integrity = integrity;

        let mut file_worker = self.inner.file_worker.lock()?;
        // here waiting for worker queue

        // write all to tmp file
        for (key, value) in map.iter() {
            let key_val_json = serde_json::to_string(&(&key, &value))?;
            write_insert_to_file(&key_val_json, &mut tmp_file, &mut integrity)?;
        }

        drop(tmp_file);

        let reaname_res = std::fs::rename(&tmp_file_path, self.inner.file_path.deref());

        let reopened_file = OpenOptions::new().create(true).read(true).write(true).append(true)
            .open(self.inner.file_path.deref())?;
        *file_worker = FileWorker::new(reopened_file, self.inner.on_background_error.clone());

        if let Err(err) = reaname_res {
            return Err(BTreeError::FileError(err));
        }

        *self.inner.integrity.lock()? = integrity;

        Ok(())
    }

    /// Create custom index by value.
    /// 'make_index_key_callback' function is called during all operations of inserting,
    /// and deleting elements. In the function it is necessary to determine
    /// the value and type of the index key in any way related to the value of the 'BTree'.
    pub fn create_btree_index<IndexKey, F>(&self, make_index_key_callback: F)
        -> Result<BtreeIndex<IndexKey, Key, Value>, BtreeIndexError>
    where
        IndexKey: Clone + Ord + Send + Sync + 'static,
        F: Fn(&Value) -> IndexKey + Send + Sync + 'static,
    {
        let mut index_map: BTreeMap<IndexKey, BTreeSet<Key>> = BTreeMap::new();

        { // lock
            let map = self.inner.map.read()?;
            for (key, val_rw) in map.iter() {
                let val = val_rw.read()?;
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
        } // unlock

        let index = BtreeIndex {
            inner: Arc::new(crate::btree_index::Inner {
                map: RwLock::new(index_map),
                make_index_key_callback: RwLock::new(Box::new(make_index_key_callback)),
            }),
        };

        self.inner.indexes.write()?.push(Box::new(index.clone()));

        Ok(index)
    }

    /// Returns cloned keys of the map, in sorted order. No writing to the operations log file.
    pub fn keys(&self) -> Result<Vec<Key>, BTreeError> {
        Ok(self.inner.map.read()?.keys().cloned().collect())
    }

    /// Returns cloned values of the map, in sorted order. No writing to the operations log file.
    pub fn values(&self) -> Result<Vec<Value>, BTreeError> {
        let mut values = vec![];
        let map = self.inner.map.read()?;
        for val in map.values() {
            values.push(val.read()?.clone());
        }

        Ok(values)
    }
}

/// Errors when working with BTree.
#[derive(Debug)]
pub enum BTreeError {
    /// Error of working with file.
    FileError(std::io::Error),
    /// There is no expected checksum or hash in the log file line when integrity used.
    NoExpectedHash { line_num: usize, },
    /// Wrong Sha256 of log file line data when crc32 integrity used.
    WrongSha256Blockchain { line_num: usize, },
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

/// Custom index. 'BTree' contains this dyn traits and use when insert or delete elements for update indexes.
pub(crate) trait IndexTrait<BTreeKey, BTreeValue> {
    /// Updates index when insert or update operation on map.
    fn on_insert(&self, key: BTreeKey, value: BTreeValue, old_value: Option<BTreeValue>) -> Result<(), BtreeIndexError>;
    /// Updates index when remove operation on map.
    fn on_remove(&self, key: &BTreeKey, value: &BTreeValue) -> Result<(), BtreeIndexError>;
}
