/// Mechanism of controlling the integrity of stored data in a log file.
pub enum Integrity {
    // For Sha256 blockchain. Each line in the operations log file will contain
    // the sum of the hash of the previous line with the operation + data hash of the current line.
    Sha256Chain(String),
    // crc32 (ieee) checksum of operation and data for each line in the operations log file.
    Crc32,
}
