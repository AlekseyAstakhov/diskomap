#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use crate::BTree;
    use crate::cfg::Cfg;
    use std::io::Write;
    use crate::file_work::LoadFileError;

    #[test]
    fn common() -> Result<(), Box<dyn std::error::Error>> {
        // new file
        let file = tempdir()?.path().join("btree_test.txt").to_str().unwrap().to_string();
        {
            let mut map = BTree::open_or_create(&file, Cfg::default())?;
            map.insert((), ())?;
        }
        // after restart
        {
            let mut map = BTree::open_or_create(&file, Cfg::default())?;
            assert_eq!(Some(&()), map.get(&()));
            map.insert((), ())?;
            assert_eq!(1, map.len());
            map.insert((), ())?;
            assert_eq!(1, map.len());
            map.insert((), ())?;
            assert_eq!(1, map.len());
            assert_eq!(Some(&()), map.get(&()));
            map.remove(&())?;
            assert_eq!(0, map.len());
        }

        // new log file
        let file = tempdir()?.path().join("btree_test2.txt").to_str().unwrap().to_string();
        {
            let mut map = BTree::open_or_create(&file, Cfg::default())?;
            map.insert("key 1".to_string(), 1)?;
            map.insert("key 2".to_string(), 2)?;
            map.insert("key 3".to_string(), 3)?;
            map.insert("key 4".to_string(), 4)?;
            map.insert("key 5".to_string(), 5)?;
            assert_eq!(5, map.len());
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
        }
        // after restart
        {
            let mut map = BTree::open_or_create(&file, Cfg::default())?;
            assert_eq!(5, map.len());
            assert_eq!(Some(&100), map.get(&"key 1".to_string()));
            assert_eq!(None, map.get(&"key 4".to_string()));
            assert_eq!(None, map.get(&"key 2".to_string()));
            map.insert("key 3".to_string(), 33)?;
            assert_eq!(Some(&33), map.get(&"key 3".to_string()));
            map.remove(&"key 1".to_string())?;
        }
        // after restart
        {
            let map = BTree::open_or_create(&file, Cfg::default())?;
            assert_eq!(4, map.len());
            assert_eq!(Some(&33), map.get(&"key 3".to_string()));
            assert_eq!(None, map.get(&"key 1".to_string()));
            let keys = map.map().keys().cloned().collect::<Vec<String>>();
            assert_eq!(keys, vec!["key 3".to_string(), "key 5".to_string(), "key 6".to_string(), "key 7".to_string()]);
            let values = map.map().values().cloned().collect::<Vec<i32>>();
            assert_eq!(values, vec![33, 5, 6, 7]);
        }

        Ok(())
    }

    #[test]
    fn btree_index() -> Result<(), Box<dyn std::error::Error>> {
        use serde::{Deserialize, Serialize};
        use tempfile::tempdir;
        use crate::cfg::Cfg;

        #[derive(Clone, Serialize, Deserialize)]
        struct User {
            name: String,
            age: u8,
        }

        // new log file
        let file = tempdir()?.path().join("index_test.txt").to_str().unwrap().to_string();
        let mut map = crate::BTree::open_or_create(&file, Cfg::default())?;
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
        use tempfile::tempdir;

        #[derive(Clone, Serialize, Deserialize)]
        struct User {
            name: String,
            age: u8,
        }

        // new log file
        let file = tempdir()?.path().join("index_test.txt").to_str().unwrap().to_string();
        let mut map = crate::BTree::open_or_create(&file, Cfg::default())?;
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
        use crate::BTree;
        use std::fs::OpenOptions;
        use crate::cfg::*;

        let cfg = Cfg {integrity: Some(Integrity::Crc32)};

        let file = tempdir()?.path().join("integrity_test.txt").to_str().unwrap().to_string();
        let mut map = BTree::open_or_create(&file, cfg.clone())?;
        map.insert(0, "a".to_string())?;
        map.insert(3, "b".to_string())?;
        map.insert(5, "c".to_string())?;
        drop(map);
        let file_content = std::fs::read_to_string(&file)?;
        let expected_content = "ins [0,\"a\"] 1874290170\nins [3,\"b\"] 3949308173\nins [5,\"c\"] 1023287335\n";
        assert_eq!(file_content, expected_content);

        let mut map: BTree<i32, String> = BTree::open_or_create(&file, cfg.clone())?;
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
        let res: Result<BTree<i32, String>, LoadFileError> = BTree::open_or_create(&file, cfg);
        let mut crc_is_correct = true;
        if let Err(res) = res {
            if let LoadFileError::WrongCrc32 { line_num } = res {
                    if line_num == 2 {
                    crc_is_correct = false;
                }
            }
        }
        assert!(!crc_is_correct);

        Ok(())
    }

    #[test]
    fn sha256_chain_integrity() -> Result<(), Box<dyn std::error::Error>> {
        use crate::Integrity;
        use crate::BTree;
        use std::fs::OpenOptions;

        let inital_hash = "7a2131d1a326940d3a04d4ee70e7ba4992b0b826ce5c3521b67edcac9ae6041e";

        let cfg = Cfg { integrity: Some(Integrity::Sha256Chain(inital_hash.to_string())) };

        let file = tempdir()?.path().join("integrity_test.txt").to_str().unwrap().to_string();
        let mut map = BTree::open_or_create(&file, cfg.clone())?;
        map.insert(0, "a".to_string())?;
        map.insert(3, "b".to_string())?;
        map.insert(5, "c".to_string())?;
        drop(map);

        let file_content = std::fs::read_to_string(&file)?;
        let expected = "ins [0,\"a\"] fcfd9332281f041699b205ac1dfd27adecee7f3861d8893215dc93dda3b8803c\n\
                              ins [3,\"b\"] d7d09f5c06dea915b6a6f26a5f8414e19c02251887be41c5006c1334f6307f49\n\
                              ins [5,\"c\"] a78a60587a54d0580f6d4df05ccbefb89931b6a1f018315d3d9b9747686a9d56\n";

        assert_eq!(file_content, expected);

        let mut map: BTree<i32, String> = BTree::open_or_create(&file, cfg.clone())?;
        map.remove(&3)?;
        drop(map);
        let file_content = std::fs::read_to_string(&file)?;
        let expected = "ins [0,\"a\"] fcfd9332281f041699b205ac1dfd27adecee7f3861d8893215dc93dda3b8803c\n\
                              ins [3,\"b\"] d7d09f5c06dea915b6a6f26a5f8414e19c02251887be41c5006c1334f6307f49\n\
                              ins [5,\"c\"] a78a60587a54d0580f6d4df05ccbefb89931b6a1f018315d3d9b9747686a9d56\n\
                              rem 3 c686f4889cd4b10acfea6d23d16079bb76cadba1840543a1100a60c20360c4b2\n";

        assert_eq!(file_content, expected);

        let mut f = OpenOptions::new().read(true).write(true).create(true).open(&file)?;
        // wrong d7e09f5c06dea915b6a6f26a5f8414e19c02251887be41c5006c1334f6307f49
        let bad_content = "ins [0,\"a\"] fcfd9332281f041699b205ac1dfd27adecee7f3861d8893215dc93dda3b8803c\n\
                              ins [3,\"b\"] d7e09f5c06dea915b6a6f26a5f8414e19c02251887be41c5006c1334f6307f49\n\
                              ins [5,\"c\"] a78a60587a54d0580f6d4df05ccbefb89931b6a1f018315d3d9b9747686a9d56\n";

        f.write_all(bad_content.as_bytes())?;
        drop(f);
        let res: Result<BTree<i32, String>, LoadFileError> = BTree::open_or_create(&file, cfg);
        let mut crc_is_correct = true;
        if let Err(res) = res {
            if let LoadFileError::WrongSha256Chain { line_num } = res {
                if line_num == 2 {
                    crc_is_correct = false;
                }
            }
        }
        assert!(!crc_is_correct);

        Ok(())
    }
}
