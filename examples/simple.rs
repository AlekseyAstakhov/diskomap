fn main() -> Result<(), Box<dyn std::error::Error>> {
    {
        let map = diskomap::BTree::open_or_create("db/simple_db.txt", None)?;
        map.insert(0, "Masha".to_string())?;
        map.insert(1, "Sasha".to_string())?;
        map.insert(0, "Natasha".to_string())?;
    }

    // reload
    {
        let map = diskomap::BTree::open_or_create("db/simple_db.txt", None)?;
        assert_eq!(map.get(&0)?, Some("Natasha".to_string()));
        assert_eq!(map.get(&1)?, Some("Sasha".to_string()));
        map.remove(&0)?;
        map.remove(&1)?;
    }

    Ok(())
}
