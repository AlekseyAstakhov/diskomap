use crate::Integrity;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::BTreeMap;
use crc::crc32;
use crate::btree::{blockchain_sha256, BTreeError};
use std::sync::RwLock;
use serde::de::DeserializeOwned;
use std::io::Write;

// Load from file and process all operations and make actual map.
pub fn load_from_file<Key, Value>(file: &mut File, integrity: &mut Option<Integrity>)
    -> Result<BTreeMap<Key, RwLock<Value>>, BTreeError>
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
            return Err(BTreeError::FileLineLengthLessThenMinimum { line_num });
        }

        let line_data = if let Some(integrity) = integrity {
            let data_index = line.rfind(' ').ok_or(BTreeError::NoExpectedHash { line_num })?;
            let line_data = &line[..data_index];
            let hash_data = line[data_index + 1..line.len()].trim_end();

            match integrity {
                Integrity::Sha256Chain(hash_of_prev) => {
                    let sum = blockchain_sha256(&hash_of_prev, line_data.as_bytes());
                    if sum != hash_data {
                        return Err(BTreeError::WrongSha256Blockchain { line_num });
                    }
                    *hash_of_prev = sum;
                },
                Integrity::Crc32 => {
                    let crc = crc32::checksum_ieee(line_data.as_bytes());
                    if crc.to_string() != hash_data {
                        return Err(BTreeError::WrongCrc32 { line_num });
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
                    map.insert(key, RwLock::new(val));
                }
                Err(err) => {
                    return Err(BTreeError::DeserializeJsonError { err, line_num });
                }
            },
            "rem" => match serde_json::from_str(&line_data[4..]) {
                Ok(key) => {
                    map.remove(&key);
                }
                Err(err) => {
                    return Err(BTreeError::DeserializeJsonError { err, line_num });
                }
            },
            _ => {
                return Err(BTreeError::NoLineDefinition { line_num });
            }
        }

        line_num += 1;
        line.clear();
    }

    Ok(map)
}

pub fn write_insert_to_log_file(key_val_json: &str, file: &mut File, integrity: &mut Option<Integrity>)
                                       -> Result<(), std::io::Error> {

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

    file.write_all(line.as_bytes())
}

pub fn write_remove_to_log_file(key_json: &str, file: &mut File, integrity: &mut Option<Integrity>)
                            -> Result<(), std::io::Error> {

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

    file.write_all(line.as_bytes())
}