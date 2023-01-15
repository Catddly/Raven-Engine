use std::collections::VecDeque;

use parking_lot::{Once, OnceState};
use thiserror::Error;

type OnceJobFunc = Box<dyn FnOnce() -> anyhow::Result<()> + 'static>;

#[derive(Debug, Error)]
pub enum OnceQueueError {
    #[error("Once queue execution failed on {func_name}")]
    ExecutionPoisoned {
        func_name: String,
    },
}

struct OnceJob {
    once: Once,
    job: OnceJobFunc,
}

/// Queue to do all job once and check if they all success.
pub struct OnceQueue {
    queue: VecDeque<(OnceJob, String)>,
}

impl OnceQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn push_job<F>(&mut self, func: F)
    where
        F: FnOnce() -> anyhow::Result<()> + 'static,
    {
        let func_name = std::any::type_name::<F>().to_string();
        self.queue.push_back((OnceJob {
            once: Once::new(),
            job: Box::new(func)
        }, func_name));
    }

    pub fn execute(&mut self) -> anyhow::Result<(), OnceQueueError> {
        let drained_queue = self.queue.drain(..);
        Self::execute_impl(drained_queue)
    }

    pub fn execute_backwards(&mut self) -> anyhow::Result<(), OnceQueueError> {
        let drained_queue = self.queue.drain(..).rev();
        Self::execute_impl(drained_queue)
    }

    fn execute_impl(iter: impl Iterator<Item = (OnceJob, String)>) -> anyhow::Result<(), OnceQueueError> {
        for job in iter {
            let (job, name) = job;
            let job_func = job.job;

            // here we call unwarp(), it this once call failed, we get poisoned result.
            job.once.call_once(move || { job_func().unwrap() });
            let result = job.once.state();

            if let OnceState::Poisoned = &result {
                return Err(OnceQueueError::ExecutionPoisoned { func_name: name });
            }
        }

        Ok(())
    }

    #[inline]
    pub fn is_finished(&self) -> bool {
        self.queue.is_empty()
    }
}