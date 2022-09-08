use std::collections::VecDeque;

use crate::reflection::function::*;

type OnceJob = Box<dyn FnOnce() -> anyhow::Result<()> + 'static>;

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
        let func_name = get_function_name(&func).to_string();
        self.queue.push_back((Box::new(func), func_name));
    }

    pub fn execute(&mut self) -> anyhow::Result<()> {
        let drained_queue = self.queue.drain(..);

        for job in drained_queue {
            let (job, name) = job;
            
            let result = job();
            if let Err(err) = &result {
                // log module may not be initialized, so use eprintln! here.
                eprintln!("Failed to execute once queue job: {}, with error: {}", name, err);
                return result;
            }
        }

        Ok(())
    }

    #[inline]
    pub fn is_finished(&self) -> bool {
        self.queue.is_empty()
    }
}