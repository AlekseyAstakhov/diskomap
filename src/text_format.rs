use crate::file_work::{MapOperation, blockchain_sha1, blockchain_sha256, IntegrityError};
use crate::map_trait::MapTrait;
use serde::de::DeserializeOwned;
use crate::{LoadFileError, Integrity};
use serde::Serialize;
use std::io::{BufReader, BufRead};
use crc::crc32;

/// Make line with insert operation for write to file.
pub fn text_file_line_of_insert<Key, Value>(key: &Key, value: Value, integrity: &mut Option<Integrity>)
    -> Result<String, serde_json::Error>
where
    Key: Serialize,
    Value: Serialize
{
    let key_val_json = serde_json::to_string(&(&key, &value))?;
    let mut line = "ins ".to_string() + &key_val_json;
    post_process_text_file_line(&mut line, integrity);
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
    post_process_text_file_line(&mut line, integrity);
    Ok(line)
}

/// Load from text format file all operations and make actual map.
pub fn map_from_text_file<Map, Key, Value, ReadCallback, Reader>(
    file: &mut Reader,
    integrity: &mut Option<Integrity>,
    read_callback: Option<ReadCallback>,
) -> Result<Map, LoadFileError>
    where
        Key: std::cmp::Ord + DeserializeOwned,
        Value: DeserializeOwned,
        Map: MapTrait<Key, Value> + Default,
        ReadCallback: FnMut(&mut String) -> Result<(), Box<dyn std::error::Error>>,
        Reader: std::io::Read,
{
    let mut map = Map::default();
    load_from_text_file(file, integrity, read_callback, |map_operation| {
        match map_operation {
            MapOperation::Insert(key, value) => map.insert(key, value),
            MapOperation::Remove(key) => map.remove(&key),
        };

        Ok(())
    })?;

    Ok(map)
}

/// Load from text format file all map history records and call 'ProcessedCallback' callback for each.
pub fn load_from_text_file<Key, Value, ReadCallback, ProcessedCallback, Reader>(
    file: &mut Reader,
    integrity: &mut Option<Integrity>,
    mut after_read_callback: Option<ReadCallback>,
    mut processed_callback: ProcessedCallback
) -> Result<(), LoadFileError>
    where
        Key: DeserializeOwned,
        Value: DeserializeOwned,
        ProcessedCallback: FnMut(MapOperation<Key, Value>) -> Result<(), ()>,
        ReadCallback: FnMut(&mut String) -> Result<(), Box<dyn std::error::Error>>,
        Reader: std::io::Read,
{
    let mut reader = BufReader::new(file);
    let mut line = String::with_capacity(150);
    let mut line_num = 1;
    while reader.read_line(&mut line)? > 0 {
        if let Some(callback) = &mut after_read_callback {
            callback(&mut line)
                .map_err(|err| LoadFileError::InterruptedWithBeforeReadCallback(err))?;
        }

        if !line.ends_with('\n') {
            return Err(LoadFileError::LastLineWithoutEndLine { line_num });
        }

        const MIN_LINE_LEN: usize = 4;
        if line.len() < MIN_LINE_LEN {
            return Err(LoadFileError::FileLineLengthLessThenMinimum { line_num });
        }

        let line_data = if let Some(integrity) = integrity {
            process_line_integrity(&line, integrity, line_num)?
        } else {
            &line[..]
        };

        match &line_data[..4] {
            "ins " => {
                let (key, val) = serde_json::from_str(&line_data[4..]).map_err(|err| LoadFileError::DeserializeJsonError { err, line_num })?;
                processed_callback(MapOperation::Insert(key, val)).map_err(|()| LoadFileError::Interrupted)?;
            },
            "rem " => {
                let key = serde_json::from_str(&line_data[4..]).map_err(|err| LoadFileError::DeserializeJsonError { err, line_num })?;
                processed_callback(MapOperation::Remove(key)).map_err(|()| LoadFileError::Interrupted)?;
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

/// Check data integrity after read from file.
pub fn process_line_integrity<'a>(line: &'a str, integrity: &mut Integrity, line_num: usize) -> Result<&'a str, IntegrityError> {
    let data_index = line.rfind(' ').ok_or(IntegrityError::NoExpectedHash { line_num })?;
    let line_data = &line[..data_index];
    let hash_in_file = line[data_index + 1..].trim_end();

    match integrity {
        Integrity::Crc32 => {
            let crc = crc32::checksum_ieee(line_data.as_bytes());
            if crc.to_string() != hash_in_file {
                return Err(IntegrityError::Crc32Error { line_num });
            }
        },
        Integrity::Sha1Chain(hash_of_prev) => {
            let mut current_hash: [u8; 20]  = [0; 20];
            blockchain_sha1(&hash_of_prev[..], line_data.as_bytes(), &mut current_hash);
            if hex::encode(current_hash) != hash_in_file {
                return Err(IntegrityError::Sha1ChainError { line_num });
            }
            *hash_of_prev = current_hash;
        },
        Integrity::Sha256Chain(hash_of_prev) => {
            let mut current_hash: [u8; 32]  = [0; 32];
            blockchain_sha256(&hash_of_prev[..], line_data.as_bytes(), &mut current_hash);
            if hex::encode(current_hash) != hash_in_file {
                return Err(IntegrityError::Sha256ChainError { line_num });
            }
            *hash_of_prev = current_hash;
        },
    }

    Ok(line_data)
}

/// Depending on the settings in 'cfg', it adds a checksum, calculates the blockchain, compresses, encrypts, etc.
pub fn post_process_text_file_line(line: &mut String, integrity: &mut Option<Integrity>) {
    if let Some(integrity) = integrity {
        match integrity {
            Integrity::Crc32 => {
                let crc = crc32::checksum_ieee(line.as_bytes());
                *line += &format!(" {}", crc);
            },
            Integrity::Sha1Chain(prev_hash) => {
                let mut hash: [u8; 20] = [0; 20];
                blockchain_sha1(&prev_hash[..], line.as_bytes(), &mut hash);
                *line += &format!(" {}", hex::encode(&hash));
                *prev_hash = hash;
            },
            Integrity::Sha256Chain(prev_hash) => {
                let mut hash: [u8; 32] = [0; 32];
                blockchain_sha256(&prev_hash[..], line.as_bytes(), &mut hash);
                *line += &format!(" {}", hex::encode(&hash[..]));
                *prev_hash = hash;
            },
        }
    }

    line.push('\n');
}
