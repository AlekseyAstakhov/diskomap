use crate::file_work::{MapOperation, blockchain_sha1, blockchain_sha256, IntegrityError};
use crate::map_trait::MapTrait;
use serde::de::DeserializeOwned;
use crate::{LoadFileError, Integrity};
use std::io::{BufReader, Read};
use serde::Serialize;
use crc::crc32;

/// 1 byte for block length if right 2 bits of first byte of block is 0b00.
const U8_LEN: u8 = 0;
/// 2 bytes for block length if right 2 bits of first byte of block is 0b01.
const U16_LEN: u8 = 1;
/// 4 bytes for block length if right 2 bits of first byte of block is 0b10.
const U32_LEN: u8 = 2;
/// 8 bytes for block length if right 2 bits of first byte of block is 0b11.
const U64_LEN: u8 = 3;

/// Code of insert to map operation.
const INSERT: u8 = 0;
/// Code of remove from map operation.
const REMOVE: u8 = 1;

/// Make data block with insert operation for write to file.
pub fn bin_file_block_of_insert<Key, Value>(key: &Key, value: Value, integrity: &mut Option<Integrity>)
    -> Result<Vec<u8>, bincode2::Error>
where
    Key: Serialize,
    Value: Serialize
{
    let key_val_bin_data = bincode2::serialize(&(&key, &value))?;
    let mut data = vec![INSERT];
    data.extend_from_slice(&key_val_bin_data);
    post_process_file_bin_block(&mut data, integrity);
    let mut res = bin_block_len(data.len());
    res.extend_from_slice(&data);
    Ok(res)
}

/// Make data block with remove operation for write to file.
pub fn bin_file_block_of_remove<Key>(key: &Key, integrity: &mut Option<Integrity>)
    -> Result<Vec<u8>, bincode2::Error>
where
    Key: Serialize
{
    let key_bin_data = bincode2::serialize(&key)?;
    let mut data = vec![REMOVE];
    data.extend_from_slice(&key_bin_data);
    post_process_file_bin_block(&mut data, integrity);
    let mut res = bin_block_len(data.len());
    res.extend_from_slice(&data);
    Ok(res)
}

/// Load from binary format file all operations and make actual map.
pub fn map_from_bin_file<Map, Key, Value, ReadCallback, Reader>(
    file: &mut Reader,
    integrity: &mut Option<Integrity>,
    read_callback: Option<ReadCallback>,
) -> Result<Map, LoadFileError>
    where
        Key: std::cmp::Ord + DeserializeOwned,
        Value: DeserializeOwned,
        Map: MapTrait<Key, Value> + Default,
        ReadCallback: FnMut(&mut Vec<u8>) -> Result<(), Box<dyn std::error::Error>>,
        Reader: std::io::Read,
{
    let mut map = Map::default();
    load_from_bin_file(file, integrity, read_callback, |map_operation| {
        match map_operation {
            MapOperation::Insert(key, value) => map.insert(key, value),
            MapOperation::Remove(key) => map.remove(&key),
        };

        Ok(())
    })?;

    Ok(map)
}

/// Load from binary format file all map history records and call 'ProcessedCallback' callback for each.
pub fn load_from_bin_file<Key, Value, ReadCallback, ProcessedCallback, Reader>(
    file: &mut Reader,
    integrity: &mut Option<Integrity>,
    mut after_read_callback: Option<ReadCallback>,
    mut processed_callback: ProcessedCallback
    ) -> Result<(), LoadFileError>
where
    Key: DeserializeOwned,
    Value: DeserializeOwned,
    ProcessedCallback: FnMut(MapOperation<Key, Value>) -> Result<(), ()>,
    ReadCallback: FnMut(&mut Vec<u8>) -> Result<(), Box<dyn std::error::Error>>,
    Reader: std::io::Read,
{
    let mut reader = BufReader::new(file);
    let mut block_num = 1;
    loop {
        let block_len = read_bin_block_len(&mut reader)?;
        if block_len == 0 {
            return Ok(())
        }

        let mut data_block = vec![0; block_len];
        reader.read_exact(&mut data_block[..])?;

        if let Some(callback) = &mut after_read_callback {
            callback(&mut data_block)
               .map_err(|err| LoadFileError::InterruptedWithBeforeReadCallback(err))?;
        }

        let data_block = if let Some(integrity) = integrity {
            process_block_integrity(&mut data_block, integrity, block_num)?
        } else {
            &data_block[..]
        };

        match data_block[0] {
            INSERT => {
                let (key, val) = bincode2::deserialize(&data_block[1..]).map_err(|err| LoadFileError::DeserializeBincodeError { err, block_num })?;
                processed_callback(MapOperation::Insert(key, val)).map_err(|()| LoadFileError::Interrupted)?;
            }
            REMOVE => {
                let key = bincode2::deserialize(&data_block[1..]).map_err(|err| LoadFileError::DeserializeBincodeError { err, block_num })?;
                processed_callback(MapOperation::Remove(key)).map_err(|()| LoadFileError::Interrupted)?;
            }
            _ => {
            }
        }

        block_num += 1;
    }
}

