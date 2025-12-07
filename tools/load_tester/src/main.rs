use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use reqwest::Client;
use tokio::sync::Semaphore;

use platform::{record_counter, record_histogram};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "load_tester",
    version,
    about = "Simple HTTP load tester for /api/v1/chat"
)]
struct Args {
    /// Target endpoint (e.g. http://127.0.0.1:8181/api/v1/chat)
    #[arg(long)]
    endpoint: String,

    /// Maximum number of concurrent in-flight requests
    #[arg(long, default_value_t = 16)]
    concurrency: usize,

    /// Duration to run the test (in seconds)
    #[arg(long, default_value_t = 60)]
    duration_secs: u64,

    /// Message text to send in the chat payload
    #[arg(long, default_value = "ping")]
    message: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize basic logging and metrics context for the tool.
    if let Err(e) = platform::init_tracing("load_tester") {
        eprintln!("failed to init tracing: {e}");
    }

    let client = Client::new();
    let semaphore = Arc::new(Semaphore::new(args.concurrency));
    let end_at = Instant::now() + Duration::from_secs(args.duration_secs);

    let mut handles = Vec::with_capacity(args.concurrency);

    for _ in 0..args.concurrency {
        let client = client.clone();
        let sem = semaphore.clone();
        let endpoint = args.endpoint.clone();
        let message = args.message.clone();
        let end_at = end_at;

        let handle = tokio::spawn(async move {
            loop {
                if Instant::now() >= end_at {
                    break;
                }

                let permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                let start = Instant::now();
                let payload = serde_json::json!({
                    "message": message,
                    // Let the server default the api_version and correlation_id.
                });

                let res = client.post(&endpoint).json(&payload).send().await;

                let duration = start.elapsed().as_secs_f64();
                record_counter("load_tester_requests_total", 1);
                record_histogram("load_tester_request_duration_seconds", duration);

                drop(permit);

                if let Err(e) = res {
                    eprintln!("request failed: {e}");
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    println!(
        "Load test complete: endpoint={}, concurrency={}, duration_secs={}",
        args.endpoint, args.concurrency, args.duration_secs
    );
}
