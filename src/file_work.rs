use crate::cfg::Integrity;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use crc::crc32;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::map_trait::MapTrait;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use crypto::sha1::Sha1;
use crate::Cfg;
use fs2::FileExt;
use std::fs;
use uuid::Uuid;

/// Record about operation on map in history file.
pub enum MapOperation<Key, Value> {
    /// Insert operation.
    Insert(Key, Value),
    /// Remove operation.
    Remove(Key),
}

/// Load from file all records and call callback for each.
pub fn load_file<Key, Value, F>(file: &mut File, integrity: &mut Option<Integrity>, mut f: F)
    -> Result<(), LoadFileError>
where
    Key: DeserializeOwned,
    Value: DeserializeOwned,
    F: FnMut(MapOperation<Key, Value>) -> Result<(), ()>,
{
    let mut reader = BufReader::new(file);
    let mut line = String::with_capacity(150);
    let mut line_num = 1;
    while reader.read_line(&mut line)? > 0 {
        const MIN_LINE_LEN: usize = 4;
        if line.len() < MIN_LINE_LEN {
            return Err(LoadFileError::FileLineLengthLessThenMinimum { line_num });
        }

        let line_data = if let Some(integrity) = integrity {
            let data_index = line.rfind(' ').ok_or(LoadFileError::NoExpectedHash { line_num })?;
            let line_data = &line[..data_index];
            let hash_data = line[data_index + 1..line.len()].trim_end();

            match integrity {
                Integrity::Crc32 => {
                    let crc = crc32::checksum_ieee(line_data.as_bytes());
                    if crc.to_string() != hash_data {
                        return Err(LoadFileError::WrongCrc32 { line_num });
                    }
                },
                Integrity::Sha1Chain(hash_of_prev) => {
                    let sum = blockchain_sha1(&hash_of_prev, line_data.as_bytes());
                    if sum != hash_data {
                        return Err(LoadFileError::WrongSha1Chain { line_num });
                    }
                    *hash_of_prev = sum;
                },
                Integrity::Sha256Chain(hash_of_prev) => {
                    let sum = blockchain_sha256(&hash_of_prev, line_data.as_bytes());
                    if sum != hash_data {
                        return Err(LoadFileError::WrongSha256Chain { line_num });
                    }
                    *hash_of_prev = sum;
                },
            }

            line_data
        } else {
            &line[..]
        };

        match &line_data[..3] {
            "ins" => match serde_json::from_str(&line_data[4..]) {
                Ok((key, val)) => {
                    if let Err(()) = f(MapOperation::Insert(key, val)) {
                        return Err(LoadFileError::Interrupted);
                    }
                }
                Err(err) => {
                    return Err(LoadFileError::DeserializeJsonError { err, line_num });
                }
            },
            "rem" => match serde_json::from_str(&line_data[4..]) {
                Ok(key) => {
                    if let Err(()) = f(MapOperation::Remove(key)) {
                        return Err(LoadFileError::Interrupted);
                    }
                }
                Err(err) => {
                    return Err(LoadFileError::DeserializeJsonError { err, line_num });
                }
            },
            _ => {
                return Err(LoadFileError::NoLineDefinition { line_num });
            }
        }

        line_num += 1;
        line.clear();
    }

    Ok(())
}

/// Load from file all operations and make actual map.
pub fn map_from_file<Map, Key, Value>(file: &mut File, integrity: &mut Option<Integrity>)
    -> Result<Map, LoadFileError>
where
    Key: std::cmp::Ord + DeserializeOwned,
    Value: DeserializeOwned,
    Map: MapTrait<Key, Value> + Default,
{
    let mut map = Map::default();
    load_file(file, integrity, |map_operation| {
        match map_operation {
            MapOperation::Insert(key, value) => map.insert(key, value),
            MapOperation::Remove(key) => map.remove(&key),
        };

        Ok(())
    })?;

    Ok(map)
}

