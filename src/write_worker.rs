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

/// For write to the log file in background thread.
pub(crate) struct WriteWorker {
    tasks_sender: Sender<WriteWorkerTask>,
    join_handle: Option<JoinHandle<()>>,
}

enum WriteWorkerTask {
    WriteInsert {
        key_val_json: String,
        file: Arc<Mutex<File>>,
        error_callback: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>,
        integrity: Arc<Mutex<Option<Integrity>>>,
    },
    WriteRemove {
        key_json: String,
        file: Arc<Mutex<File>>,
        error_callback: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>,
        integrity: Arc<Mutex<Option<Integrity>>>,
    },
    Stop,
}

impl WriteWorker {
    pub fn new() -> Self {
        let (tasks_sender, task_receiver) = channel();

        let join_handle = Some(spawn(move || 'thread_loop: loop {
            match task_receiver.recv() {
                Ok(task) => {
                    match task {
                        WriteWorkerTask::WriteInsert { key_val_json, file, error_callback, integrity } => {
                            write_insert_to_log_file(&key_val_json, &file, &error_callback, &integrity);
                        },
                        WriteWorkerTask::WriteRemove { key_json, file, error_callback, integrity  } => {
                            write_remove_to_log_file(&key_json, &file, &error_callback, &integrity);
                        },
                        WriteWorkerTask::Stop => {
                            break 'thread_loop;
                        },
                    }
                },
                Err(err) => {
                    dbg!(err);
                }
            }
        }));

        WriteWorker { tasks_sender, join_handle }
    }

    pub fn write_insert(&self, key_val_json: String, file: Arc<Mutex<File>>, error_callback: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>, integrity: Arc<Mutex<Option<Integrity>>>) -> Result<(), ()> {
        let task = WriteWorkerTask::WriteInsert { key_val_json, file, error_callback, integrity };
        self.tasks_sender.send(task).map_err(|_|())
    }

    pub fn write_remove(&self, key_json: String, file: Arc<Mutex<File>>, error_callback: Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>, integrity: Arc<Mutex<Option<Integrity>>>) -> Result<(), ()> {
        let task = WriteWorkerTask::WriteRemove { key_json, file, error_callback, integrity };
        self.tasks_sender.send(task).map_err(|_|())
    }
}

impl Drop for WriteWorker {
    fn drop(&mut self) {
        if let Err(err) = self.tasks_sender.send(WriteWorkerTask::Stop) {
            unreachable!(err);
        }
        self.join_handle.take().map(JoinHandle::join);
    }
}

fn write_insert_to_log_file(key_val_json: &str, file: &Arc<Mutex<File>>, error_callback: &Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>, integrity: &Arc<Mutex<Option<Integrity>>>) {
    let mut line = "ins ".to_string() + &key_val_json;

    if let Ok(mut integrity) = integrity.lock() {
        if let Some(integrity) = integrity.deref_mut() {
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
    } else {
        unreachable!();
    }

    line.push('\n');

    let res = match file.lock() {
        Ok(mut file) => file.write_all(line.as_bytes()),
        Err(err) => {
            dbg!(err);
            unreachable!();
        }
    };

    if let Err(err) = res {
        call_background_error_callback_or_dbg(&error_callback, err);
    }
}

fn write_remove_to_log_file(key_json: &str, file: &Arc<Mutex<File>>, error_callback: &Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>, integrity: &Arc<Mutex<Option<Integrity>>>) {
    let mut line = "rem ".to_string() + key_json;

    if let Ok(mut integrity) = integrity.lock() {
        if let Some(integrity) = integrity.deref_mut() {
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
    } else {
        unreachable!();
    }

    line.push('\n');

    let res = match file.lock() {
        Ok(mut file) => file.write_all(line.as_bytes()),
        Err(err) => { dbg!(err); unreachable!(); }
    };

    if let Err(err) = res {
        call_background_error_callback_or_dbg(&error_callback, err);
    }
}

fn call_background_error_callback_or_dbg(hook: &Arc<Mutex<Option<Box<dyn Fn(std::io::Error) + Send>>>>, err: std::io::Error) {
    match hook.lock() {
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
