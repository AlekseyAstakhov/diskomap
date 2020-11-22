#[cfg(test)]
mod tests {
    use crate::{BTreeMap, Integrity};
    use crate::cfg::Cfg;
    use std::io::Write;
    use crate::file_work::{LoadFileError, MapOperation, IntegrityError};
    use crate::map_with_file::HashMap;
    use uuid::Uuid;
    use crate::cfg::Format;

    #[test]
    fn common() -> Result<(), Box<dyn std::error::Error>> {
        // new file
        let file = tmp_file()?;
        let mut map = BTreeMap::open_or_create(&file, Cfg::default())?;
        map.insert((), ())?;
        drop(map);

        // after restart
        let mut map = BTreeMap::open_or_create(&file, Cfg::default())?;
        assert_eq!(Some(&()), map.get(&()));
        map.insert((), ())?;
        assert_eq!(1, map.map().len());
        map.insert((), ())?;
        assert_eq!(1, map.map().len());
        map.insert((), ())?;
        assert_eq!(1, map.map().len());
        assert_eq!(Some(&()), map.map().get(&()));
        map.remove(&())?;
        assert_eq!(0, map.map().len());
        drop(map);

        // new log file
        let file = tmp_file()?;
        let mut map = BTreeMap::open_or_create(&file, Cfg::default())?;
        map.insert("key 1".to_string(), 1)?;
        map.insert("key 2".to_string(), 2)?;
        map.insert("key 3".to_string(), 3)?;
        map.insert("key 4".to_string(), 4)?;
        map.insert("key 5".to_string(), 5)?;
        assert_eq!(5, map.map().len());
        assert_eq!(Some(&3), map.get(&"key 3".to_string()));
        map.remove(&"key 1".to_string())?;
        map.remove(&"key 4".to_string())?;
        map.insert("key 6".to_string(), 6)?;
        map.insert("key 1".to_string(), 100)?;
        map.remove(&"key 2".to_string())?;
        map.insert("key 7".to_string(), 7)?;
        let keys = map.map().keys().cloned().collect::<Vec<String>>();
        assert_eq!(keys, vec!["key 1".to_string(), "key 3".to_string(), "key 5".to_string(), "key 6".to_string(), "key 7".to_string()]);
        let values = map.map().values().cloned().collect::<Vec<i32>>();
        assert_eq!(values, vec![100, 3, 5, 6, 7]);
        drop(map);

        // after restart
        let mut map = BTreeMap::open_or_create(&file, Cfg::default())?;
        assert_eq!(5, map.map().len());
        assert_eq!(Some(&100), map.get(&"key 1".to_string()));
        assert_eq!(None, map.get(&"key 4".to_string()));
        assert_eq!(None, map.get(&"key 2".to_string()));
        map.insert("key 3".to_string(), 33)?;
        assert_eq!(Some(&33), map.get(&"key 3".to_string()));
        map.remove(&"key 1".to_string())?;
        drop(map);

        // after restart
        let map = BTreeMap::open_or_create(&file, Cfg::default())?;
        assert_eq!(4, map.map().len());
        assert_eq!(Some(&33), map.get(&"key 3".to_string()));
        assert_eq!(None, map.get(&"key 1".to_string()));
        let keys = map.map().keys().cloned().collect::<Vec<String>>();
        assert_eq!(keys, vec!["key 3".to_string(), "key 5".to_string(), "key 6".to_string(), "key 7".to_string()]);
        let values = map.map().values().cloned().collect::<Vec<i32>>();
        assert_eq!(values, vec![33, 5, 6, 7]);
        drop(map);

        Ok(())
    }

