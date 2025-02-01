// src/standalone.rs
use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;
use crate::token_bucket::TokenBucket;

async fn send_request(request_id: usize, client: &Client, target_url: &str) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_token_bucket() {
        let mut bucket = TokenBucket::new(5, 1.0);
        assert!(bucket.try_consume(3.0).await);
        assert!(!bucket.try_consume(3.0).await);
        sleep(Duration::from_secs(3)).await;
        assert!(bucket.try_consume(3.0).await);
    }
}
