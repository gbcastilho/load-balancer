use futures::future::join_all;
use std::sync::{Arc, RwLock};
use tokio::time::{Duration, sleep};

#[derive(Debug)]
pub struct Server {
    id: u64,
    speed: u64,
}

impl Server {
    async fn process_package(&self) {
        let timeout_ms = self.speed.max(1);

        sleep(Duration::from_millis(timeout_ms)).await;

        println!(
            "Server {} finished after {} milliseconds",
            self.id, self.speed
        );
    }
}

#[tokio::main]
async fn main() {
    let num_servers = 3;
    let counter = Arc::new(RwLock::new(10));

    println!("{} missing packages", counter.read().unwrap());

    let mut server_handles = Vec::with_capacity(num_servers);

    for i in 0..num_servers {
        let server = Server {
            id: (i as u64) + 1,
            speed: ((i as u64) + 1) * 1000,
        };

        let counter_clone = Arc::clone(&counter);

        let handle = tokio::spawn(async move {
            loop {
                server.process_package().await;
                let mut counter_guard = counter_clone.write().unwrap();
                *counter_guard -= 1;
                println!("{counter_guard} missing packages");
            }
        });

        server_handles.push(handle);
    }

    loop {
        let counter_guard = counter.read().unwrap();
        if *counter_guard <= 0 {
            for handle in &server_handles {
                handle.abort();
            }
            break;
        }
        drop(counter_guard);
        let _ = sleep(Duration::from_millis(100));
    }

    let _ = join_all(server_handles).await;
}
