use crate::integrity::{Integrity, blockchain_sha256};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::BTreeMap;
use crc::crc32;
use serde::de::DeserializeOwned;

/// Load from file all operations and make actual map.
pub fn load_from_file<Key, Value>(file: &mut File, integrity: &mut Option<Integrity>)
    -> Result<BTreeMap<Key, Value>, LoadFileError>
    where
        Key: std::cmp::Ord + DeserializeOwned,
        Value: DeserializeOwned {

    let mut map = BTreeMap::new();
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
                Integrity::Sha256Chain(hash_of_prev) => {
                    let sum = blockchain_sha256(&hash_of_prev, line_data.as_bytes());
                    if sum != hash_data {
                        return Err(LoadFileError::WrongSha256Chain { line_num });
                    }
                    *hash_of_prev = sum;
                },
                Integrity::Crc32 => {
                    let crc = crc32::checksum_ieee(line_data.as_bytes());
                    if crc.to_string() != hash_data {
                        return Err(LoadFileError::WrongCrc32 { line_num });
                    }
                },
            }
            line_data
        } else {
            &line[..]
        };

        match &line_data[..3] {
            "ins" => match serde_json::from_str(&line_data[4..]) {
                Ok((key, val)) => {
                    map.insert(key, val);
                }
                Err(err) => {
                    return Err(LoadFileError::DeserializeJsonError { err, line_num });
                }
            },
            "rem" => match serde_json::from_str(&line_data[4..]) {
                Ok(key) => {
                    map.remove(&key);
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

    Ok(map)
}

/// Possible errors of 'load_from_file'.
#[derive(Debug)]
pub enum LoadFileError {
    /// When line length in operations log file less then need.
    FileLineLengthLessThenMinimum { line_num: usize, },
    /// Read file error.
    ReadFileError(std::io::Error),
    /// There is no expected checksum or hash in the log file line when integrity used.
    NoExpectedHash { line_num: usize },
    /// Wrong Sha256 of log file line data when Sha256 blockchain integrity used.
    WrongSha256Chain { line_num: usize, },
    /// Wrong crc32 of log file line data when crc32 integrity used.
    WrongCrc32 { line_num: usize, },
    /// Json error with line number in operations log file.
    DeserializeJsonError { err: serde_json::Error, line_num: usize, },
    /// Line in operations log file no contains operation name as "ins" or "rem".
    NoLineDefinition { line_num: usize, },
}

/// Make line with insert operation for write to file.
pub fn file_line_of_insert(key_val_json: &str, integrity: &mut Option<Integrity>) -> String {
    let mut line = "ins ".to_string() + &key_val_json;

    if let Some(integrity) = integrity {
        match integrity {
            Integrity::Sha256Chain(prev_hash) => {
                let sum = blockchain_sha256(&prev_hash, line.as_bytes());
                line = format!("{} {}", line, sum);
                *prev_hash = sum;
            },
            Integrity::Crc32 => {
                let crc = crc32::checksum_ieee(line.as_bytes());
                line = format!("{} {}", line, crc);
            },
        }
    }

    line.push('\n');
    line
}

/// Make line with remove operation for write to file.
pub fn file_line_of_remove(key_json: &str, integrity: &mut Option<Integrity>) -> String {

    let mut line = "rem ".to_string() + key_json;

    if let Some(integrity) = integrity {
        match integrity {
            Integrity::Sha256Chain(prev_hash) => {
                let sum = blockchain_sha256(&prev_hash, line.as_bytes());
                line = format!("{} {}", line, sum);
                *prev_hash = sum;
            },
            Integrity::Crc32 => {
                let crc = crc32::checksum_ieee(line.as_bytes());
                line = format!("{} {}", line, crc);
            },
        }
    }

    line.push('\n');
    line
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

impl From<std::io::Error> for LoadFileError {
    fn from(err: std::io::Error) -> Self {
        LoadFileError::ReadFileError(err)
    }
}
