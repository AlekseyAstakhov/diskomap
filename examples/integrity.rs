fn main() -> Result<(), Box<dyn std::error::Error>> {
    // For simple data integrity on disk, you can use crc32 for each line in log file.
    let users = diskomap::BTree::open_or_create("db/integrity_crc32.txt", Some(diskomap::Integrity::Crc32))?;

    users.insert(0, "a".to_string())?;
    users.insert(3, "b".to_string())?;
    users.insert(5, "c".to_string())?;

    // For for irreplaceable integrity, you can use sha256 chain, where each line
    // will contain the hash of the continuation of the hash of the previous line
    // with the hash of the data of the current line.

    // initial_hash will use as previous hash of first block
    let initial_hash = String::new();
    let users = diskomap::BTree::open_or_create("db/blockchain.txt", Some(diskomap::Integrity::Sha256Blockchain(initial_hash)))?;

    users.insert(0, "a".to_string())?;
    users.insert(3, "b".to_string())?;
    users.insert(5, "c".to_string())?;

    Ok(())
}
