mod display;
mod request;
mod server;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use request::Request;
use std::collections::VecDeque;
use std::fmt;
use std::time::Instant;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{Duration, interval};

use crate::display::run_ui;
use crate::server::ServerState;

const INITIAL_AVG_RATE: i32 = 3; // requests/second
pub const PENDING_REQUESTS_LIMIT: i32 = 20;

#[derive(Clone)]
enum ServerChoiceMode {
    Random,
    RoundRobin { server_num: usize },
    SmallerQueue,
}

impl fmt::Display for ServerChoiceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Random => write!(f, "Random"),
            Self::RoundRobin { .. } => write!(f, "Round Robin"),
            Self::SmallerQueue => write!(f, "Smaller Queue"),
        }
    }
}

impl ServerChoiceMode {
    fn choose(&mut self, server_states: &[ServerState; 3], rng: &mut StdRng) -> Vec<usize> {
        let indices = match self {
            ServerChoiceMode::Random => {
                let mut indices = vec![0, 1, 2];
                indices.shuffle(rng);
                indices
            }
            ServerChoiceMode::RoundRobin { server_num } => {
                let start = *server_num;
                *server_num = (*server_num + 1) % 3;
                vec![start, (start + 1) % 3, (start + 2) % 3]
            }
            ServerChoiceMode::SmallerQueue => {
                let mut servers_by_load: Vec<(usize, u64)> = server_states
                    .iter()
                    .enumerate()
                    .map(|(idx, state)| (idx, state.total_workload))
                    .collect();

                servers_by_load.sort_by_key(|(_, workload)| *workload);
                servers_by_load.into_iter().map(|(idx, _)| idx).collect()
            }
        };
        indices
    }
}

struct SystemConfig {
    arrival_rate: f32,
    choice_mode: ServerChoiceMode,
}

#[derive(Clone)]
enum SystemEvent {
    RequestCreated(Request),
    RequestAssigned {
        server_id: u64,
        request: Request,
    },
    RequestProcessStarted {
        request_id: usize,
        server_id: u64,
    },
    RequestProcessed {
        request_id: usize,
        server_id: u64,
        created_at: Instant,
    },
    ErrorEncountered(String),
    ConfigChanged {
        arrival_rate: Option<f32>,
        choice_mode: Option<ServerChoiceMode>,
    },
}

pub struct SystemState {
    pending_requests: VecDeque<Request>,
    servers: [ServerState; 3],
    logs: Vec<String>,
    configs: SystemConfig,
    stats: SystemStats,
}

pub struct SystemStats {
    total_requests: usize,
    processed_requests: usize,
    avg_wait_time: f64,
    throughput: f64,
    throughput_window: Vec<Instant>,
}

#[tokio::main]
async fn main() {
    let (main_tx, main_rx) = mpsc::channel::<SystemEvent>(1000);

    let (gen_tx, gen_rx) = mpsc::channel::<SystemEvent>(1000);
    let (allocator_tx, allocator_rx) = mpsc::channel::<SystemEvent>(1000);
    let (server_tx, server_rx) = mpsc::channel::<SystemEvent>(1000);
    let (ui_tx, ui_rx) = mpsc::channel::<SystemEvent>(1000);

    let router_handle = spawn_event_router(main_rx, gen_tx, allocator_tx, server_tx, ui_tx);

    let gen_handle = spawn_request_generator(main_tx.clone(), gen_rx);
    let alloc_handle = spawn_request_allocator(main_tx.clone(), allocator_rx);
    let server_handle = spawn_servers(main_tx.clone(), server_rx);

    let ui_handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = run_ui(main_tx.clone(), ui_rx) {
            eprintln!("UI error: {}", e);
        }
    });

    ui_handle.await.unwrap();

    router_handle.abort();
    gen_handle.abort();
    alloc_handle.abort();
    server_handle.abort();
}

fn spawn_event_router(
    mut event_rx: Receiver<SystemEvent>,
    gen_tx: Sender<SystemEvent>,
    allocator_tx: Sender<SystemEvent>,
    server_tx: Sender<SystemEvent>,
    ui_tx: Sender<SystemEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                SystemEvent::RequestCreated(_) => {
                    allocator_tx.send(event.clone()).await.ok();

                    ui_tx.send(event).await.ok();
                }
                SystemEvent::RequestAssigned { .. } => {
                    gen_tx.send(event.clone()).await.ok();
                    server_tx.send(event.clone()).await.ok();

                    ui_tx.send(event).await.ok();
                }
                SystemEvent::RequestProcessed { .. } => {
                    allocator_tx.send(event.clone()).await.ok();
                    server_tx.send(event.clone()).await.ok();

                    ui_tx.send(event).await.ok();
                }
                SystemEvent::RequestProcessStarted { .. } => {
                    ui_tx.send(event.clone()).await.ok();
                }
                SystemEvent::ErrorEncountered(_) => {
                    ui_tx.send(event.clone()).await.ok();
                }
                SystemEvent::ConfigChanged { .. } => {
                    gen_tx.send(event.clone()).await.ok();
                    allocator_tx.send(event.clone()).await.ok();

                    ui_tx.send(event.clone()).await.ok();
                }
            }
        }
    })
}

