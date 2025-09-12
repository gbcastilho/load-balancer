use std::time::Instant;

use rand::Rng;

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
#[derive(Clone, Copy)]
pub struct Request {
    pub id: usize,
    pub kind: RequestType,
    pub size: RequestSize,
    pub created_at: Instant,
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
            created_at: Instant::now(),
        }
    }
}
