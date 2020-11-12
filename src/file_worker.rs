use std::sync::mpsc::{channel, Sender};
use std::thread::{spawn, JoinHandle};
use std::fs::File;
use std::panic;
use std::io::Write;

/// For write to the log file in background thread.
pub(crate) struct FileWorker {
    task_sender: Sender<FileWorkerTask>,
    join_handle: Option<JoinHandle<()>>,
}

impl FileWorker {
    /// Constructs 'FileWorker'. 'file' is opened and exclusive locked file.
    pub fn new(mut file: File, mut error_callback: Option<Box<dyn FnMut(std::io::Error) + Send>>) -> Self {
        let (tasks_sender, task_receiver) = channel();

        let join_handle = Some(spawn(move || 'thread_loop: loop {
            let task = task_receiver.recv()
                .unwrap_or_else(|err| unreachable!(err)); // unreachable because owner thread will join this thread handle after send FileWorkerTask::Stop and only after will disconnect channel

            match task {
                FileWorkerTask::Write(data) => {
                    if let Err(err) = file.write_all(data.as_bytes()) {
                        if let Some(callback) = &mut error_callback { callback(err); }
                    }
                },
                FileWorkerTask::Stop => {
                    break 'thread_loop;
                },
            }
        }));

        FileWorker { task_sender: tasks_sender, join_handle }
    }

    /// Write insert operation in the file in the background thread.
    pub fn write(&self, data: String) {
        let task = FileWorkerTask::Write(data);
        self.task_sender.send(task)
            .unwrap_or_else(|err| unreachable!(err)); // unreachable because channel receiver will drop only after out of thread and thread can't stop while FileWorkerTask::Stop is not received
    }
}

impl Drop for FileWorker {
    fn drop(&mut self) {
        self.task_sender.send(FileWorkerTask::Stop)
            .unwrap_or_else(|err| unreachable!(err)); // unreachable because thread can't stop while FileWorkerTask::Stop is not received
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