    #[test]
    fn btree_index() -> Result<(), Box<dyn std::error::Error>> {
        use serde::{Deserialize, Serialize};
        use crate::cfg::Cfg;

        #[derive(Clone, Serialize, Deserialize)]
        struct User {
            name: String,
            age: u8,
        }

        // new log file
        let file = tmp_file()?;
        let mut map = crate::BTreeMap::open_or_create(&file, Cfg::default())?;
        let user_name_index = map.create_btree_index(|value: &User| value.name.clone());

        map.insert(0, User { name: "Mary".to_string(), age: 21 })?;
        map.insert(1, User { name: "John".to_string(), age: 37 })?;

        assert_eq!(user_name_index.get(&"John".to_string()), vec![1]);
        assert!(user_name_index.get(&"Masha".to_string()).is_empty());

        map.insert(3, User { name: "Masha".to_string(), age: 27 })?;
        map.insert(0, User { name: "Natasha".to_string(), age: 23 })?;

        assert_eq!(user_name_index.get(&"Natasha".to_string()), vec![0]);
        assert!(user_name_index.get(&"Mary".to_string()).is_empty());
        assert_eq!(user_name_index.get(&"Masha".to_string()), vec![3]);
        map.insert(5, User { name: "Natasha".to_string(), age: 25 })?;

        map.insert(1, User { name: "Bob".to_string(), age: 27 })?;
        assert!(user_name_index.get(&"John".to_string()).is_empty());

        map.remove(&1)?;
        assert!(user_name_index.get(&"Bob".to_string()).is_empty());

        map.insert(8, User { name: "Masha".to_string(), age: 23 })?;
        map.insert(12, User { name: "Masha".to_string(), age: 24 })?;
        assert_eq!(user_name_index.get(&"Masha".to_string()), vec![3, 8, 12]);

        map.insert(8, User { name: "Natasha".to_string(), age: 35 })?;
        assert_eq!(user_name_index.get(&"Masha".to_string()), vec![3, 12]);
        assert_eq!(user_name_index.get(&"Natasha".to_string()), vec![0, 5, 8]);

        Ok(())
    }

