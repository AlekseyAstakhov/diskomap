use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // For simple data integrity in the file, you can use a crc32 checksum of operation + data
    // for each line of the file.
    let file_with_crc32 = "db/integrity_crc32.txt";
    let mut cfg = diskomap::Cfg::default();
    cfg.integrity = Some(diskomap::Integrity::Crc32);
    let mut map = diskomap::BTreeMap::open_or_create(file_with_crc32, cfg)?;
    map.insert(0, "a".to_string())?;
    map.insert(3, "b".to_string())?;
    map.insert(5, "c".to_string())?;
    drop(map);
    println!("File content with crc32:");
    print!("{}", fs::read_to_string(file_with_crc32)?);

    // For unchanged integrity, you can use the sha256 chain.
    // Each line in the operations log file will contain the sum of the hash of the previous line
    // with the data hash of the current line.

    // The initial hash to be used as the previous line hash for generate hash of first line.
    let initial_hash = [0; 32];
    let blockchain_file = "db/blockchain.txt";
    let mut cfg = diskomap::Cfg::default();
    cfg.integrity = Some(diskomap::Integrity::Sha256Chain(initial_hash));
    let mut map = diskomap::BTreeMap::open_or_create(blockchain_file, cfg)?;
    map.insert(0, "a".to_string())?;
    map.insert(3, "b".to_string())?;
    map.insert(5, "c".to_string())?;
    drop(map);
    println!("Blockchain file content:");
    print!("{}", fs::read_to_string(blockchain_file)?);

    Ok(())
}
