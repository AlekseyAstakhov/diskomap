use std::sync::mpsc::{channel, Sender};
use std::thread::{spawn, JoinHandle};
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::Write;
use crc::crc32;
use std::panic;
use std::ops::{Deref, DerefMut};
use crate::Integrity;
use crate::btree::blockchain_sha256;
use fs2::FileExt;

/// For write to the log file in background thread.
pub(crate) struct FileWorker {
    tasks_sender: Sender<FileWorkerTask>,
    join_handle: Option<JoinHandle<()>>,
}

impl FileWorker {
    /// Constructs 'FileWorker'. 'file' is opened and exclusive locked file.
    pub fn new(mut file: File) -> Self {
        let (tasks_sender, task_receiver) = channel();

        let join_handle = Some(spawn(move || 'thread_loop: loop {
            match task_receiver.recv() {
                Ok(task) => {
                    match task {
                        FileWorkerTask::WriteInsert { key_val_json, error_callback, integrity } => {
                            match write_insert_to_log_file(&key_val_json, &mut file, &mut integrity.lock().unwrap().deref_mut()) {
                                Ok(()) => {},
                                Err(err) => call_error_callback(&error_callback, err),
                            }
                        },
                        FileWorkerTask::WriteRemove { key_json, error_callback, integrity  } => {
                            match write_remove_to_log_file(&key_json, &mut file, &mut integrity.lock().unwrap().deref_mut()) {
                                Ok(()) => {},
                                Err(err) => call_error_callback(&error_callback, err),
                            }
                        },
                        FileWorkerTask::Stop => {
                            file.unlock();
                            break 'thread_loop;
                        },
                    }
                },
                Err(err) => {
                    dbg!(err);
                }
            }
        }));

        FileWorker { tasks_sender, join_handle }
    }

    /// Write insert operation in the file in the background thread.
    pub fn write_insert(&self, key_val_json: String, error_callback: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>, integrity: Arc<Mutex<Option<Integrity>>>) -> Result<(), ()> {
        let task = FileWorkerTask::WriteInsert { key_val_json, error_callback, integrity };
        self.tasks_sender.send(task).map_err(|_|())
    }

    /// Write remove operation in the file in the background thread.
    pub fn write_remove(&self, key_json: String, error_callback: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>, integrity: Arc<Mutex<Option<Integrity>>>) -> Result<(), ()> {
        let task = FileWorkerTask::WriteRemove { key_json, error_callback, integrity };
        self.tasks_sender.send(task).map_err(|_|())
    }
}

impl Drop for FileWorker {
    fn drop(&mut self) {
        if let Err(err) = self.tasks_sender.send(FileWorkerTask::Stop) {
            unreachable!(err);
        }
        self.join_handle.take().map(JoinHandle::join);
    }
}

pub(crate) fn write_insert_to_log_file(key_val_json: &str, file: &mut File, integrity: &mut Option<Integrity>)
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

fn write_remove_to_log_file(key_json: &str, file: &mut File, integrity: &mut Option<Integrity>)
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

fn call_error_callback(callback: &Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>, err: std::io::Error) {
    match callback.lock() {
        Ok(hook) => match hook.deref() {
            Some(hook) => {
                if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    hook(err);
                })) {
                    dbg!(format!("panic in background error hook function {:?}", &err));
                }
            }
            None => {
                dbg!(&err);
            }
        },
        Err(err) => {
            dbg!(err);
            unreachable!();
        }
    }
}

/// Task for send to worker thread.
enum FileWorkerTask {
    /// Write insert operation in the file in the background thread.
    WriteInsert {
        key_val_json: String,
        error_callback: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>,
        integrity: Arc<Mutex<Option<Integrity>>>,
    },
    /// Write remove operation in the file in the background thread.
    WriteRemove {
        key_json: String,
        error_callback: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>,
        integrity: Arc<Mutex<Option<Integrity>>>,
    },
    /// Stop worker
    Stop,
}
