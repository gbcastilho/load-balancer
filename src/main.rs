mod display;
mod request;
mod server;

use rand::{Rng, SeedableRng};
use request::Request;
use std::collections::VecDeque;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{Duration, interval};

use crate::display::run_ui;
use crate::server::ServerState;

enum ServerChoiceMode {
    Random,
    RoundRobin { server_num: usize },
    SmallerQueue,
}

#[derive(Clone, Copy)]
enum SystemEvent {
    RequestCreated(Request),
    RequestAssigned { server_id: u64, request: Request },
    RequestProcessed { request_id: usize, server_id: u64 },
}

pub struct SystemState {
    pending_requests: VecDeque<Request>,
    servers: [ServerState; 3],
    logs: Vec<String>,
    stats: SystemStats,
}

pub struct SystemStats {
    total_requests: usize,
    processed_requests: usize,
    avg_wait_time: f64,
}

const REQUEST_AVG_RATE: i32 = 3; // requests/second

#[tokio::main]
async fn main() {
    let (main_tx, main_rx) = mpsc::channel::<SystemEvent>(100);

    let (allocator_tx, allocator_rx) = mpsc::channel::<SystemEvent>(100);
    let (server_tx, server_rx) = mpsc::channel::<SystemEvent>(100);
    let (ui_tx, ui_rx) = mpsc::channel::<SystemEvent>(100);

    let router_handle = spawn_event_router(main_rx, allocator_tx, server_tx, ui_tx);

    let gen_handle = spawn_request_generator(main_tx.clone());
    let alloc_handle = spawn_request_allocator(main_tx.clone(), allocator_rx);
    let server_handle = spawn_servers(main_tx.clone(), server_rx);

    let ui_handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = run_ui(ui_rx) {
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
                    server_tx.send(event.clone()).await.ok();

                    ui_tx.send(event).await.ok();
                }
                SystemEvent::RequestProcessed { .. } => {
                    allocator_tx.send(event.clone()).await.ok();
                    server_tx.send(event.clone()).await.ok();

                    ui_tx.send(event).await.ok();
                }
            }
        }
    })
}

fn spawn_request_generator(event_tx: Sender<SystemEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());
        let mut ticker = interval(Duration::from_millis(100));

        loop {
            if rng.random_range(0..10) < REQUEST_AVG_RATE {
                let request = Request::create_random();

                event_tx
                    .send(SystemEvent::RequestCreated(request.clone()))
                    .await
                    .ok();
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

        loop {
            while let Ok(event) = event_rx.try_recv() {
                match event {
                    SystemEvent::RequestCreated(request) => {
                        requests.push_back(request);
                    }
                    SystemEvent::RequestProcessed {
                        request_id: _,
                        server_id,
                    } => {
                        let server_idx = (server_id - 1) as usize;

                        server_states[server_idx].remove_request();
                        server_states[server_idx].is_processing = false;
                    }
                    _ => {}
                }
            }

            if !requests.is_empty() {
                let server_idx = match choice_mode {
                    ServerChoiceMode::Random => rng.random_range(0..3),
                    ServerChoiceMode::RoundRobin { ref mut server_num } => {
                        let idx = *server_num;
                        *server_num = (*server_num + 1) % 3;
                        idx
                    }
                    ServerChoiceMode::SmallerQueue => server_states
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, state)| state.total_workload)
                        .map(|(idx, _)| idx)
                        .unwrap_or(0),
                };

                let server = &mut server_states[server_idx];

                if server.queue.len() >= server.queue.capacity() {
                    continue;
                }

                let request = requests.pop_front().unwrap();

                server.add_request(request.clone());

                event_tx
                    .send(SystemEvent::RequestAssigned {
                        server_id: server.id,
                        request: request,
                    })
                    .await
                    .ok();
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
                    server.is_processing = true;

                    if let Some(request) = server.remove_request() {
                        let server_id = server.id;
                        let event_tx = event_tx.clone();

                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_millis(request.get_time())).await;

                            event_tx
                                .send(SystemEvent::RequestProcessed {
                                    request_id: request.id,
                                    server_id,
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
