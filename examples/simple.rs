fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_name = "db/simple_db.txt";

    let mut map = diskomap::BTreeMap::open_or_create(file_name, diskomap::Cfg::default())?;
    // Using with this map is not much different from using a std BTreeMap or HashMap.
    // This map does not provide a mutable iterator and some mutable functions.
    //
    // When calling of insert or remove functions,
    // data will be written to the td::collection::BTreeMap immediately,
    // and to disk later in a background thread.
    map.insert(0, "Masha".to_string())?;
    map.insert(1, "Sasha".to_string())?;
    map.insert(0, "Natasha".to_string())?;

    // When calling of get function, data will be retrieved from std::collection::BTreeMap
    // immediately, without overhead or disk operations.
    let name = map.get(&0).unwrap();
    println!("name {}", name);

    // All non-mutable functions of the original map are also available.
    let count = map.map().len();
    println!("count {}", count);

    // The file will be closed here.
    drop(map);

    // Open in next time.
    let mut map = diskomap::BTreeMap::open_or_create(file_name, diskomap::Cfg::default())?;
    map.remove(&0)?;
    map.remove(&1)?;
    map.insert(0, "Abc".to_string())?;
    drop(map);

    println!("File content:");
    print!("{}", std::fs::read_to_string(file_name)?);

    Ok(())
}
