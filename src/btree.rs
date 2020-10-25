use crate::btree_index::BtreeIndex;
use crate::btree_index::IndexError;
use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::ops::Deref;
use std::panic;
use std::sync::{Arc, Mutex, RwLock};
use tempfile::tempdir;
use threadpool::ThreadPool;

/// A map based on a B-Tree with the operations log file on the disk.
/// Used in a similar way as a BTreeMap, but store to file log of operations as insert and remove
/// for restoring actual data after restart application.
/// Thread safe and clone-shareable.
#[derive(Clone)]
pub struct BTree<Key, Value> {
    /// Inner data this struct, need for Arc all fields together.
    inner: Arc<Inner<Key, Value>>,
}

/// Inner data of 'BTree', need for Arc all fields of 'BTree' together.
struct Inner<Key, Value> {
    /// Map in memory.
    map: RwLock<BTreeMap<Key, RwLock<Value>>>,
    /// Path to operations log file.
    log_file_path: Arc<String>,
    /// Opened with exclusive lock operations log file.
    log_file: Arc<Mutex<File>>,
    /// Thread pool with one thread for asynchronously append operations to the operations log file.
    thread_pool: Mutex<ThreadPool>,
    /// Created indexes.
    indexes: RwLock<Vec<Box<dyn IndexTrait<Key, Value> + Send + Sync>>>,
    /// Error handler of background thread. It's will call when error of writing to log file.
    on_background_error: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>,
}

