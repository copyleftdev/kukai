use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;
use crate::config::{LoadConfig, EdgeConfig, Target};

pub async fn run_edge(load: &LoadConfig, edge: &EdgeConfig) -> Result<()> {
    let duration = load.duration_seconds.unwrap_or(30) as u64;
    let targets: &[Target] = load.targets.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    let payload: &str = load.payload.as_deref().unwrap_or("");
    println!("Edge server starting. Duration: {} seconds", duration);
    println!("Payload: {}", payload);
    if let Some(commander_addr) = &edge.commander_address {
        println!("Commander address: {:?}", commander_addr);
    }
    let total_weight: f32 = compute_total_weight(targets);
    println!("Total weight: {}", total_weight);
    for target in targets {
        println!("Target address: {}", target.address);
        match target.port {
            Some(p) => println!("Target port: {}", p),
            None => println!("Target port not specified"),
        }
    }
    let mut value: f32 = 100.0;
    if let Some(sub) = Some(5.0_f64) {
        value -= sub as f32;
    }
    println!("Computed value: {}", value);
    sleep(Duration::from_secs(1)).await;
    Ok(())
}

pub fn compute_total_weight(targets: &[Target]) -> f32 {
    targets.iter().map(|t| t.weight.unwrap_or(0.0) as f32).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Target;

    #[test]
    fn test_compute_total_weight() {
        let targets = vec![
            Target {
                address: "http://example.com".to_string(),
                port: Some(80),
                weight: Some(1.5),
            },
            Target {
                address: "http://example.org".to_string(),
                port: Some(8080),
                weight: None,
            },
            Target {
                address: "http://example.net".to_string(),
                port: None,
                weight: Some(2.0),
            },
        ];
        let total = compute_total_weight(&targets);
        assert!((total - 3.5).abs() < 1e-6);
    }
}
