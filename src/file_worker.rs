use std::sync::mpsc::{channel, Sender};
use std::thread::{spawn, JoinHandle};
use std::fs::File;
use std::panic;
use fs2::FileExt;
use std::io::Write;

/// For write to the log file in background thread.
pub(crate) struct FileWorker {
    task_sender: Sender<FileWorkerTask>,
    join_handle: Option<JoinHandle<()>>,
}

impl FileWorker {
    /// Constructs 'FileWorker'. 'file' is opened and exclusive locked file.
    pub fn new(mut file: File, error_callback: Option<Box<dyn Fn(std::io::Error) + Send>>) -> Self {
        let (tasks_sender, task_receiver) = channel();

        let join_handle = Some(spawn(move || 'thread_loop: loop {
            match task_receiver.recv() {
                Ok(task) => {
                    match task {
                        FileWorkerTask::Write(data) => {
                            if let Err(err) = file.write_all(data.as_bytes()) {
                                call_error_callback(&error_callback, err);
                            }
                        },
                        FileWorkerTask::Stop => {
                            if let Err(err) = file.unlock() {
                                call_error_callback(&error_callback, err);
                            }
                            break 'thread_loop;
                        },
                    }
                },
                Err(err) => {
                    dbg!(err);
                }
            }
        }));

        FileWorker { task_sender: tasks_sender, join_handle }
    }

    /// Write insert operation in the file in the background thread.
    pub fn write(&self, data: String) {
        let task = FileWorkerTask::Write(data);
        if self.task_sender.send(task).is_err() {
            unreachable!()
        }
    }
}

impl Drop for FileWorker {
    fn drop(&mut self) {
        if let Err(err) = self.task_sender.send(FileWorkerTask::Stop) {
            unreachable!(err);
        }
        self.join_handle.take().map(JoinHandle::join);
    }
}

fn call_error_callback(callback: &Option<Box<dyn Fn(std::io::Error) + Send>>, err: std::io::Error) {
    match callback {
        Some(callback) => {
            if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                callback(err);
            })) {
                dbg!(format!("panic in background error hook function {:?}", &err));
            }
        }
        None => {
            dbg!(&err);
        }
    }
}

/// Task for send to worker thread.
enum FileWorkerTask {
    /// Write operation to the file in the background thread.
    Write(String),
    /// Stop worker
    Stop,
}
