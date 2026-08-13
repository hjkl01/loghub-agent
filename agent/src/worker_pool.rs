use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone, Debug)]
pub struct WorkerPoolConfig {
    pub workers: usize,
    pub max_inflight: usize,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            workers: 4,
            max_inflight: 16,
        }
    }
}

#[derive(Clone)]
pub struct WorkerLimiter {
    semaphore: Arc<Semaphore>,
}

impl WorkerLimiter {
    pub fn new(max_inflight: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_inflight)),
        }
    }

    pub async fn acquire(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.semaphore.clone().acquire_owned().await.unwrap()
    }
}