/// Check data integrity after read from file.
pub fn process_block_integrity<'a>(data_block: &'a mut [u8], integrity: &mut Integrity, block_num: usize) -> Result<&'a [u8], IntegrityError> {
    match integrity {
        Integrity::Crc32 => {
            if data_block.len() < 6 {
                return Err(IntegrityError::Crc32Error { line_num: block_num });
            }
            let crc = crc32::checksum_ieee(&data_block[..data_block.len() - 4]);
            let mut crc_in_file = [0u8; 4];
            crc_in_file.clone_from_slice(&data_block[data_block.len() - 4..]);
            if crc != u32::from_le_bytes(crc_in_file) {
                return Err(IntegrityError::Crc32Error { line_num: block_num });
            }
            Ok(&data_block[..data_block.len() - 4])
        },
        Integrity::Sha1Chain(hash_of_prev) => {
            const HASH_LEN: usize = 20;
            if data_block.len() < HASH_LEN + 1 {
                return Err(IntegrityError::Sha1ChainError { line_num: block_num });
            }
            let data = &data_block[..data_block.len() - HASH_LEN];
            let mut current_hash: [u8; HASH_LEN] = [0; HASH_LEN];
            blockchain_sha1(&hash_of_prev[..], data, &mut current_hash);
            let hash_in_file = &data_block[data_block.len() - HASH_LEN..];
            if current_hash != hash_in_file {
                return Err(IntegrityError::Sha1ChainError { line_num: block_num });
            }
            *hash_of_prev = current_hash;
            Ok(data)
        },
        Integrity::Sha256Chain(hash_of_prev) => {
            const HASH_LEN: usize = 32;
            if data_block.len() < HASH_LEN + 1 {
                return Err(IntegrityError::Sha1ChainError { line_num: block_num });
            }
            let data = &data_block[..data_block.len() - HASH_LEN];
            let mut current_hash: [u8; HASH_LEN] = [0; HASH_LEN];
            blockchain_sha256(&hash_of_prev[..], data, &mut current_hash);
            let hash_in_file = &data_block[data_block.len() - HASH_LEN..];
            if current_hash != hash_in_file {
                return Err(IntegrityError::Sha1ChainError { line_num: block_num });
            }
            *hash_of_prev = current_hash;
            Ok(data)
        },
    }
}

/// Returns the number of bytes in the binary block.
pub fn bin_block_len(len: usize) -> Vec<u8> {
    let mut res = vec![];

    if len <= u8::MAX as usize {
        res.push(U8_LEN);
        res.push(len as u8);
    } else if len <= u16::MAX as usize {
        res.push(U16_LEN);
        res.extend_from_slice(&len.to_le_bytes())
    } else if len <= u32::MAX as usize {
        res.push(U32_LEN);
        res.extend_from_slice(&len.to_le_bytes())
    } else {
        res.push(U64_LEN);
        res.extend_from_slice(&len.to_le_bytes())
    }

    res
}

/// Returns len of binary data block. Returns 0 if end of file.
/// Errors if file read error or unexpected file termination.
pub fn read_bin_block_len<Reader>(reader: &mut Reader) -> Result<usize, LoadFileError>
    where
        Reader: std::io::Read
{
    let mut first_byte_buf = [0];
    if reader.read(&mut first_byte_buf)? < 1 {
        return Ok(0);
    }

    let len_of_len = first_byte_buf[0];

    let len =  if len_of_len == U8_LEN {
        let mut len_buf = [0; 1];
        reader.read_exact(&mut len_buf)?;
        u8::from_le_bytes(len_buf) as usize
    } else if len_of_len == U16_LEN {
        let mut len_buf = [0; 2];
        reader.read_exact(&mut len_buf)?;
        u16::from_le_bytes(len_buf) as usize
    } else if len_of_len == U32_LEN {          // if len in 4 bytes
        let mut len_buf = [0; 4];
        reader.read_exact(&mut len_buf)?;
        u32::from_le_bytes(len_buf) as usize
    } else {                             // if len in 8 bytes
        let mut len_buf = [0; 8];
        reader.read_exact(&mut len_buf)?;
        u64::from_le_bytes(len_buf) as usize
    };

    if len == 0 {
        return Err(LoadFileError::WrongMinBinBlockLen);
    }

    Ok(len)
}

/// Depending on the settings in 'cfg', it adds a checksum, calculates the blockchain, compresses, encrypts, etc.
pub fn post_process_file_bin_block(bin_block: &mut Vec<u8>, integrity: &mut Option<Integrity>) {
    if let Some(integrity) = integrity {
        match integrity {
            Integrity::Crc32 => {
                let crc = crc32::checksum_ieee(bin_block);
                bin_block.extend_from_slice(&crc.to_le_bytes());
            },
            Integrity::Sha1Chain(prev_hash) => {
                let mut hash: [u8; 20] = [0; 20];
                blockchain_sha1(&prev_hash[..], bin_block, &mut hash);
                bin_block.extend_from_slice(&hash);
                *prev_hash = hash;
            },
            Integrity::Sha256Chain(prev_hash) => {
                let mut hash: [u8; 32] = [0; 32];
                blockchain_sha256(prev_hash, bin_block, &mut hash);
                bin_block.extend_from_slice(&hash);
                *prev_hash = hash;
            },
        }
    }
}
