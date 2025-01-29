mod config;
mod commander;
mod edge;
mod standalone;

use anyhow::Result;
use config::load_config;
use std::env;
use tokio::runtime::Runtime;

fn main() -> Result<()> {
    let rt = Runtime::new()?;
    rt.block_on(async {
        let args: Vec<String> = env::args().collect();
        if args.len() < 3 || args[1] != "--config" {
            eprintln!("Usage: {} --config <PATH>", args[0]);
            std::process::exit(1);
        }
        let cfg = load_config(&args[2])?;
        match cfg.mode.as_str() {
            "commander" => {
                if let Some(c) = &cfg.commander {
                    commander::run_commander(&cfg.load, c).await?;
                } else {
                    eprintln!("Commander mode requires [commander] in config.");
                }
            }
            "edge" => {
                if let Some(e) = &cfg.edge {
                    edge::run_edge(&cfg.load, e).await?;
                } else {
                    eprintln!("Edge mode requires [edge] in config.");
                }
            }
            "standalone" => {
                standalone::run_standalone(&cfg.load).await?;
            }
            _ => {
                eprintln!("Unknown mode: {}", cfg.mode);
            }
        }
        Ok(())
    })
}
