#[cfg(test)]
mod tests {
    #[test]
    fn btree() -> Result<(), Box<dyn std::error::Error>> {
        use std::ops::Bound::{Excluded, Included};
        use tempfile::tempdir;
        use crate::BTree;

        // new file
        let file = tempdir()?.path().join("test.txt").to_str().unwrap().to_string();
        {
            let map = BTree::open_or_create(&file, None)?;
            map.insert((), ())?;
        }
        // after restart
        {
            let map = BTree::open_or_create(&file, None)?;
            assert_eq!(Some(()), map.get(&())?);
            map.insert((), ())?;
            assert_eq!(1, map.len()?);
            map.insert((), ())?;
            assert_eq!(1, map.len()?);
            map.insert((), ())?;
            assert_eq!(1, map.len()?);
            assert_eq!(Some(()), map.get(&())?);
            map.remove(&())?;
            assert_eq!(0, map.len()?);
        }

        // new log file
        let file = tempdir()?.path().join("test2.txt").to_str().unwrap().to_string();
        {
            let map = BTree::open_or_create(&file, None)?;
            map.insert("key 1".to_string(), 1)?;
            map.insert("key 2".to_string(), 2)?;
            map.insert("key 3".to_string(), 3)?;
            map.insert("key 4".to_string(), 4)?;
            map.insert("key 5".to_string(), 5)?;
            assert_eq!(5, map.len()?);
            assert_eq!(Some(3), map.get(&"key 3".to_string())?);
            map.remove(&"key 1".to_string())?;
            map.remove(&"key 4".to_string())?;
            map.insert("key 6".to_string(), 6)?;
            map.insert("key 1".to_string(), 100)?;
            map.remove(&"key 2".to_string())?;
            map.insert("key 7".to_string(), 7)?;
            assert_eq!(map.keys()?, vec!["key 1".to_string(), "key 3".to_string(), "key 5".to_string(), "key 6".to_string(), "key 7".to_string()]);
            assert_eq!(map.values()?, vec![100, 3, 5, 6, 7]);
            assert_eq!(map.range_values((Included(&"key 3".to_string()), Included(&"key 6".to_string())))?, vec![3, 5, 6]);
            assert_eq!(map.range_keys((Included(&"key 3".to_string()), Included(&"key 5".to_string())))?, vec!["key 3".to_string(), "key 5".to_string()]);
        }
        // after restart
        {
            let map = BTree::open_or_create(&file, None)?;
            assert_eq!(5, map.len()?);
            assert_eq!(Some(100), map.get(&"key 1".to_string())?);
            assert_eq!(None, map.get(&"key 4".to_string())?);
            assert_eq!(None, map.get(&"key 2".to_string())?);
            map.insert("key 3".to_string(), 33)?;
            assert_eq!(Some(33), map.get(&"key 3".to_string())?);
            map.remove(&"key 1".to_string())?;
        }
        // after restart
        {
            let map = BTree::open_or_create(&file, None)?;
            assert_eq!(4, map.len()?);
            assert_eq!(Some(33), map.get(&"key 3".to_string())?);
            assert_eq!(None, map.get(&"key 1".to_string())?);
            assert_eq!(map.keys()?, vec!["key 3".to_string(), "key 5".to_string(), "key 6".to_string(), "key 7".to_string()]);
            assert_eq!(map.values()?, vec![33, 5, 6, 7]);
            assert_eq!(map.range((Excluded(&"key 3".to_string()), Excluded(&"key 6".to_string())))?, vec![("key 5".to_string(), 5)]);
        }

        Ok(())
    }

    #[test]
    fn index() -> Result<(), Box<dyn std::error::Error>> {
        use serde::{Deserialize, Serialize};
        use tempfile::tempdir;

        #[derive(Clone, Serialize, Deserialize)]
        struct User {
            name: String,
            age: u8,
        }

        // new log file
        let file = tempdir()?.path().join("test.txt").to_str().unwrap().to_string();
        let map = crate::BTree::open_or_create(&file, None)?;
        let user_name_index = map.create_btree_index(|value: &User| value.name.clone())?;

        map.insert(0, User { name: "Mary".to_string(), age: 21 })?;
        map.insert(1, User { name: "John".to_string(), age: 37 })?;

        assert_eq!(user_name_index.get(&"John".to_string())?, vec![1]);
        assert!(user_name_index.get(&"Masha".to_string())?.is_empty());

        map.insert(3, User { name: "Masha".to_string(), age: 27 })?;
        map.insert(0, User { name: "Natasha".to_string(), age: 23 })?;

        assert_eq!(user_name_index.get(&"Natasha".to_string())?, vec![0]);
        assert!(user_name_index.get(&"Mary".to_string())?.is_empty());
        assert_eq!(user_name_index.get(&"Masha".to_string())?, vec![3]);
        map.insert(5, User { name: "Natasha".to_string(), age: 25 })?;

        map.insert(1, User { name: "Bob".to_string(), age: 27 })?;
        assert!(user_name_index.get(&"John".to_string())?.is_empty());

        map.remove(&1)?;
        assert!(user_name_index.get(&"Bob".to_string())?.is_empty());

        map.insert(8, User { name: "Masha".to_string(), age: 23 })?;
        map.insert(12, User { name: "Masha".to_string(), age: 24 })?;
        assert_eq!(user_name_index.get(&"Masha".to_string())?, vec![3, 8, 12]);

        map.insert(8, User { name: "Natasha".to_string(), age: 35 })?;
        assert_eq!(user_name_index.get(&"Masha".to_string())?, vec![3, 12]);
        assert_eq!(user_name_index.get(&"Natasha".to_string())?, vec![0, 5, 8]);

        Ok(())
    }
}
