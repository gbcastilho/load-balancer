use crossterm::{ExecutableCommand, cursor, terminal};
use futures::future::join_all;
use rand::Rng;
use std::io::{self, Write};
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

        // println!(
        //     "Server {} finished after {} milliseconds",
        //     self.id, self.speed
        // );
    }
}

fn print_progress_bar(current: usize, total: usize) {
    let bar_width = 50;
    let progress = (bar_width as f64 * (total - current) as f64 / total as f64) as usize;

    let mut bar = String::with_capacity(bar_width + 2);
    bar.push('[');
    for i in 0..bar_width {
        if i < progress {
            bar.push('#');
        } else {
            bar.push('-');
        }
    }
    bar.push(']');

    reset_line();
    print!("{} {}/{}", bar, total - current, total);
    io::stdout().flush().unwrap();
}

fn reset_line() {
    let mut stdout = io::stdout();
    let _ = stdout.execute(cursor::MoveToColumn(0));
    let _ = stdout.execute(terminal::Clear(terminal::ClearType::CurrentLine));
}

#[tokio::main]
async fn main() {
    print!("Number of servers: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read input");

    let num_servers: usize = match input.trim().parse() {
        Ok(n) if n > 0 => n,
        Ok(_) => 1,
        Err(_) => {
            println!("Invalid input, using 1 server");
            1
        }
    };

    let total_packages = 10;
    let counter = Arc::new(RwLock::new(total_packages));

    let mut server_handles = Vec::with_capacity(num_servers);
    let mut rng = rand::rng();

    for i in 0..num_servers {
        let server = Server {
            id: (i as u64) + 1,
            speed: rng.random_range(1000..=5000),
        };

        let counter_clone = Arc::clone(&counter);

        let handle = tokio::spawn(async move {
            loop {
                server.process_package().await;
                let mut counter_guard = counter_clone.write().unwrap();
                *counter_guard -= 1;

                reset_line();
                println!("Server {} processed package", server.id);

                if *counter_guard <= 0 {
                    break;
                }
            }
        });

        server_handles.push(handle);
    }

    loop {
        let counter_guard = counter.read().unwrap();
        print_progress_bar(*counter_guard, total_packages);
        if *counter_guard <= 0 {
            for handle in &server_handles {
                handle.abort();
            }
            break;
        }
        drop(counter_guard);
        let _ = sleep(Duration::from_millis(100)).await;
    }

    let _ = join_all(server_handles).await;
    println!();
}
