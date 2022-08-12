use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub type JobFunc = Box<dyn FnOnce() -> () + Send + 'static>;

pub struct Job {
    func: Option<JobFunc>,
    complete: Arc<AtomicBool>,
}

unsafe impl Send for Job {}

impl Job {
    pub(super) fn new(func: JobFunc) -> Self {
        Self {
            func: Some(func),
            complete: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn execute(&mut self) {
        // function call only be executed once.
        if let Some(func) = self.func.take() {
            func();
        }
        self.complete.store(true, Ordering::Relaxed);
    }

    pub fn handle(&self) -> JobHandle {
        JobHandle::new(self.complete.clone())
    }
}

/// Job handle to check if a job is done.
pub struct JobHandle {
    complete: Arc<AtomicBool>,
}

impl JobHandle {
    fn new(complete: Arc<AtomicBool>) -> Self {
        Self { complete }
    }

    /// If the job is completed.
    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Relaxed)
    }

    /// Wait for current job to complete.
    /// It will block current thread until the thread pool finished the job.
    pub fn wait(&self) {
        // self spin to wait for job to complete
        while !self.is_complete() {
            std::thread::yield_now();
        }
    }
}