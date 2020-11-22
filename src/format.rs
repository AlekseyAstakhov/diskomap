use crate::cfg::Format;
use crate::Cfg;
use std::io::Write;
use serde::de::DeserializeOwned;
use serde::Serialize;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use crypto::sha1::Sha1;
use std::fs;
use fs2::FileExt;
use uuid::Uuid;
use crate::text_format::{text_file_line_of_insert, file_line_of_remove, load_from_text_file};
use crate::bin_format::load_from_bin_file;

/// Record about operation on map in history file.
pub enum MapOperation<Key, Value> {
    /// Insert operation.
    Insert(Key, Value),
    /// Remove operation.
    Remove(Key),
}

/// Convert history file for other config or key-values types.
// If 'src_file_path' and 'dst_file_path' is equal, then file will rewritten via tmp file.
pub fn convert<SrcKey, SrcValue, DstKey, DstValue, F>(
    src_file_path: &str,
    mut src_cfg: Cfg,
    dst_file_path: &str,
    mut dst_cfg: Cfg, f: F
) -> Result<(), ConvertError>
where
    SrcKey: DeserializeOwned,
    SrcValue: DeserializeOwned,
    DstKey: Serialize,
    DstValue: Serialize,
    F: Fn(MapOperation<SrcKey, SrcValue>) -> MapOperation<DstKey, DstValue>
{
    let mut src_file = fs::OpenOptions::new().read(true).open(src_file_path)
        .map_err(ConvertError::OpenSrcFileError)?;
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

    let mut dst_file = fs::OpenOptions::new().write(true).create(true).open(&dst_file_path)
        .map_err(ConvertError::OpenDstFileError)?;

    dst_file.set_len(0).map_err(ConvertError::ClearDstFileError)?;

    dst_file.lock_exclusive()
        .map_err(|_| ConvertError::LockDstFileError)?;

    let mut write_err: Option<ConvertError> = None;

    let process_map_operation = |map_operation| {
        match f(map_operation) {
            MapOperation::Insert(key, value) => {
                match text_file_line_of_insert(&key, &value, &mut dst_cfg.integrity) {
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
    };

    match src_cfg.format {
        Format::Text(_, after_read_callback) => {
            load_from_text_file::<SrcKey, SrcValue, _, _, _>(&mut src_file, &mut src_cfg.integrity, after_read_callback, process_map_operation)
                .map_err(ConvertError::LoadFileError)?;
        },
        Format::Bin(_, after_read_callback) => {
            load_from_bin_file::<SrcKey, SrcValue, _, _, _>(&mut src_file, &mut src_cfg.integrity, after_read_callback, process_map_operation)
                .map_err(ConvertError::LoadFileError)?;
        },
    };

    if file_is_same {
        drop(src_file);
        drop(dst_file);
        fs::rename(&dst_file_path, &src_file_path)
            .map_err(|_| ConvertError::TmpFileError)?;
    }

    Ok(())
}

/// Create dirs to path if not exist.
pub(crate) fn create_dirs_to_path_if_not_exist(path_to_file: &str) -> Result<(), std::io::Error> {
    if let Some(index) = path_to_file.rfind('/') {
        let dir_path = &path_to_file[..index];
        if !std::path::Path::new(dir_path).exists() {
            fs::create_dir_all(&path_to_file[..index])?;
        }
    }

    Ok(())
}

/// Returns hash of significant data of current record of file (hash of sum of prev hash and hash of current line data).
pub fn blockchain_sha1(prev_hash: &[u8], data: &[u8], out: &mut [u8]) {
    let mut hasher = Sha1::new();
    hasher.input(data);
    let mut current_hash = [0; 20];
    hasher.result(&mut current_hash);
    let mut buf = Vec::with_capacity(prev_hash.len() + current_hash.len());
    buf.extend_from_slice(prev_hash);
    buf.extend_from_slice(&current_hash);
    let mut hasher = Sha1::new();
    hasher.input(&buf);
    hasher.result(out);
}

/// Returns hash of significant data of current record of file (hash of sum of prev hash and hash of current line data).
pub fn blockchain_sha256(prev_hash: &[u8], data: &[u8], out: &mut [u8]) {
    let mut hasher = Sha256::new();
    hasher.input(data);
    let mut current_hash = [0; 32];
    hasher.result(&mut current_hash);
    let mut buf = Vec::with_capacity(prev_hash.len() + current_hash.len());
    buf.extend_from_slice(prev_hash);
    buf.extend_from_slice(&current_hash);
    let mut hasher = Sha256::new();
    hasher.input(&buf);
    hasher.result(out);
}

/// Possible errors of 'load_from_file'.
#[derive(Debug)]
pub enum LoadFileError {
    /// When line length in file less then needed.
    LastLineWithoutEndLine { line_num: usize, },
    /// When line length in operations log file less then needed.
    FileLineLengthLessThenMinimum { line_num: usize, },
    /// Block len of file must be more then 1.
    WrongMinBinBlockLen,
    /// In current implementation first byte contains bytes count of block len in first 2 bits and need other bits set 0.
    WrongFirstByte,
    /// Open, create or read file error.
    FileError(std::io::Error),
    /// Error of integrity.
    IntegrityError(IntegrityError),
    /// Json error with line number in operations log file.
    DeserializeJsonError { err: serde_json::Error, line_num: usize },
    /// Json error with line number in operations log file.
    DeserializeBincodeError { err: bincode2::Error, block_num: usize },
    /// Line in operations log file no contains operation name as "ins" or "rem".
    NoLineDefinition { line_num: usize, },
    /// Load file function is manually interrupted.
    Interrupted,
    /// Load file function is manually interrupted with 'after_read_callback'.
    InterruptedWithBeforeReadCallback(Box<dyn std::error::Error>),
}

/// Errors of integrity.
#[derive(Debug)]
pub enum IntegrityError {
    /// There is no expected checksum or hash in the log file line when integrity used.
    NoExpectedHash { line_num: usize },
    /// Wrong crc32 of log file line data when crc32 integrity used.
    Crc32Error { line_num: usize, },
    /// Wrong Sha1 of log file line data when Sha256 blockchain integrity used.
    Sha1ChainError { line_num: usize, },
    /// Wrong Sha256 of log file line data when Sha256 blockchain integrity used.
    Sha256ChainError { line_num: usize, },
}

impl From<IntegrityError> for LoadFileError {
    fn from(err: IntegrityError) -> Self {
        LoadFileError::IntegrityError(err)
    }
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
    ClearDstFileError(std::io::Error),
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
