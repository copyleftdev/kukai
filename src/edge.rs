use anyhow::Result;
use std::sync::Arc;
use std::time::{Instant, Duration, SystemTime, UNIX_EPOCH};
use rand::{Rng, rngs::StdRng, SeedableRng};
use tokio::time::sleep;
use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    middleware::NoOpMiddleware,
    RateLimiter,
    Quota,
};
use std::num::NonZeroU32;
use arrow_flight::flight_service_client::FlightServiceClient;
use arrow_flight::FlightData;
use tonic::Request;
use tracing::{info, warn};

use crate::config::{LoadConfig, EdgeConfig, Target};

// Define the rate limiter type at module level
type MyRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>;

#[derive(Clone)]
struct AttemptMetric {
    ts_micros: i64,
    target: String,
    success: bool,
    latency_us: u64,
}

pub async fn run_edge(load: &LoadConfig, edge: &EdgeConfig) -> Result<()> {
    info!("Edge mode => Commander at {}", edge.commander_address);
    info!("Edge RPS: {}", load.rps);

    let reuse_conn = load.reuse_connection.unwrap_or(false);
    if reuse_conn {
        info!("Connection reuse: enabled.");
    } else {
        info!("Connection reuse: disabled.");
    }

    let metrics = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let end_time = Instant::now() + Duration::from_secs(load.duration_seconds as u64);

    // Create rate limiter with proper quota
    let q = Quota::per_second(
        NonZeroU32::new(load.rps as u32)
            .unwrap_or_else(|| NonZeroU32::new(1).unwrap())
    );
    let limiter = Arc::new(MyRateLimiter::direct(q));

    let mut tasks = vec![];
    for _ in 0..load.concurrency {
        let m_ref = Arc::clone(&metrics);
        let tlist = load.targets.clone();
        let pay = load.payload.clone();
        let lim_clone = Arc::clone(&limiter);
        let reuse = reuse_conn;

        tasks.push(tokio::spawn(async move {
            if reuse {
                run_worker_reuse(m_ref, &tlist, &pay, end_time, lim_clone).await;
            } else {
                run_worker_fresh(m_ref, &tlist, &pay, end_time, lim_clone).await;
            }
        }));
    }

    // Flush metrics in background
    let flush_m = Arc::clone(&metrics);
    let flush_addr = edge.commander_address.clone();
    let flusher = tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(2)).await;
            let mut local = {
                let mut g = flush_m.lock().await;
                std::mem::take(&mut *g)
            };
            if !local.is_empty() {
                let _ = push_metrics(&flush_addr, &mut local).await;
            }
        }
    });

    for t in tasks {
        let _ = t.await;
    }

    // Final flush
    {
        let mut leftover = {
            let mut g = metrics.lock().await;
            std::mem::take(&mut *g)
        };
        if !leftover.is_empty() {
            push_metrics(&edge.commander_address, &mut leftover).await?;
        }
    }
    flusher.abort();

    info!("Edge mode complete.");
    Ok(())
}

async fn run_worker_fresh(
    metrics: Arc<tokio::sync::Mutex<Vec<AttemptMetric>>>,
    targets: &[Target],
    payload: &str,
    end_time: Instant,
    limiter: Arc<MyRateLimiter>,
) {
    let mut rng = StdRng::from_entropy();
    while Instant::now() < end_time {
        limiter.until_ready().await;

        let t = pick_target(targets, &mut rng);
        let addr_str = format!("{}:{}", t.addr, t.port);
        let start = Instant::now();
        let success = match tokio::net::TcpStream::connect(&addr_str).await {
            Ok(mut s) => {
                use tokio::io::AsyncWriteExt;
                s.write_all(payload.as_bytes()).await.is_ok()
            }
            Err(_) => false,
        };
        let latency_us = start.elapsed().as_micros() as u64;

        {
            let mut g = metrics.lock().await;
            g.push(AttemptMetric {
                ts_micros: now_micros(),
                target: addr_str,
                success,
                latency_us,
            });
        }
        sleep(Duration::from_millis(5)).await;
    }
}

async fn run_worker_reuse(
    metrics: Arc<tokio::sync::Mutex<Vec<AttemptMetric>>>,
    targets: &[Target],
    payload: &str,
    end_time: Instant,
    limiter: Arc<MyRateLimiter>,
) {
    let mut rng = StdRng::from_entropy();
    let t = pick_target(targets, &mut rng);
    let addr_str = format!("{}:{}", t.addr, t.port);

    let mut stream = match tokio::net::TcpStream::connect(&addr_str).await {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to connect for reuse: {:?}", e);
            return;
        }
    };

    while Instant::now() < end_time {
        limiter.until_ready().await;

        let start = Instant::now();
        let success = {
            use tokio::io::AsyncWriteExt;
            stream.write_all(payload.as_bytes()).await.is_ok()
        };
        let latency_us = start.elapsed().as_micros() as u64;

        {
            let mut g = metrics.lock().await;
            g.push(AttemptMetric {
                ts_micros: now_micros(),
                target: addr_str.clone(),
                success,
                latency_us,
            });
        }
        sleep(Duration::from_millis(5)).await;
    }
}

fn pick_target(targets: &[Target], rng: &mut StdRng) -> Target {
    let sum: f32 = targets.iter().map(|x| x.weight).sum();
    if sum <= 0.0 {
        return targets[0].clone();
    }
    let mut roll = rng.gen_range(0.0..sum);
    for t in targets {
        if roll < t.weight {
            return t.clone();
        }
        roll -= t.weight;
    }
    targets[targets.len() - 1].clone()
}

fn now_micros() -> i64 {
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    dur.as_micros() as i64
}

async fn push_metrics(addr: &str, items: &mut Vec<AttemptMetric>) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let mut raw = Vec::new();
    for m in items.iter() {
        let line = format!("{},{},{},{}\n", m.ts_micros, m.target, m.success, m.latency_us);
        raw.extend_from_slice(line.as_bytes());
    }
    items.clear();

    let mut client = FlightServiceClient::connect(format!("http://{}", addr)).await?;
    let chunk = FlightData {
        data_header: vec![].into(),
        data_body: raw.into(),
        app_metadata: vec![].into(),
        flight_descriptor: None,
    };

    let data_stream = futures::stream::iter(vec![chunk]);
    let response = client.do_put(Request::new(data_stream)).await?;
    let mut resp_stream = response.into_inner();
    while let Some(_msg) = resp_stream.message().await? {
        // ack
    }
    Ok(())
}