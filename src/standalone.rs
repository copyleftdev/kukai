use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;
use crate::token_bucket::TokenBucket;

pub async fn send_request(request_id: usize, client: &Client, target_url: &str) -> Result<()> {
    println!("Dispatching request {} at {:?}", request_id, Instant::now());
    let response = client.get(target_url).send().await?;
    println!("Completed request {} with status {} at {:?}", request_id, response.status(), Instant::now());
    Ok(())
}


pub async fn run_standalone() -> Result<()> {
    let test_duration = Duration::from_secs(30);
    let target_rate = 5.0;
    let bucket_capacity = 10;
    let target_url = "http://example.com";
    let client = Client::new();
    let token_bucket = Arc::new(Mutex::new(TokenBucket::new(bucket_capacity, target_rate)));
    let start_time = Instant::now();
    let mut request_count: usize = 0;
    while Instant::now().duration_since(start_time) < test_duration {
        {
            let mut bucket = token_bucket.lock().await;
            if bucket.try_consume(1.0).await {
                request_count += 1;
                let request_id = request_count;
                let client = client.clone();
                let url = target_url.to_string();
                tokio::spawn(async move {
                    if let Err(e) = send_request(request_id, &client, &url).await {
                        eprintln!("Error sending request {}: {}", request_id, e);
                    }
                });
            }
        }
        sleep(Duration::from_millis(10)).await;
    }
    println!("Load test completed. Total requests dispatched: {}", request_count);
    Ok(())
}

fn calculate_request_interval(rate: f64) -> Duration {
    Duration::from_secs_f64(1.0 / rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_calculate_request_interval() {
        let interval = calculate_request_interval(5.0);
        assert_eq!(interval, Duration::from_millis(200));
    }
}