    #[test]
    fn hashmap_index() -> Result<(), Box<dyn std::error::Error>> {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Serialize, Deserialize)]
        struct User {
            name: String,
            age: u8,
        }

        // new log file
        let file = tmp_file()?;
        let mut map = crate::BTreeMap::open_or_create(&file, Cfg::default())?;
        let user_name_index = map.create_hashmap_index(|value: &User| value.name.clone());

        map.insert(0, User { name: "Mary".to_string(), age: 21 })?;
        map.insert(1, User { name: "John".to_string(), age: 37 })?;

        assert_eq!(user_name_index.get(&"John".to_string()), vec![1]);
        assert!(user_name_index.get(&"Masha".to_string()).is_empty());

        map.insert(3, User { name: "Masha".to_string(), age: 27 })?;
        map.insert(0, User { name: "Natasha".to_string(), age: 23 })?;

        assert_eq!(user_name_index.get(&"Natasha".to_string()), vec![0]);
        assert!(user_name_index.get(&"Mary".to_string()).is_empty());
        assert_eq!(user_name_index.get(&"Masha".to_string()), vec![3]);
        map.insert(5, User { name: "Natasha".to_string(), age: 25 })?;

        map.insert(1, User { name: "Bob".to_string(), age: 27 })?;
        assert!(user_name_index.get(&"John".to_string()).is_empty());

        map.remove(&1)?;
        assert!(user_name_index.get(&"Bob".to_string()).is_empty());

        map.insert(8, User { name: "Masha".to_string(), age: 23 })?;
        map.insert(12, User { name: "Masha".to_string(), age: 24 })?;
        assert_eq!(user_name_index.get(&"Masha".to_string()), vec![3, 8, 12]);

        map.insert(8, User { name: "Natasha".to_string(), age: 35 })?;
        assert_eq!(user_name_index.get(&"Masha".to_string()), vec![3, 12]);
        assert_eq!(user_name_index.get(&"Natasha".to_string()), vec![0, 5, 8]);

        Ok(())
    }

    #[test]
    fn crc32_integrity() -> Result<(), Box<dyn std::error::Error>> {
        use crate::Integrity;
        use crate::BTreeMap;
        use std::fs::OpenOptions;
        use crate::cfg::*;

        let mut cfg = Cfg::default();
        cfg.integrity = Some(Integrity::Crc32);
        let file = tmp_file()?;
        let mut map = BTreeMap::open_or_create(&file, cfg)?;
        map.insert(0, "a".to_string())?;
        map.insert(3, "b".to_string())?;
        map.insert(5, "c".to_string())?;
        drop(map);
        let file_content = std::fs::read_to_string(&file)?;
        let expected_content = "ins [0,\"a\"] 1874290170\nins [3,\"b\"] 3949308173\nins [5,\"c\"] 1023287335\n";
        assert_eq!(file_content, expected_content);

        let mut cfg = Cfg::default();
        cfg.integrity = Some(Integrity::Crc32);
        let mut map: HashMap<i32, String> = HashMap::open_or_create(&file, cfg)?;
        map.remove(&3)?;
        drop(map);
        let file_content = std::fs::read_to_string(&file)?;
        let expected_content = "ins [0,\"a\"] 1874290170\nins [3,\"b\"] 3949308173\nins [5,\"c\"] 1023287335\nrem 3 596860484\n";
        assert_eq!(file_content, expected_content);

        let mut f = OpenOptions::new().read(true).write(true).create(true).open(&file)?;
        // wrong crc 3949338173
        let bad_content = "ins [0,\"a\"] 1874290170\nins [3,\"b\"] 3949338173\nins [5,\"c\"] 1023287335\n";
        f.write_all(bad_content.as_bytes())?;
        drop(f);

        let mut cfg = Cfg::default();
        cfg.integrity = Some(Integrity::Crc32);
        let res: Result<BTreeMap<i32, String>, LoadFileError> = BTreeMap::open_or_create(&file, cfg);
        let mut crc_is_correct = true;
        if let Err(res) = res {
            if let LoadFileError::IntegrityError(err) = res {
                if let IntegrityError::Crc32Error { line_num } = err {
                    if line_num == 2 {
                        crc_is_correct = false;
                    }
                }
            }
        }
        assert!(!crc_is_correct);

        Ok(())
    }

    #[test]
    fn sha1_chain_integrity() -> Result<(), Box<dyn std::error::Error>> {
        use crate::Integrity;
        use crate::BTreeMap;
        use std::fs::OpenOptions;

        let inital_hash = [0,2,4,56,32,6,6,23,34,32,1,234,115,141,153,20,34,50,01,45];

        let file = tmp_file()?;
        let mut cfg = Cfg::default();
        cfg.integrity = Some(Integrity::Sha1Chain(inital_hash.clone()));
        let mut map = BTreeMap::open_or_create(&file, cfg)?;
        map.insert(0, "a".to_string())?;
        map.insert(3, "b".to_string())?;
        map.insert(5, "c".to_string())?;
        drop(map);

        let file_content = std::fs::read_to_string(&file)?;
        let expected = "ins [0,\"a\"] d89086c29dac4f39a47d05aed7f78a2b310cd82d\n\
                              ins [3,\"b\"] 7add20016461fb0e9d8ed53abca6912cb30cbd15\n\
                              ins [5,\"c\"] ed9f607342b112c6dc8b6136f0d405dd1ef946de\n";
        assert_eq!(file_content, expected);

        let mut cfg = Cfg::default();
        cfg.integrity = Some(Integrity::Sha1Chain(inital_hash.clone()));
        let mut map: BTreeMap<i32, String> = BTreeMap::open_or_create(&file, cfg)?;
        map.remove(&3)?;
        drop(map);

        let file_content = std::fs::read_to_string(&file)?;
        let expected = "ins [0,\"a\"] d89086c29dac4f39a47d05aed7f78a2b310cd82d\n\
                              ins [3,\"b\"] 7add20016461fb0e9d8ed53abca6912cb30cbd15\n\
                              ins [5,\"c\"] ed9f607342b112c6dc8b6136f0d405dd1ef946de\n\
                              rem 3 40bfdfd88c6a74e36b07c21abbd87decb1062e1e\n";
        assert_eq!(file_content, expected);

        let mut f = OpenOptions::new().read(true).write(true).create(true).open(&file)?;
        // wrong 7add20016461fb3e9d8ed53abca6912cb30cbd15
        let bad_content = "ins [0,\"a\"] d89086c29dac4f39a47d05aed7f78a2b310cd82d\n\
                              ins [3,\"b\"] 7add20016461fb3e9d8ed53abca6912cb30cbd15\n\
                              ins [5,\"c\"] ed9f607342b112c6dc8b6136f0d405dd1ef946de\n";

        f.write_all(bad_content.as_bytes())?;
        drop(f);

        let mut cfg = Cfg::default();
        cfg.integrity = Some(Integrity::Sha1Chain(inital_hash.clone()));
        let res: Result<HashMap<i32, String>, LoadFileError> = HashMap::open_or_create(&file, cfg);
        let mut crc_is_correct = true;
        if let Err(res) = res {
            if let LoadFileError::IntegrityError(err) = res {
                if let IntegrityError::Sha1ChainError { line_num } = err {
                    if line_num == 2 {
                        crc_is_correct = false;
                    }
                }
            }
        }
        assert!(!crc_is_correct);

        Ok(())
    }

    #[test]
    fn sha256_chain_integrity() -> Result<(), Box<dyn std::error::Error>> {
        use crate::Integrity;
        use std::fs::OpenOptions;

        let inital_hash = [0,2,1,234,115,141,153,20,34,56,32,115,141,153,20,34,50,01,45,6,23,34,32,1,234,141,153,20,34,50,01,45];

        let file = tmp_file()?;
        let mut cfg = Cfg::default();
        cfg.integrity = Some(Integrity::Sha256Chain(inital_hash.clone()));
        let mut map = BTreeMap::open_or_create(&file, cfg)?;
        map.insert(0, "a".to_string())?;
        map.insert(3, "b".to_string())?;
        map.insert(5, "c".to_string())?;
        drop(map);

        let file_content = std::fs::read_to_string(&file)?;
        let expected = "ins [0,\"a\"] 54337bc91f5e7ff1fff6ef55c341c95112cfba4ae0fc6b5a0f38fc1271cc30ba\n\
                              ins [3,\"b\"] 792abea8afabf421de44af6aa458d6123d4245b401ecac931066ea3cd1c938f5\n\
                              ins [5,\"c\"] ce8434b92d512311b5c0cceaaf93305b74e7c740f0a342f94f2488a25b792b2a\n";

        assert_eq!(file_content, expected);

        let mut cfg = Cfg::default();
        cfg.integrity = Some(Integrity::Sha256Chain(inital_hash.clone()));
        let mut map: BTreeMap<i32, String> = BTreeMap::open_or_create(&file, cfg)?;
        map.remove(&3)?;
        drop(map);
        let file_content = std::fs::read_to_string(&file)?;
        let expected = "ins [0,\"a\"] 54337bc91f5e7ff1fff6ef55c341c95112cfba4ae0fc6b5a0f38fc1271cc30ba\n\
                              ins [3,\"b\"] 792abea8afabf421de44af6aa458d6123d4245b401ecac931066ea3cd1c938f5\n\
                              ins [5,\"c\"] ce8434b92d512311b5c0cceaaf93305b74e7c740f0a342f94f2488a25b792b2a\n\
                              rem 3 e60936f0133f6f27df7b5521c0b792d6467ed07e19a0919ad3ece9d8be84913d\n";

        assert_eq!(file_content, expected);

        let mut f = OpenOptions::new().read(true).write(true).create(true).open(&file)?;
        // wrong 792abea8af3bf421de44af6aa458d6123d4245b401ecac931066ea3cd1c938f5
        let bad_content = "ins [0,\"a\"] 54337bc91f5e7ff1fff6ef55c341c95112cfba4ae0fc6b5a0f38fc1271cc30ba\n\
                              ins [3,\"b\"] 792abea8af3bf421de44af6aa458d6123d4245b401ecac931066ea3cd1c938f5\n\
                              ins [5,\"c\"] ce8434b92d512311b5c0cceaaf93305b74e7c740f0a342f94f2488a25b792b2a\n";

        f.write_all(bad_content.as_bytes())?;
        drop(f);

        let mut cfg = Cfg::default();
        cfg.integrity = Some(Integrity::Sha256Chain(inital_hash.clone()));
        let res: Result<HashMap<i32, String>, LoadFileError> = HashMap::open_or_create(&file, cfg);
        let mut crc_is_correct = true;
        if let Err(res) = res {
            if let LoadFileError::IntegrityError(err) = res {
                if let IntegrityError::Sha256ChainError { line_num } = err {
                    if line_num == 2 {
                        crc_is_correct = false;
                    }
                }
            }
        }
        assert!(!crc_is_correct);

        Ok(())
    }

    #[test]
    fn convert() -> Result<(), Box<dyn std::error::Error>> {
        use serde::{Deserialize, Serialize};
        use crate::file_work::convert;

        #[derive(Serialize, Deserialize, Clone, Debug)]
        struct User {
            name: String,
            age: u8,
        }

        #[derive(Serialize, Deserialize, Clone, Debug)]
        struct NewUser {
            name: String,
            last_visit_date_time: Option<u64>,
        }

        let src_file = tmp_file()?;
        let mut users = crate::BTreeMap::open_or_create(&src_file, Cfg::default())?;
        users.insert(0, User { name: "Masha".to_string(), age: 23 })?;
        users.insert(3, User { name: "Sasha".to_string(), age: 58 })?;
        users.insert(5, User { name: "Pasha".to_string(), age: 33 })?;
        drop(users);

        let file_content = std::fs::read_to_string(&src_file)?;
        let expected = "ins [0,{\"name\":\"Masha\",\"age\":23}]\n\
                              ins [3,{\"name\":\"Sasha\",\"age\":58}]\n\
                              ins [5,{\"name\":\"Pasha\",\"age\":33}]\n";
        assert_eq!(file_content, expected);

        // Convert map history file for new configuration of storing with Sha256 blockchain integrity.
        let converted_file = tmp_file()?;
        let old_cfg = Cfg::default();
        let mut new_cfg = Cfg::default();
        new_cfg.integrity = Some(Integrity::Sha256Chain([0; 32]));

        convert::<i32, User, i32, User, _>(&src_file, old_cfg, &converted_file, new_cfg, |map_operation| {
            map_operation
        })?;

        let file_content = std::fs::read_to_string(&converted_file)?;
        let expected = "ins [0,{\"name\":\"Masha\",\"age\":23}] 5b2bb07c78852e70bcb0bd003bb00417ce2cf27d83c508835d0e434da1d0655f\n\
                              ins [3,{\"name\":\"Sasha\",\"age\":58}] a5acf8fc1b4a5ace0c5a2de7ea5ef4befc1feb56f1197d4726179ea31a6757a2\n\
                              ins [5,{\"name\":\"Pasha\",\"age\":33}] 857aefa8e960ba47f5b71eb95b04c1da928f053d6565731a17cb49e3c92e030c\n";
        assert_eq!(file_content, expected);

        // Convert map history file for new 'User' structure and crc32 integrity of storing.
        let mut old_cfg = Cfg::default();
        old_cfg.integrity = Some(Integrity::Sha256Chain([0; 32]));
        let mut new_cfg = Cfg::default();
        new_cfg.integrity = Some(Integrity::Crc32);

        convert::<i32, User, i32, NewUser, _>(&converted_file, old_cfg, &converted_file, new_cfg, |map_operation| {
            match map_operation {
                MapOperation::Insert(key, user) => {
                    MapOperation::Insert(key, NewUser { name: user.name, last_visit_date_time: None })
                },
                MapOperation::Remove(key) => {
                    MapOperation::Remove(key)
                },
            }
        })?;

        let file_content = std::fs::read_to_string(&converted_file)?;
        let expected = "ins [0,{\"name\":\"Masha\",\"last_visit_date_time\":null}] 2937967141\n\
                              ins [3,{\"name\":\"Sasha\",\"last_visit_date_time\":null}] 1287121668\n\
                              ins [5,{\"name\":\"Pasha\",\"last_visit_date_time\":null}] 2217782757\n";
        assert_eq!(file_content, expected);

        Ok(())
    }

    #[test]
    fn arbitrary_map() -> Result<(), Box<dyn std::error::Error>> {
        use crate::map_trait::MapTrait;
        use crate::map_with_file::MapWithFile;

        struct StupidMap<Key, Value> {
            vec: Vec<(Key, Value)>
        }

        type StupidMapWithFile<Key, Value> = MapWithFile<Key, Value, StupidMap<Key, Value>>;

        impl<Key: Ord, Value> MapTrait<Key, Value> for StupidMap<Key, Value> {
            fn get(&self, key: &Key) -> Option<&Value> {
                let res = self.vec.binary_search_by(|(k, _)| {
                    k.cmp(key)
                });

                if let Ok(index) = res {
                    self.vec.get(index).map(|(_, val)| val)
                } else {
                    None
                }
            }

            fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
                let res = self.vec.binary_search_by(|(k, _)| {
                    k.cmp(key)
                });

                if let Ok(index) = res {
                    self.vec.get_mut(index).map(|(_, val)| val)
                } else {
                    None
                }
            }

            fn insert(&mut self, key: Key, value: Value) -> Option<Value> {
                let res = self.vec.binary_search_by(|(k, _)| {
                    k.cmp(&key)
                });

                let mut value = value;

                let old_val = if let Ok(index) = res {
                    if let Some(current_val) = self.vec.get_mut(index).map(|(_, val)| val) {
                        std::mem::swap(current_val, &mut value);
                        Some(value)
                    } else {
                        unreachable!();
                    }
                } else {
                    self.vec.push((key, value));
                    self.vec.sort_by(|(key_a, _), (key_b, _)| { key_a.cmp(key_b) });
                    None
                };

                old_val
            }

            fn remove(&mut self, key: &Key) -> Option<Value> {
                let res = self.vec.binary_search_by(|(k, _)| {
                    k.cmp(key)
                });

                if let Ok(index) = res {
                    let (_, old_val) = self.vec.remove(index);
                    Some(old_val)
                } else {
                    None
                }
            }

            fn for_each(&self, mut f: impl FnMut(&Key, &Value)) {
                for (key, val) in self.vec.iter() {
                    f(key, val)
                }
            }
        }

        impl<Key: Default, Value: Default> Default for StupidMap<Key, Value> {
            fn default() -> Self {
                StupidMap { vec: Vec::new() }
            }
        }

        let file_name = tmp_file()?;

        // VecMapWithFile based on vector_map::VecMap
        let mut map = StupidMapWithFile::open_or_create(&file_name, crate::Cfg::default())?;

        map.insert(0, "Masha".to_string())?;
        map.insert(1, "Sasha".to_string())?;
        map.insert(3, "Natasha".to_string())?;
        map.remove(&1)?;

        assert_eq!(map.get(&0), Some(&"Masha".to_string()));
        assert_eq!(map.get(&1), None);
        assert_eq!(map.get(&3), Some(&"Natasha".to_string()));

        drop(map);

        let file_content = std::fs::read_to_string(&file_name)?;
        let expected = "ins [0,\"Masha\"]\n\
                              ins [1,\"Sasha\"]\n\
                              ins [3,\"Natasha\"]\n\
                              rem 1\n";
        assert_eq!(file_content, expected);

        Ok(())
    }

    #[test]
    fn before_write_and_after_read_callbacks() -> Result<(), Box<dyn std::error::Error>> {
        let src_file = tmp_file()?;
        let mut cfg = Cfg::default();
        cfg.format = Format::Text(
            Some(Box::new(|line| {
                assert_eq!(line, "ins [0,\"Masha\"]\n");
            })),
            Some(Box::new(|line| {
                assert_eq!(line, "ins [0,\"Masha\"]\n");
                Ok(())
            }))
        );

        let mut map = crate::BTreeMap::open_or_create(&src_file, cfg)?;
        map.insert(0, "Masha".to_string())?;

        Ok(())
    }

    #[test]
    fn before_write_and_after_read_callbacks2() -> Result<(), Box<dyn std::error::Error>> {
        let src_file = tmp_file()?;
        let mut cfg = Cfg::default();
        cfg.format = Format::Text(
            Some(Box::new(|line| {
                assert_eq!(line, "ins [0,\"Masha\"]\n");
                *line = line.trim_end().to_string() + " + Sasha\n";
            })),
            None,
        );

        let mut map = crate::BTreeMap::open_or_create(&src_file, cfg)?;
        map.insert(0, "Masha".to_string())?;
        drop(map);

        // reopen
        let mut cfg = Cfg::default();
        cfg.format = Format::Text(
            None,
            Some(Box::new(|line| {
                assert_eq!(line, "ins [0,\"Masha\"] + Sasha\n");
                *line = line[..line.len() - 8].to_string() + "\n";
                Ok(())
            }))
        );

        let mut map = crate::HashMap::open_or_create(&src_file, cfg)?;
        map.insert(1, "Masha".to_string())?;

        Ok(())
    }

    #[derive(Debug)]
    struct TempDirError();

    fn tmp_file() -> Result<String, TempDirError> {
        let tempdir = std::env::temp_dir()
            .to_str().ok_or(TempDirError())?
            .to_string();
        Ok(format!("{}/{}.txt", tempdir, Uuid::new_v4()))
    }

    impl std::error::Error for TempDirError {}

    impl std::fmt::Display for TempDirError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self)
        }
    }
}