impl<Key, Value: 'static> BTree<Key, Value>
where
    Key: Serialize + DeserializeOwned + Ord + Clone + Send + Sync + 'static,
    Value: Serialize + DeserializeOwned + Clone,
{
    /// Open/create map with 'operations_log_file'.
    /// If file is exist then load map from file.
    /// If file not is not exist then create new file.
    pub fn open_or_create(operations_log_file: &str) -> Result<Self, Error> {
        create_dirs_to_path_if_not_exist(operations_log_file)?;

        let mut file = OpenOptions::new().read(true).write(true).append(true).create(true).open(operations_log_file)?;
        file.lock_exclusive()?;

        // load current map from operations log file
        let map = match BTree::load_from_file(&mut file) {
            Ok(map) => {
                map
            }
            Err(err) => {
                file.unlock()?;
                return Err(err);
            }
        };

        Ok(BTree {
            inner: Arc::new(Inner {
                map: RwLock::new(map),
                log_file_path: Arc::new(operations_log_file.to_string()),
                log_file: Arc::new(Mutex::new(file)),
                thread_pool: Mutex::new(ThreadPool::new(1)),
                indexes: RwLock::new(Vec::new()),
                on_background_error: Arc::new(Mutex::new(None)),
            }),
        })
    }

    /// Insert value to the map in memory and asynchronously append operation to the file.
    pub fn insert(&self, key: Key, value: Value) -> Result<Option<Value>, Error> {
        let key_val_json = serde_json::to_string(&(&key, &value))?;

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
        self.write_insert_to_log_file_async(key_val_json)?;

        Ok(old_value)
    }

    /// Get value by key from the map in memory. No writing to the operations log file.
    pub fn get(&self, key: &Key) -> Result<Option<Value>, Error> {
        let map = self.inner.map.read()?;
        if let Some(val_rw) = map.get(key) {
            return Ok(Some(val_rw.read()?.clone()));
        }

        Ok(None)
    }

    /// Remove value by key from the map in memory and asynchronously append operation to the file.
    pub fn remove(&self, key: &Key) -> Result<Option<Value>, Error> {
        if let Some(old_value) = self.inner.map.write()?.remove(&key) {
            let value = old_value.read()?;

            // remove from indexes
            for index in self.inner.indexes.read()?.iter() {
                index.on_remove(&key, &value)?;
            }

            let key_json = serde_json::to_string(&key)?;
            self.write_remove_to_log_file_async(key_json)?;
            return Ok(Some(value.clone()));
        }

        Ok(None)
    }

    /// Returns `true` if the map in memory contains a value for the specified key.
    pub fn contains_key(&self, key: &Key) -> bool {
        match self.inner.map.read() {
            Ok(map) => map.contains_key(key),
            Err(err) => { dbg!(err); unreachable!(); }
        }
    }

    /// Returns cloned keys with values of sub-range of elements in the map. No writing to the operations log file.
    pub fn range<R>(&self, range: R) -> Vec<(Key, Value)>
    where
        R: std::ops::RangeBounds<Key>,
    {
        let mut key_values = vec![];
        match self.inner.map.read() {
            Ok(map) => {
                let range = map.range(range);
                for (key, val_rw) in range {
                    match val_rw.read() {
                        Ok(val) => {
                            key_values.push((key.clone(), val.clone()))
                        }
                        Err(err) => {
                            dbg!(err);
                            unreachable!();
                        }
                    }
                }
            }
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }

        key_values
    }

    /// Returns cloned keys of sub-range of elements in the map. No writing to the operations log file.
    pub fn range_keys<R>(&self, range: R) -> Vec<Key>
    where
        R: std::ops::RangeBounds<Key>,
    {
        match self.inner.map.read() {
            Ok(map) => {
                map.range(range).map(|(key, _)| key.clone()).collect()
            }
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }
    }

    /// Returns cloned values of sub-range of elements in the map. No writing to the operations log file.
    pub fn range_values<R>(&self, range: R) -> Vec<Value>
    where
        R: std::ops::RangeBounds<Key>,
    {
        let mut values = vec![];
        match self.inner.map.read() {
            Ok(map) => {
                let range = map.range(range);
                for (_, val_rw) in range {
                    match val_rw.read() {
                        Ok(val) => {
                            values.push(val.clone())
                        }
                        Err(err) => {
                            dbg!(err);
                            unreachable!();
                        }
                    }
                }
            }
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }

        values
    }

    /// Returns the number of elements in the map. No writing to the operations log file.
    pub fn len(&self) -> usize {
        match self.inner.map.read() {
            Ok(map) => map.len(),
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }
    }

    /// Returns `true` if the map contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // Load from file and process all operations and make actual map.
    pub fn load_from_file(file: &mut File) -> Result<BTreeMap<Key, RwLock<Value>>, Error> {
        let mut map = BTreeMap::new();
        let mut reader = BufReader::new(file);
        let mut line = String::with_capacity(150);
        let mut line_num = 0;
        while reader.read_line(&mut line)? > 0 {
            const MIN_LINE_LEN: usize = 4;
            if line.len() < MIN_LINE_LEN {
                return Err(Error::FileLineLengthLessThenMinimum { line_num });
            }

            match &line[..3] {
                "ins" => match serde_json::from_str(&line[4..]) {
                    Ok((key, val)) => {
                        map.insert(key, RwLock::new(val));
                    }
                    Err(err) => {
                        return Err(Error::DeserializeJsonError { err, line_num });
                    }
                },
                "rem" => match serde_json::from_str(&line[4..]) {
                    Ok(key) => {
                        map.remove(&key);
                    }
                    Err(err) => {
                        return Err(Error::DeserializeJsonError { err, line_num });
                    }
                },
                _ => {
                    return Err(Error::NoLineDefinition { line_num });
                }
            }

            line_num += 1;
            line.clear();
        }

        Ok(map)
    }

    /// Remove history from log file.
    /// To reduce the size of the log file and speed up loading into RAM.
    /// If you don't need the entire history of all operations.
    /// All current data state will be presented as 'set' records.
    /// Locks 'Self::map' with shared read access while processing.
    /// If data is big it's take some time because writes all contents to a file.
    pub fn remove_history(&self) -> Result<(), Error> {
        let map = self.inner.map.read()?;
        let tempdir = tempdir()?;
        let tmp_file_path = tempdir.path().join(self.inner.log_file_path.deref()).to_str().unwrap_or("").to_string();
        create_dirs_to_path_if_not_exist(&tmp_file_path)?;
        let mut tmp_file = OpenOptions::new().read(true).write(true).append(true).create(true).open(&tmp_file_path)?;

        // wait writing queue
        self.inner.thread_pool.lock()?.join();

        let mut log_file = self.inner.log_file.lock()?;
        // write all to tmp file
        for (key, value) in map.iter() {
            let key_val_json = serde_json::to_string(&(&key, &value))?;
            let user_line = "ins ".to_string() + &key_val_json + "\n";
            tmp_file.write_all(user_line.as_bytes())?;
        }

        drop(tmp_file);

        log_file.unlock()?;

        let reaname_res = std::fs::rename(&tmp_file_path, self.inner.log_file_path.deref());

        *log_file = OpenOptions::new().create(true).read(true).write(true).append(true)
            .open(self.inner.log_file_path.deref())?;
        log_file.lock_exclusive()?;

        if let Err(err) = reaname_res {
            return Err(Error::FileError(err));
        }

        Ok(())
    }

    /// Set error handler of background thread. It's will call when error of writing to log file.
    pub fn on_background_write_error(&self, callback: Option<impl Fn(std::io::Error) + Send + 'static>) {
        if let Ok(mut hook) = self.inner.on_background_error.try_lock() {
            *hook = match callback {
                Some(callback) => Some(Box::new(callback)),
                None => None,
            };
        } else {
            unreachable!();
        }
    }

    /// Write "insert" operation to the operations log file in background thread.
    /// Calling need blocking map. Under blocking only set task to the background thread.
    fn write_insert_to_log_file_async(&self, key_val_json: String) -> Result<(), Error> {
        let file = self.inner.log_file.clone();
        let error_callback = self.inner.on_background_error.clone();

        self.inner.thread_pool.lock()?.execute(move || {
            let user_line = "ins ".to_string() + &key_val_json + "\n";
            let res = match file.lock() {
                Ok(mut file) => file.write_all(user_line.as_bytes()),
                Err(err) => {
                    dbg!(err);
                    unreachable!();
                }
            };

            if let Err(err) = res {
                Self::call_background_error_callback_or_dbg(&error_callback, err);
            }
        });

        Ok(())
    }

    /// Write "remove" operation to the operations log file in background thread.
    /// Calling need blocking map. Under blocking only set task to the background thread.
    fn write_remove_to_log_file_async(&self, key_json: String) -> Result<(), Error> {
        let file = self.inner.log_file.clone();
        let error_hook = self.inner.on_background_error.clone();
        self.inner.thread_pool.lock()?.execute(move || {
            let user_line = "rem ".to_string() + &key_json + "\n";
            let res = match file.lock() {
                Ok(mut file) => file.write_all(user_line.as_bytes()),
                Err(err) => { dbg!(err); unreachable!(); }
            };

            if let Err(err) = res {
                Self::call_background_error_callback_or_dbg(&error_hook, err);
            }
        });

        Ok(())
    }

    fn call_background_error_callback_or_dbg(hook: &Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>, err: std::io::Error) {
        match hook.lock() {
            Ok(hook) => match hook.deref() {
                Some(hook) => {
                    if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                        hook(err);
                    })) {
                        dbg!(format!("panic in background error hook function {:?}", &err));
                    }
                }
                None => {
                    dbg!(&err);
                }
            },
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }
    }

    /// Create custom index by value.
    /// 'make_index_key_callback' function is called during all operations of inserting,
    /// and deleting elements. In the function it is necessary to determine
    /// the value and type of the index key in any way related to the value of the 'BTree'.
    pub fn create_btree_index<IndexKey, F>(&self, make_index_key_callback: F) -> BtreeIndex<IndexKey, Key, Value>
    where
        IndexKey: Clone + Ord + Send + Sync + 'static,
        F: Fn(&Value) -> IndexKey + Send + Sync + 'static,
    {
        let mut index_map: BTreeMap<IndexKey, BTreeSet<Key>> = BTreeMap::new();

        match self.inner.map.read() {
            Ok(map) => {
                for (key, val_rw) in map.iter() {
                    match val_rw.read() {
                        Ok(val) => {
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
                        Err(err) => {
                            dbg!(err);
                            unreachable!();
                        }
                    }
                }
            }
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }

        let index = BtreeIndex {
            inner: Arc::new(crate::btree_index::Inner {
                map: RwLock::new(index_map),
                make_index_key_callback: RwLock::new(Box::new(make_index_key_callback)),
            }),
        };

        match self.inner.indexes.write() {
            Ok(mut indexes) => indexes.push(Box::new(index.clone())),
            Err(err) => { dbg!(err); unreachable!(); }
        }
        index
    }

    /// Returns cloned keys of the map, in sorted order. No writing to the operations log file.
    pub fn keys(&self) -> Vec<Key> {
        match self.inner.map.read() {
            Ok(map) => {
                map.keys().cloned().collect()
            }
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }
    }

    /// Returns cloned values of the map, in sorted order. No writing to the operations log file.
    pub fn values(&self) -> Vec<Value> {
        let mut values = vec![];
        match self.inner.map.read() {
            Ok(map) => {
                let map_values = map.values();
                for val_rw in map_values {
                    match val_rw.read() {
                        Ok(val) => values.push(val.clone()),
                        Err(err) => {
                            dbg!(err);
                            unreachable!();
                        }
                    }
                }
            }
            Err(err) => {
                dbg!(err);
                unreachable!();
            }
        }

        values
    }
}

