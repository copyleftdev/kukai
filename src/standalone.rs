use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration, SystemTime, UNIX_EPOCH};
use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::time;
use arrow::array::{
    TimestampMicrosecondBuilder, StringBuilder, BooleanBuilder, UInt64Builder,
};
use arrow::datatypes::{Schema, Field, DataType, TimeUnit};
use arrow::record_batch::RecordBatch;
use arrow_ipc::writer::FileWriter;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom};

use crate::config::{LoadConfig, Target};

#[derive(Clone)]
struct AttemptMetric {
    ts_micros: i64,
    target: String,
    success: bool,
    latency_us: u64,
}

pub async fn run_standalone(load: &LoadConfig) -> Result<()> {
    println!("Standalone mode -> writing Arrow to {}", load.arrow_output);
    let metrics = Arc::new(Mutex::new(Vec::new()));
    let end_time = Instant::now() + Duration::from_secs(load.duration_seconds as u64);

    let mut workers = Vec::new();
    for _ in 0..load.concurrency {
        let mref = metrics.clone();
        let tlist = load.targets.clone();
        let pay = load.payload.clone();
        workers.push(tokio::spawn(async move {
            run_worker(mref, tlist, pay, end_time).await;
        }));
    }

    let flush_mref = metrics.clone();
    let path = load.arrow_output.clone();
    let flusher = tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_secs(2)).await;
            let mut local = {
                let mut g = flush_mref.lock().unwrap();
                std::mem::take(&mut *g)
            };
            if !local.is_empty() {
                let _ = append_to_arrow(&path, &mut local);
            }
        }
    });

    for w in workers {
        let _ = w.await;
    }

    {
        let mut leftover = {
            let mut g = metrics.lock().unwrap();
            std::mem::take(&mut *g)
        };
        if !leftover.is_empty() {
            append_to_arrow(&load.arrow_output, &mut leftover)?;
        }
    }

    flusher.abort();
    println!("Standalone mode complete.");
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
            Ok(mut s) => {
                use tokio::io::AsyncWriteExt;
                s.write_all(payload.as_bytes()).await.is_ok()
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
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    d.as_micros() as i64
}

fn append_to_arrow(path: &str, items: &mut Vec<AttemptMetric>) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let schema = Schema::new(vec![
        Field::new("timestamp_micros", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("target", DataType::Utf8, false),
        Field::new("success", DataType::Boolean, false),
        Field::new("latency_us", DataType::UInt64, false),
    ]);

    let mut tsb = TimestampMicrosecondBuilder::new();
    let mut sb = StringBuilder::new();
    let mut bb = BooleanBuilder::new();
    let mut ub = UInt64Builder::new();

    for m in items.iter() {
        tsb.append_value(m.ts_micros)?;
        sb.append_value(&m.target)?;
        bb.append_value(m.success)?;
        ub.append_value(m.latency_us)?;
    }

    let ts_arr = tsb.finish();
    let s_arr = sb.finish();
    let b_arr = bb.finish();
    let u_arr = ub.finish();

    let batch = RecordBatch::try_new(
        std::sync::Arc::new(schema.clone()),
        vec![
            std::sync::Arc::new(ts_arr),
            std::sync::Arc::new(s_arr),
            std::sync::Arc::new(b_arr),
            std::sync::Arc::new(u_arr),
        ],
    )?;
    items.clear();

    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .write(true)
        .open(path)?;
    file.seek(SeekFrom::End(0))?;
    let mut writer = FileWriter::try_new(file, &schema)?;
    writer.write(&batch)?;
    writer.finish()?;
    Ok(())
}
