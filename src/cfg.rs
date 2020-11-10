/// Config of map with storing history to the file.
pub struct Cfg {
    /// Mechanism of controlling the integrity of stored data in a history file.
    pub integrity: Option<Integrity>,
    /// Callback for receive a file write error.
    /// If the callback from the callback is None, then errors are ignored..
    pub on_write_error: Option<Box<dyn Fn(std::io::Error) + Send>>,
}

/// Mechanism of controlling the integrity of stored data in a history file.
#[derive(Clone)]
pub enum Integrity {
    /// crc32 (ieee) checksum of operation and data for each line in the operations history file.
    Crc32,
    /// For Sha1 blockchain. Each line in the history file will contain
    /// the sum of the hash of the previous line with the operation + data hash of the current line.
    Sha1Chain(String),
    /// For Sha256 blockchain. Each line in the history file will contain
    /// the sum of the hash of the previous line with the operation + data hash of the current line.
    Sha256Chain(String),
}

impl Default for Cfg {
    /// Default config of map with storing history to the file.
    fn default() -> Self {
        Cfg {
            integrity: None,
            on_write_error: None,
        }
    }
}