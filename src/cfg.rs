/// Config of map with storing history to the file.
pub struct Cfg {
    /// Method of controlling the integrity of stored data in a history file.
    pub integrity: Option<Integrity>,
    /// Callback for receive a file write error.
    /// If the callback from the callback is None, then errors are ignored..
    pub write_error_callback: Option<Box<dyn FnMut(std::io::Error) + Send>>,

    /// Called when data of one operation prepared for write to the file.
    /// This may be needed for the necessary transformation of data written to a file
    /// or for sending data to a third-party storage.
    /// Return None from callback if no need change data.
    /// For data transformation use with 'Self.on_data_read' callback
    /// Source string ends with '\n'. Transformed string need so ends with '\n'
    /// and no contains other '\n'.
    pub before_write_callback: Option<Box<dyn FnMut(&str) -> Option<String>>>,
    /// Called when data of one operation read from file.
    /// This may be needed for the necessary transformation of data written to a file
    /// or for sending data to a third-party storage.
    /// Return None from callback if no need change data.
    /// For data transformation use with 'Self.on_data_prepared' callback
    pub after_read_callback: Option<Box<dyn FnMut(&str) -> Option<String>>>,
}

/// Method of controlling the integrity of stored data in a history file.
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
            write_error_callback: None,
            before_write_callback: None,
            after_read_callback: None,
        }
    }
}