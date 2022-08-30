use std::{
    thread,
    thread::JoinHandle,
    sync::{Arc, atomic::{AtomicBool, Ordering}},
};
use crossbeam_deque::{Worker as LocalQueue, Injector as GlobalQueue, Stealer};

use super::Job;

pub struct Worker {
    is_finish: Arc<AtomicBool>,
    // join handle of this worker thread
    handle: Option<JoinHandle<()>>,
    stealer: Option<Stealer<Job>>,
    /// Consider: Will this cause performance issues???
    local_queue: Option<LocalQueue<Job>>,
    // reference to global queue which can steal jobs from.
    global_queue: Arc<GlobalQueue<Job>>,
    id: String,
}

impl Worker {
    pub fn new(global_queue: Arc<GlobalQueue<Job>>, name: String) -> Self {
        let local_queue: LocalQueue<Job> = LocalQueue::new_fifo();

        Self {
            is_finish: Arc::new(AtomicBool::new(false)),
            handle: None,
            stealer: Some(local_queue.stealer()),
            local_queue: Some(local_queue),
            global_queue,
            id: name,
        }
    }

    /// Spawn thread and begin to execute jobs from queue.
    pub fn launch(&mut self, coworkers: Vec<Stealer<Job>>, flag: Arc<AtomicBool>) {
        // data that will be moved into thread scope
        let local_queue = self.local_queue.take().unwrap();
        let global_queue = self.global_queue.clone();
        let thread_name = self.id.to_owned();
        let finish = self.is_finish.clone();

        // spawn thread and store the thread handle
        self.handle = Some(thread::Builder::new()
            .name(thread_name.to_owned())
            .spawn(move || {
                let mut had_sent_finished = false;

                // if the flag is not true, keep pulling jobs from the queue
                while !flag.load(Ordering::SeqCst) {
                    // try pop job from the local queue first
                    if let Some(ref mut job) = local_queue.pop().or_else(|| {
                        // if no jobs in the local queue, try steal jobs from global queue
                        std::iter::repeat_with(|| {
                            global_queue.steal_batch_and_pop(&local_queue)
                                // failed to steal jobs from global queue, try steal jobs from other workers
                                .or_else(|| {
                                    coworkers.iter().map(|s| s.steal()).collect()
                                })
                        })
                        // loop until no jobs can be pulled from workers and any steal operations needs to be retired
                        .find(|s| !s.is_retry())
                        // successfully find a job, pull it out
                        .and_then(|s| s.success())
                    }) {
                        if had_sent_finished {
                            finish.fetch_and(false, Ordering::Release);
                            had_sent_finished = false;
                        }

                        job.execute()
                    } else {
                        if !had_sent_finished {
                            finish.fetch_or(true, Ordering::Release);
                            had_sent_finished = true;
                        }
                        std::thread::yield_now();
                    }
                }

                println!("{} finished!", thread_name);
            })
            .unwrap()
        );
    }

    /// Get a stealer from current worker thread.
    /// This can be done even if current thread is launched.
    pub fn stealer(&self) -> Stealer<Job> {
        if let Some(stealer) = &self.stealer {
            stealer.clone()
        } else {
            panic!("Please start worker before getting the stealer from worker!");
        }
    }

    /// Terminate a worker, it will be invalid forever.
    pub fn terminate(self) {
        if let Some(handle) = self.handle {
            handle.join().expect("Worker thread had been poisoned!")
        }
    }

    #[inline]
    pub fn is_finished(&self) -> bool {
        self.is_finish.load(Ordering::Acquire)
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.id.as_str()
    }
}