use crypto::digest::Digest;
use crypto::sha2::Sha256;

/// Mechanism of controlling the integrity of stored data in a log file.
pub enum Integrity {
    // For Sha256 blockchain. Each line in the operations log file will contain
    // the sum of the hash of the previous line with the operation + data hash of the current line.
    Sha256Chain(String),
    // crc32 checksum of operation and data for each line in the operations log file.
    Crc32,
}

/// Returns hash of current log line (hash of sum of prev hash and hash of current line data).
pub fn blockchain_sha256(prev_hash: &str, line_data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.input(line_data);
    let current_data_hash = hasher.result_str();
    let mut buf = Vec::new(); // need optimize to [u8; 512]
    buf.extend_from_slice(prev_hash.as_bytes());
    buf.extend_from_slice(&current_data_hash.as_bytes());
    let mut hasher = Sha256::new();
    hasher.input(&buf);
    hasher.result_str()
}
