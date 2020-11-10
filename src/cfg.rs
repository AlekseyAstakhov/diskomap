/// Config of map with storing history to the file.
#[derive(Clone)]
pub struct Cfg {
    /// Mechanism of controlling the integrity of stored data in a history file.
    pub integrity: Option<Integrity>
}

/// Mechanism of controlling the integrity of stored data in a history file.
#[derive(Clone)]
pub enum Integrity {
    /// For Sha256 blockchain. Each line in the history file will contain
    /// the sum of the hash of the previous line with the operation + data hash of the current line.
    Sha256Chain(String),
    /// crc32 (ieee) checksum of operation and data for each line in the operations history file.
    Crc32,
}

impl Default for Cfg {
    /// Default config of map with storing history to the file.
    fn default() -> Self {
        Cfg {
            integrity: None,
        }
    }
}