/// Convert history file for other config or key-values types.
// If 'src_file_path' and 'dst_file_path' is equal, then file will rewritten via tmp file.
pub fn convert<SrcKey, SrcValue, DstKey, DstValue, F>
(src_file_path: &str, mut src_cfg: Cfg, dst_file_path: &str, mut dst_cfg: Cfg, f: F)
 -> Result<(), ConvertError>
    where
        SrcKey: DeserializeOwned,
        SrcValue: DeserializeOwned,
        DstKey: Serialize,
        DstValue: Serialize,
        F: Fn(MapOperation<SrcKey, SrcValue>) -> MapOperation<DstKey, DstValue>
{
    let mut src_file = OpenOptions::new().read(true).open(src_file_path)
        .map_err(|err| ConvertError::OpenSrcFileError(err))?;
    src_file.lock_exclusive()
        .map_err(|_| ConvertError::LockSrcFileError)?;

    let file_is_same = src_file_path == dst_file_path;

    let dst_file_path = if file_is_same {
        let tempdir = std::env::temp_dir()
            .to_str().ok_or(ConvertError::TmpFileError)?
            .to_string();
        format!("{}/{}.txt", tempdir, Uuid::new_v4())
    } else {
        dst_file_path.to_string()
    };

    let mut dst_file = if file_is_same {
        OpenOptions::new().write(true).create(true).open(&dst_file_path)
            .map_err(|err| ConvertError::OpenDstFileError(err))?
    } else {
        OpenOptions::new().write(true).create(true).open(&dst_file_path)
            .map_err(|err| ConvertError::OpenDstFileError(err))?
    };

    dst_file.set_len(0).map_err(|_| ConvertError::ClearDstFileError)?;

    dst_file.lock_exclusive()
        .map_err(|_| ConvertError::LockDstFileError)?;

    let mut write_err: Option<ConvertError> = None;

    load_file::<SrcKey, SrcValue, _>(&mut src_file, &mut src_cfg.integrity, |map_operation| {
        match f(map_operation) {
            MapOperation::Insert(key, value) => {
                match file_line_of_insert(&key, &value, &mut dst_cfg.integrity) {
                    Ok(line) => {
                        if let Err(err) = dst_file.write_all(line.as_bytes()) {
                            write_err = Some(ConvertError::WriteToFileError(err));
                            return Err(())
                        }
                    },
                    Err(err) => {
                        write_err = Some(ConvertError::SerializeError(err));
                        return Err(())
                    },
                }
            },
            MapOperation::Remove(key) => {
                match file_line_of_remove(&key, &mut dst_cfg.integrity) {
                    Ok(line) => {
                        if let Err(err) = dst_file.write_all(line.as_bytes()) {
                            write_err = Some(ConvertError::WriteToFileError(err));
                            return Err(())
                        }
                    },
                    Err(err) => {
                        write_err = Some(ConvertError::SerializeError(err));
                        return Err(())
                    },
                }
            },
        }

        Ok(())
    }).map_err(|err| ConvertError::LoadFileError(err))?;

    if file_is_same {
        drop(src_file);
        drop(dst_file);
        fs::rename(&dst_file_path, &src_file_path)
            .map_err(|_| ConvertError::TmpFileError)?;
    }

    Ok(())
}

/// Possible errors of 'load_from_file'.
#[derive(Debug)]
pub enum LoadFileError {
    /// When line length in operations log file less then need.
    FileLineLengthLessThenMinimum { line_num: usize, },
    /// Open, create or read file error.
    FileError(std::io::Error),
    /// There is no expected checksum or hash in the log file line when integrity used.
    NoExpectedHash { line_num: usize },
    /// Wrong Sha1 of log file line data when Sha256 blockchain integrity used.
    WrongSha1Chain { line_num: usize, },
    /// Wrong Sha256 of log file line data when Sha256 blockchain integrity used.
    WrongSha256Chain { line_num: usize, },
    /// Wrong crc32 of log file line data when crc32 integrity used.
    WrongCrc32 { line_num: usize, },
    /// Json error with line number in operations log file.
    DeserializeJsonError { err: serde_json::Error, line_num: usize, },
    /// Line in operations log file no contains operation name as "ins" or "rem".
    NoLineDefinition { line_num: usize, },
    /// Load file function is manually interrupted.
    Interrupted,
}

