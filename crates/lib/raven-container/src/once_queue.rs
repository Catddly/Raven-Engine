use std::collections::{VecDeque};

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
    job_name: String,
}

/// Queue to do all job once and check if they all success.
pub struct OnceQueue {
    init_queue: VecDeque<OnceJob>,
    shutdown_queue: VecDeque<OnceJob>,
}

impl OnceQueue {
    pub fn new() -> Self {
        Self {
            init_queue: VecDeque::new(),
            shutdown_queue: VecDeque::new(),
        }
    }

    pub fn push_job<F1, F2>(&mut self, init_func: F1, shutdown_func: F2)
    where
        F1: FnOnce() -> anyhow::Result<()> + 'static,
        F2: FnOnce() -> anyhow::Result<()> + 'static,
    {
        let init_func_name = std::any::type_name::<F1>().to_string();
        let shutdown_func_name = std::any::type_name::<F2>().to_string();

        self.init_queue.push_back(OnceJob {
            once: Once::new(),
            job: Box::new(init_func),
            job_name: init_func_name,
        });

        self.shutdown_queue.push_back(OnceJob {
            once: Once::new(),
            job: Box::new(shutdown_func),
            job_name: shutdown_func_name,
        });
    }

    pub fn initialize<'a>(&mut self) -> anyhow::Result<(), OnceQueueError> {
        let queue = self.init_queue.drain(..);
        Self::execute_impl(queue)
    }

    pub fn shutdown(&mut self) -> anyhow::Result<(), OnceQueueError> {
        let queue = self.shutdown_queue.drain(..).rev();
        Self::execute_impl(queue)
    }

    fn execute_impl(iter: impl Iterator<Item = OnceJob>) -> anyhow::Result<(), OnceQueueError> {
        for job in iter {
            let (job_func, name) = (job.job, job.job_name);
            
            // here we call unwarp(), it this once call failed, we get poisoned result.
            job.once.call_once(|| { job_func().unwrap() });
            let result = job.once.state();

            if let OnceState::Poisoned = &result {
                return Err(OnceQueueError::ExecutionPoisoned { func_name: name });
            }
        }

        Ok(())
    }

    #[inline]
    pub fn all_initialized(&self) -> bool {
        self.init_queue.is_empty()
    }

    #[inline]
    pub fn all_shutdowned(&self) -> bool {
        self.shutdown_queue.is_empty()
    }
}