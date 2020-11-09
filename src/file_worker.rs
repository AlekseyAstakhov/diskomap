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
                                if let Some(callback) = &error_callback { callback(err); }
                            }
                        },
                        FileWorkerTask::Stop => {
                            if let Err(err) = file.unlock() {
                                if let Some(callback) = &error_callback { callback(err); }
                            }
                            break 'thread_loop;
                        },
                    }
                },
                Err(err) => {
                    unreachable!(err);
                }
            }
        }));

        FileWorker { task_sender: tasks_sender, join_handle }
    }

    /// Write insert operation in the file in the background thread.
    pub fn write(&self, data: String) {
        let task = FileWorkerTask::Write(data);
        if let Err(err) = self.task_sender.send(task) {
            unreachable!(err);
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

/// Task for send to worker thread.
enum FileWorkerTask {
    /// Write operation to the file in the background thread.
    Write(String),
    /// Stop worker
    Stop,
}
