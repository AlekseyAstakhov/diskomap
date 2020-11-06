fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_name = "db/simple_db.txt";

    let mut map = diskomap::BTree::open_or_create(file_name, None)?;
    map.insert(0, "Masha".to_string())?;
    map.insert(1, "Sasha".to_string())?;
    map.insert(0, "Natasha".to_string())?;
    drop(map);

    // open in next time
    let mut map = diskomap::BTree::open_or_create(file_name, None)?;
    map.remove(&0)?;
    map.remove(&1)?;
    map.insert(0, "Abc".to_string())?;
    drop(map);

    println!("File content:");
    print!("{}", std::fs::read_to_string(file_name)?);


    Ok(())
}
