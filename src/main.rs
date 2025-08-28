use std::sync::{Arc, Mutex};
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
    let server1 = Server { id: 1, speed: 2000 };
    let server2 = Server { id: 2, speed: 1000 };
    let server3 = Server { id: 2, speed: 3000 };

    let counter = Arc::new(Mutex::new(10));

    let counter_clone1 = Arc::clone(&counter);
    let counter_clone2 = Arc::clone(&counter);
    let counter_clone3 = Arc::clone(&counter);

    let server1_handle = tokio::spawn(async move {
        loop {
            server1.process_package().await;
            let mut counter_guard = counter_clone1.lock().unwrap();
            *counter_guard -= 1;
            println!("{counter_guard} missing packages");
        }
    });
    let server2_handle = tokio::spawn(async move {
        loop {
            server2.process_package().await;
            let mut counter_guard = counter_clone2.lock().unwrap();
            *counter_guard -= 1;
            println!("{counter_guard} missing packages");
        }
    });
    let server3_handle = tokio::spawn(async move {
        loop {
            server3.process_package().await;
            let mut counter_guard = counter_clone3.lock().unwrap();
            *counter_guard -= 1;
            println!("{counter_guard} missing packages");
        }
    });

    loop {
        let counter_guard = counter.lock().unwrap();
        if *counter_guard <= 0 {
            server1_handle.abort();
            server2_handle.abort();
            server3_handle.abort();
            break;
        }
    }

    let _ = tokio::join!(server1_handle, server2_handle, server3_handle);
}
