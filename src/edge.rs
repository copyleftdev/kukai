use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::{Arc, Mutex};
use anyhow::Result;
use tokio::time;
use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tonic::Request;
use arrow_flight::flight_service_client::FlightServiceClient;
use arrow_flight::FlightData;
use crate::config::{LoadConfig, EdgeConfig, Target};

#[derive(Clone)]
struct AttemptMetric {
    ts_micros: i64,
    target: String,
    success: bool,
    latency_us: u64,
}

pub async fn run_edge(load: &LoadConfig, edge: &EdgeConfig) -> Result<()> {
    println!("Edge mode -> Commander: {}", edge.commander_address);
    let metrics = Arc::new(Mutex::new(Vec::new()));
    let end_time = Instant::now() + Duration::from_secs(load.duration_seconds as u64);

    let mut tasks = vec![];
    for _ in 0..load.concurrency {
        let mref = metrics.clone();
        let tlist = load.targets.clone();
        let pay = load.payload.clone();
        tasks.push(tokio::spawn(async move {
            run_worker(mref, tlist, pay, end_time).await;
        }));
    }

    let flush_ref = metrics.clone();
    let commander_addr = edge.commander_address.clone();
    let flusher = tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(2)).await;
            let mut local = {
                let mut g = flush_ref.lock().unwrap();
                std::mem::take(&mut *g)
            };
            if !local.is_empty() {
                let _ = push_metrics(&commander_addr, &mut local).await;
            }
        }
    });

    for t in tasks {
        let _ = t.await;
    }

    {
        let mut leftover = {
            let mut g = metrics.lock().unwrap();
            std::mem::take(&mut *g)
        };
        if !leftover.is_empty() {
            push_metrics(&edge.commander_address, &mut leftover).await?;
        }
    }

    flusher.abort();
    println!("Edge mode complete.");
    Ok(())
}

async fn run_worker(
    metrics: Arc<Mutex<Vec<AttemptMetric>>>,
    targets: Vec<Target>,
    payload: String,
    end: Instant,
) {
    let mut rng = StdRng::from_entropy();
    while Instant::now() < end {
        let t = pick_target(&targets, &mut rng);
        let addr = format!("{}:{}", t.addr, t.port);
        let st = Instant::now();
        let success = match tokio::net::TcpStream::connect(&addr).await {
            Ok(mut stream) => {
                use tokio::io::AsyncWriteExt;
                stream.write_all(payload.as_bytes()).await.is_ok()
            }
            Err(_) => false,
        };
        let latency_us = st.elapsed().as_micros() as u64;
        {
            let mut g = metrics.lock().unwrap();
            g.push(AttemptMetric {
                ts_micros: now_micros(),
                target: addr,
                success,
                latency_us,
            });
        }
        time::sleep(Duration::from_millis(5)).await;
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
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    dur.as_micros() as i64
}

async fn push_metrics(addr: &str, items: &mut Vec<AttemptMetric>) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    // Here, we'll just produce some raw bytes (like CSV).
    let mut raw = Vec::new();
    for m in items.iter() {
        let line = format!("{},{},{},{}\n", m.ts_micros, m.target, m.success, m.latency_us);
        raw.extend_from_slice(line.as_bytes());
    }
    items.clear();

    let mut client = FlightServiceClient::connect(format!("http://{}", addr)).await?;
    let resp = client
        .do_put(Request::new(futures::stream::empty()))
        .await?;
    let (mut tx, mut rx) = resp.into_parts();
    let chunk = FlightData {
        data_header: Vec::new().into(),
        data_body: raw.into(),
        app_metadata: Vec::new().into(),
        flight_descriptor: None,
    };
    tx.send(chunk).await?;
    tx.close().await?;
    while let Some(_msg) = rx.message().await? {}
    Ok(())
}