/// Make line with insert operation for write to file.
pub fn file_line_of_insert<Key, Value>(key: &Key, value: Value, integrity: &mut Option<Integrity>)
    -> Result<String, serde_json::Error>
where
    Key: Serialize,
    Value: Serialize
{
    let key_val_json = serde_json::to_string(&(&key, &value))?;
    let mut line = "ins ".to_string() + &key_val_json;
    post_process_file_line(&mut line, integrity);
    Ok(line)
}

/// Make line with remove operation for write to file.
pub fn file_line_of_remove<Key>(key: &Key, integrity: &mut Option<Integrity>)
    -> Result<String, serde_json::Error>
where
    Key: Serialize
{
    let key_json = serde_json::to_string(key)?;
    let mut line = "rem ".to_string() + &key_json;
    post_process_file_line(&mut line, integrity);
    Ok(line)
}

/// Depending on the settings in 'cfg', it adds a checksum, calculates the blockchain, compresses, encrypts, etc.
pub fn post_process_file_line(line: &mut String, integrity: &mut Option<Integrity>) {
    if let Some(integrity) = integrity {
        match integrity {
            Integrity::Crc32 => {
                let crc = crc32::checksum_ieee(line.as_bytes());
                *line += &format!(" {}", crc);
            },
            Integrity::Sha1Chain(prev_hash) => {
                let sum = blockchain_sha1(&prev_hash, line.as_bytes());
                *line += &format!(" {}", sum);
                *prev_hash = sum;
            },
            Integrity::Sha256Chain(prev_hash) => {
                let sum = blockchain_sha256(&prev_hash, line.as_bytes());
                *line += &format!(" {}", sum);
                *prev_hash = sum;
            },
        }
    }

    line.push('\n');
}

/// Create dirs to path if not exist.
pub(crate) fn create_dirs_to_path_if_not_exist(path_to_file: &str) -> Result<(), std::io::Error> {
    if let Some(index) = path_to_file.rfind('/') {
        let dir_path = &path_to_file[..index];
        if !std::path::Path::new(dir_path).exists() {
            std::fs::create_dir_all(&path_to_file[..index])?;
        }
    }

    Ok(())
}

/// Returns hash of significant data of current line of file (hash of sum of prev hash and hash of current line data).
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

/// Returns hash of significant data of current line of file (hash of sum of prev hash and hash of current line data).
pub fn blockchain_sha1(prev_hash: &str, line_data: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.input(line_data);
    let current_data_hash = hasher.result_str();
    let mut buf = Vec::new(); // need optimize to [u8; 512]
    buf.extend_from_slice(prev_hash.as_bytes());
    buf.extend_from_slice(&current_data_hash.as_bytes());
    let mut hasher = Sha1::new();
    hasher.input(&buf);
    hasher.result_str()
}

impl From<std::io::Error> for LoadFileError {
    fn from(err: std::io::Error) -> Self {
        LoadFileError::FileError(err)
    }
}

impl std::fmt::Display for LoadFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for LoadFileError {}

/// Error convertation of operations history file.
#[derive(Debug)]
pub enum ConvertError {
    /// When can't open file that need convert.
    OpenSrcFileError(std::io::Error),
    /// When can't open target file where to save.
    OpenDstFileError(std::io::Error),
    /// When can't clear target file before conversion.
    ClearDstFileError,
    /// When can't exclusive lock opened source file.
    LockSrcFileError,
    /// When can't exclusive lock opened target file.
    LockDstFileError,
    /// Json error when serialize key or value.
    SerializeError(serde_json::Error),
    /// Error of reading source file.
    LoadFileError(LoadFileError),
    /// When write error to the target file.
    WriteToFileError(std::io::Error),
    /// Error of creating tmp file when source and target file has same path.
    TmpFileError,
}

impl std::error::Error for ConvertError {}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