fn spawn_request_generator(
    event_tx: Sender<SystemEvent>,
    mut event_rx: Receiver<SystemEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut arrival_rate = INITIAL_AVG_RATE as f32;

        let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());
        let mut ticker = interval(Duration::from_millis(100));

        let mut pending_requests = 0;

        loop {
            if pending_requests < PENDING_REQUESTS_LIMIT
                && rng.random_range(0.0..10.0) < arrival_rate
            {
                let request = Request::create_random();

                event_tx
                    .send(SystemEvent::RequestCreated(request.clone()))
                    .await
                    .ok();

                pending_requests += 1;
            }

            while let Ok(event) = event_rx.try_recv() {
                match event {
                    SystemEvent::RequestAssigned { .. } => {
                        pending_requests -= 1;
                    }
                    SystemEvent::ConfigChanged {
                        arrival_rate: Some(new_rate),
                        ..
                    } => arrival_rate = new_rate,
                    _ => {}
                }
            }

            ticker.tick().await;
        }
    })
}

fn spawn_request_allocator(
    event_tx: Sender<SystemEvent>,
    mut event_rx: Receiver<SystemEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut server_states = [
            ServerState::new(1),
            ServerState::new(2),
            ServerState::new(3),
        ];
        let mut requests = VecDeque::new();
        let mut choice_mode = ServerChoiceMode::Random;
        let mut ticker = interval(Duration::from_millis(50));

        let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());

        let mut consecutive_full_errors = 0;
        let mut full_server = [false; 3];

        loop {
            while let Ok(event) = event_rx.try_recv() {
                match event {
                    SystemEvent::RequestCreated(request) => {
                        requests.push_back(request);
                    }
                    SystemEvent::RequestProcessed {
                        request_id: _,
                        server_id,
                        created_at: _,
                    } => {
                        let server_idx = (server_id - 1) as usize;

                        server_states[server_idx].remove_request();
                        server_states[server_idx].is_processing = false;
                    }
                    SystemEvent::ConfigChanged {
                        choice_mode: Some(new_mode),
                        ..
                    } => choice_mode = new_mode,
                    _ => {}
                }
            }

            if !requests.is_empty() {
                let mut assigned = true;

                let server_indices = choice_mode.choose(&server_states, &mut rng);

                for &idx in &server_indices {
                    let server = &mut server_states[idx];

                    if server.queue.len() < server.queue.capacity() {
                        let request = requests.pop_front().unwrap();
                        server.add_request(request.clone());

                        event_tx
                            .send(SystemEvent::RequestAssigned {
                                server_id: server.id,
                                request: request,
                            })
                            .await
                            .ok();

                        assigned = true;
                        break;
                    } else {
                        full_server[idx] = true;
                    }
                }

                if !assigned && full_server == [true; 3] {
                    consecutive_full_errors += 1;

                    if consecutive_full_errors % 10 == 1 {
                        event_tx
                            .send(SystemEvent::ErrorEncountered(format!(
                                "All servers are full",
                            )))
                            .await
                            .ok();
                    }

                    if consecutive_full_errors > 5 {
                        tokio::time::sleep(Duration::from_millis(
                            50 * consecutive_full_errors.min(20),
                        ))
                        .await;
                    }
                }
            }

            ticker.tick().await;
        }
    })
}

fn spawn_servers(
    event_tx: Sender<SystemEvent>,
    mut event_rx: Receiver<SystemEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut servers = [
            ServerState::new(1),
            ServerState::new(2),
            ServerState::new(3),
        ];

        let mut ticker = interval(Duration::from_millis(10));

        loop {
            while let Ok(event) = event_rx.try_recv() {
                match event {
                    SystemEvent::RequestAssigned { server_id, request } => {
                        let server_idx = (server_id - 1) as usize;
                        if server_idx < servers.len() {
                            let server = &mut servers[server_idx];

                            server.add_request(request);
                        }
                    }
                    SystemEvent::RequestProcessed {
                        request_id: _,
                        server_id,
                        created_at: _,
                    } => {
                        let server_idx = (server_id - 1) as usize;
                        if server_idx < servers.len() {
                            let server = &mut servers[server_idx];

                            server.is_processing = false;
                        }
                    }
                    _ => {}
                }
            }

            for server in &mut servers {
                if !server.queue.is_empty() && !server.is_processing {
                    if let Some(request) = server.remove_request() {
                        server.is_processing = true;
                        let server_id = server.id;
                        let event_tx = event_tx.clone();

                        tokio::spawn(async move {
                            event_tx
                                .send(SystemEvent::RequestProcessStarted {
                                    request_id: request.id,
                                    server_id: server_id,
                                })
                                .await
                                .ok();

                            tokio::time::sleep(Duration::from_millis(request.get_time())).await;

                            event_tx
                                .send(SystemEvent::RequestProcessed {
                                    server_id,
                                    request_id: request.id,
                                    created_at: request.created_at,
                                })
                                .await
                                .ok();
                        });
                    }
                }
            }

            ticker.tick().await;
        }
    })
}