impl<Key, Value> Drop for BTree<Key, Value> {
    /// Waits for writing to the operations log file file and unlock it.
    fn drop(&mut self) {
        match self.inner.thread_pool.lock() {
            Ok(thread_pool) => { thread_pool.join(); }
            Err(err) => { dbg!(err); unreachable!(); }
        }

        match self.inner.log_file.lock() {
            Ok(file) => file.unlock().unwrap_or_else(|err| { dbg!(err); }),
            Err(err) => { dbg!(err); unreachable!(); }
        }
    }
}

/// Error of operations log file.
#[derive(Debug)]
pub enum Error {
    /// Error of working with file.
    FileError(std::io::Error),
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
    IndexError,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::FileError(err)
    }
}

impl From<serde_json::error::Error> for Error {
    fn from(err: serde_json::error::Error) -> Self {
        Error::JsonSerializeError(err)
    }
}

// For op-?, "auto" type conversion.
impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Error::PoisonError
    }
}

impl From<IndexError> for Error {
    fn from(_: IndexError) -> Self {
        Error::IndexError
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

/// Custom index. 'BTree' contains this dyn traits and use when insert or delete elements for update indexes.
pub(crate) trait IndexTrait<BTreeKey, BTreeValue> {
    /// Updates index when insert or update operation on 'BTree'.
    fn on_insert(&self, key: BTreeKey, value: BTreeValue, old_value: Option<BTreeValue>) -> Result<(), IndexError>;
    /// Updates index when remove operation on 'BTree'.
    fn on_remove(&self, key: &BTreeKey, value: &BTreeValue) -> Result<(), IndexError>;
}

// create dirs to path if not exist
fn create_dirs_to_path_if_not_exist(path_to_file: &str) -> Result<(), std::io::Error> {
    if let Some(index) = path_to_file.rfind('/') {
        let dir_path = &path_to_file[..index];
        if !std::path::Path::new(dir_path).exists() {
            std::fs::create_dir_all(&path_to_file[..index])?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Bound::{Excluded, Included};
    use tempfile::tempdir;

    #[test]
    fn test() -> Result<(), Box<dyn std::error::Error>> {
        // new log file
        let log_file = tempdir()?.path().join("test.txt").to_str().unwrap().to_string();
        {
            let map = BTree::open_or_create(&log_file)?;
            map.insert((), ())?;
        }
        // after restart
        {
            let map = BTree::open_or_create(&log_file)?;
            assert_eq!(Some(()), map.get(&())?);
            map.insert((), ())?;
            assert_eq!(1, map.len());
            map.insert((), ())?;
            assert_eq!(1, map.len());
            map.insert((), ())?;
            assert_eq!(1, map.len());
            assert_eq!(Some(()), map.get(&())?);
            map.remove(&())?;
            assert_eq!(0, map.len());
        }

        // new log file
        let log_file = tempdir()?.path().join("test2.txt").to_str().unwrap().to_string();
        {
            let map = BTree::open_or_create(&log_file)?;
            map.insert("key 1".to_string(), 1)?;
            map.insert("key 2".to_string(), 2)?;
            map.insert("key 3".to_string(), 3)?;
            map.insert("key 4".to_string(), 4)?;
            map.insert("key 5".to_string(), 5)?;
            assert_eq!(5, map.len());
            assert_eq!(Some(3), map.get(&"key 3".to_string())?);
            map.remove(&"key 1".to_string())?;
            map.remove(&"key 4".to_string())?;
            map.insert("key 6".to_string(), 6)?;
            map.insert("key 1".to_string(), 100)?;
            map.remove(&"key 2".to_string())?;
            map.insert("key 7".to_string(), 7)?;
            assert_eq!(map.keys(), vec!["key 1".to_string(), "key 3".to_string(), "key 5".to_string(), "key 6".to_string(), "key 7".to_string()]);
            assert_eq!(map.values(), vec![100, 3, 5, 6, 7]);
            assert_eq!(map.range_values((Included(&"key 3".to_string()), Included(&"key 6".to_string()))), vec![3, 5, 6]);
            assert_eq!(map.range_keys((Included(&"key 3".to_string()), Included(&"key 5".to_string()))), vec!["key 3".to_string(), "key 5".to_string()]);
        }
        // after restart
        {
            let map = BTree::open_or_create(&log_file)?;
            assert_eq!(5, map.len());
            assert_eq!(Some(100), map.get(&"key 1".to_string())?);
            assert_eq!(None, map.get(&"key 4".to_string())?);
            assert_eq!(None, map.get(&"key 2".to_string())?);
            map.insert("key 3".to_string(), 33)?;
            assert_eq!(Some(33), map.get(&"key 3".to_string())?);
            map.remove(&"key 1".to_string())?;
        }
        // after restart
        {
            let map = BTree::open_or_create(&log_file)?;
            assert_eq!(4, map.len());
            assert_eq!(Some(33), map.get(&"key 3".to_string())?);
            assert_eq!(None, map.get(&"key 1".to_string())?);
            assert_eq!(map.keys(), vec!["key 3".to_string(), "key 5".to_string(), "key 6".to_string(), "key 7".to_string()]);
            assert_eq!(map.values(), vec![33, 5, 6, 7]);
            assert_eq!(map.range((Excluded(&"key 3".to_string()), Excluded(&"key 6".to_string()))), vec![("key 5".to_string(), 5)]);
        }

        Ok(())
    }
}
