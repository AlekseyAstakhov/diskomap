/// Config of file based map.
pub struct Cfg {
    /// Format of stored data, binary or text.
    pub format: Format,
    /// Method of controlling the integrity of stored data in a history file.
    pub integrity: Option<Integrity>,
    /// Callback for receive a file write error.
    /// If the callback from the callback is None, then errors are ignored..
    pub write_error_callback: Option<Box<dyn FnMut(std::io::Error) + Send>>,
}

/// Format of stored data, binary or text.
pub enum Format {
    /// Text format.
    /// Each changing map operation is recorded as one line ending with '\n'.
    /// The line starts with operation name as "ins " or "remove ",
    /// followed by data serialized with serde::json, for "ins " key and value serialized as
    /// tuple (key, value).
    /// Next, optionally can be data integrity, after ' '.
    ///
    /// For example:
    /// ins [3,{"name":"Сake","age":31}]
    /// rem 3
    ///
    /// Or with checksum example:
    /// ins [8,"a"] 2212816791
    /// rem 8 3024193484
    Text(Option<BeforeWriteTxtCallback>, Option<AfterReadTxtCallback>),

    /// Binary format.
    /// Each changing map operation is recorded as data block beginning with
    /// byte where first 2 bits is number of subsequent bytes specifying the block length,
    /// 00b0 - 1 byte, 01b0 - 2 bytes, 10b0 - 4 bytes, 11b0 - 8 bytes.
    /// the next 6 bits are reserved for flags.
    /// After the first byte, bytes of the block length follow in little endian.
    /// After block data where first byte of block data is
    /// code of operation as 'insert' or 'remove'. After operation code followed
    /// code arguments of operation such as key value serialized with bincode2
    /// and after, optionally can be data integrity.
    Bin(Option<BeforeWriteBinCallback>, Option<AfterReadBinCallback>),
}

/// Called when data of insert or remove prepared for writing to the file.
/// This may be needed for data transformation before write to the file
/// or for sending data to a third-party storage.
/// Source string ends with '\n' and transformed string need so ends with '\n'
/// and no contains other '\n' because reading from file will line by line.
pub type BeforeWriteTxtCallback = Box<dyn FnMut(&mut String)>;

/// Called when data of insert or remove read from file.
/// This may be needed for the necessary transformation of data written to a file
/// or for sending data to a third-party storage.
pub type AfterReadTxtCallback = Box<dyn FnMut(&mut String) -> Result<(), Box<dyn std::error::Error>>>;

/// Called when data of insert or remove prepared for writing to the file.
/// This may be needed for data transformation before write to the file
/// or for sending data to a third-party storage.
pub type BeforeWriteBinCallback = Box<dyn FnMut(&mut Vec<u8>)>;

/// Called when data of insert or remove read from file.
/// This may be needed for the necessary transformation of data written to a file
/// or for sending data to a third-party storage.
pub type AfterReadBinCallback = Box<dyn FnMut(&mut Vec<u8>) -> Result<(), Box<dyn std::error::Error>>>;


/// Method of controlling the integrity of stored data in a history file.
#[derive(Clone)]
pub enum Integrity {
    /// crc32 (ieee) checksum of operation and data for each line in the operations history file.
    Crc32,
    /// For Sha1 blockchain. Each line in the history file will contain
    /// the sum of the hash of the previous line with the operation + data hash of the current line.
    Sha1Chain([u8; 20]),
    /// For Sha256 blockchain. Each line in the history file will contain
    /// the sum of the hash of the previous line with the operation + data hash of the current line.
    Sha256Chain([u8; 32]),
}

impl Default for Cfg {
    /// Default config of file based map.
    fn default() -> Self {
        Cfg {
            integrity: None,
            write_error_callback: None,
            format: Format::Text(None, None),
        }
    }
}