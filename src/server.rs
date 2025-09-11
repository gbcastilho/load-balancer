use std::collections::VecDeque;

use crate::request::Request;

pub struct ServerState {
    pub id: u64,
    pub queue: VecDeque<Request>,
    pub total_workload: u64,
    pub is_processing: bool,
}

impl ServerState {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            queue: VecDeque::with_capacity(10),
            total_workload: 0,
            is_processing: false,
        }
    }

    pub fn add_request(&mut self, request: Request) {
        self.total_workload += request.get_time();
        self.queue.push_back(request);
    }

    pub fn remove_request(&mut self) -> Option<Request> {
        if let Some(request) = self.queue.pop_front() {
            self.total_workload = self.total_workload.saturating_sub(request.get_time());
            Some(request)
        } else {
            None
        }
    }
}
