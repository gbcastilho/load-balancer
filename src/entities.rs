use std::{fmt::Debug, time::Duration};

use rand::Rng;
use tokio::time::{self, sleep};

use crate::display::log_debug;

#[derive(Debug, Clone, Copy)]
pub enum RequestSize {
    Small,
    Mid,
    Large,
}

impl RequestSize {
    fn mult_factor(&self) -> u64 {
        match self {
            RequestSize::Small => 5,
            RequestSize::Mid => 10,
            RequestSize::Large => 50,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RequestType {
    CPUsBound,
    IOBound,
    Mixed,
}

impl RequestType {
    fn cpu_time(&self) -> u64 {
        match self {
            RequestType::CPUsBound => 95,
            RequestType::IOBound => 30,
            RequestType::Mixed => 55,
        }
    }

    fn io_time(&self) -> u64 {
        match self {
            RequestType::CPUsBound => 5,
            RequestType::IOBound => 70,
            RequestType::Mixed => 45,
        }
    }
}

pub struct Request {
    pub id: usize,
    pub kind: RequestType,
    pub size: RequestSize,
    pub arrived_at: time::Instant,
    pub finished_at: Option<time::Instant>,
}

impl Request {
    pub fn get_time(&self) -> u64 {
        let total_time = self.kind.cpu_time() + self.kind.io_time();
        total_time * self.size.mult_factor()
    }

    pub fn get_name(&self) -> String {
        format!("{:?} {:?}", self.size, self.kind)
    }

    pub fn create_random() -> Self {
        let mut rng = rand::rng();

        const REQ_TYPES: [RequestType; 3] = [
            RequestType::CPUsBound,
            RequestType::IOBound,
            RequestType::Mixed,
        ];
        const REQ_SIZES: [RequestSize; 3] =
            [RequestSize::Small, RequestSize::Mid, RequestSize::Large];

        Self {
            id: rng.random_range(1000000..10000000),
            kind: REQ_TYPES[rng.random_range(0..REQ_TYPES.len())],
            size: REQ_SIZES[rng.random_range(0..REQ_SIZES.len())],
            arrived_at: time::Instant::now(),
            finished_at: None,
        }
    }
}

pub struct Server {
    pub id: u64,
    pub queue: std::collections::VecDeque<Request>,
}

impl Server {
    pub async fn process_request(&mut self) {
        if let Some(request) = self.queue.pop_front() {
            sleep(Duration::from_millis(request.get_time())).await;
            log_debug(format!("Server {} processed #{}", self.id, request.id));
        }
    }
}
