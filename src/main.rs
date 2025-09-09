mod entities;

use crossterm::{ExecutableCommand, cursor, terminal};
use entities::{Request, Server};
use futures::future::join_all;
use rand::{Rng, SeedableRng};
use std::collections::VecDeque;
use std::io;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};

fn reset_line() {
    let mut stdout = io::stdout();
    let _ = stdout.execute(cursor::MoveToColumn(0));
    let _ = stdout.execute(terminal::Clear(terminal::ClearType::CurrentLine));
}

#[tokio::main]
async fn main() {
    let avg_rate = 300;
    let mut choice_mode = ServerChoiceMode::Random;

    let requests_queue = Arc::new(RwLock::new(VecDeque::new()));
    let req_emit_queue = Arc::clone(&requests_queue);

    let req_emit_handle = tokio::spawn(async move {
        check_n_emit_request(avg_rate, req_emit_queue).await;
    });

    let servers = Arc::new([
        Arc::new(RwLock::new(Server {
            id: 1,
            queue: VecDeque::with_capacity(10),
        })),
        Arc::new(RwLock::new(Server {
            id: 2,
            queue: VecDeque::with_capacity(10),
        })),
        Arc::new(RwLock::new(Server {
            id: 3,
            queue: VecDeque::with_capacity(10),
        })),
    ]);

    let servers_for_alloc = Arc::clone(&servers);
    let alloc_server_handle = tokio::spawn(async move {
        loop {
            alloc_req_to_server(&servers_for_alloc, &mut choice_mode, &requests_queue).await;
        }
    });

    let mut loop_handles = Vec::with_capacity(5);
    for server in servers.iter() {
        let server_clone = Arc::clone(server);
        loop_handles.push(tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(10));

            loop {
                ticker.tick().await;
                let mut server_guard = server_clone.write().await;
                server_guard.process_request().await;
            }
        }))
    }

    loop_handles.push(req_emit_handle);
    loop_handles.push(alloc_server_handle);

    let _ = join_all(loop_handles).await;
}

async fn check_n_emit_request(avg_rate: usize, req_queue: Arc<RwLock<VecDeque<Request>>>) {
    let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());

    let mut ticker = interval(Duration::from_millis(10));
    loop {
        ticker.tick().await;
        let lottery_number = rng.random_range(0..1000);
        if lottery_number < (avg_rate / 100) {
            let new_req = Request::create_random();
            println!("Request {} arrived", new_req.id);

            let mut req_queue_guard = req_queue.write().await;
            req_queue_guard.push_back(new_req);
        }
    }
}

enum ServerChoiceMode {
    Random,
    RoundRobin { server_num: usize },
    SmallerQueue,
}

impl ServerChoiceMode {
    async fn choose(
        servers: &[Arc<RwLock<Server>>; 3],
        choice_mode: &mut ServerChoiceMode,
        request: Request,
    ) {
        let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());

        let chosen_idx = match choice_mode {
            ServerChoiceMode::Random => rng.random_range(0..servers.len()),
            ServerChoiceMode::RoundRobin { server_num } => {
                let idx = *server_num;

                *server_num = (idx + 1) % servers.len();
                idx
            }
            ServerChoiceMode::SmallerQueue => {
                let mut server_workloads = Vec::with_capacity(servers.len());
                for (idx, server) in servers.iter().enumerate() {
                    let server_guard = server.read().await;
                    let workload = server_guard
                        .queue
                        .iter()
                        .map(|req| req.get_time())
                        .sum::<u64>();

                    server_workloads.push((idx, workload));
                }

                server_workloads
                    .iter()
                    .min_by_key(|(_, workload)| workload)
                    .map(|(idx, _)| *idx)
                    .unwrap_or(0)
            }
        };

        let mut server_guard = servers[chosen_idx].write().await;
        server_guard.queue.push_back(request);
    }
}

async fn alloc_req_to_server(
    servers: &[Arc<RwLock<Server>>; 3],
    choice_mode: &mut ServerChoiceMode,
    req_queue: &Arc<RwLock<VecDeque<Request>>>,
) {
    let mut ticker = interval(Duration::from_millis(3000));

    loop {
        ticker.tick().await;
        let mut req_queue_guard = req_queue.write().await;
        let request = match req_queue_guard.pop_front() {
            Some(req) => req,
            None => return,
        };

        let _ = ServerChoiceMode::choose(servers, choice_mode, request).await;
    }
}
