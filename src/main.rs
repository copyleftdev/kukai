mod config;
mod commander;
mod edge;
mod standalone;
mod token_bucket;

use anyhow::Result;
use clap::Parser;
use config::load_config;
use std::path::{Path, PathBuf};
use tokio::runtime::Runtime;
use tracing::info;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, Parser)]
#[command(version = "0.2.2", author = "CopyleftDev")]
struct Cli {
    #[arg(long, short = 'c', help = "Path to config TOML")]
    config: Option<PathBuf>,
    #[arg(long, help = "Override mode: commander|edge|standalone")]
    mode: Option<String>,
    #[arg(long, help = "Override RPS")]
    rps: Option<usize>,
    #[arg(long, help = "Override concurrency")]
    concurrency: Option<usize>,
    #[arg(long, help = "Override reuse_connection")]
    reuse_connection: Option<bool>,
}

fn main() -> Result<()> {
    // Set up tracing with an environment filter.
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Unable to set global default tracing subscriber");

    let cli = Cli::parse();
    let rt = Runtime::new()?;
    rt.block_on(async {
        // Use the provided config path or default to "kukai_config.toml".
        let path = cli.config.unwrap_or_else(|| PathBuf::from("kukai_config.toml"));
        // Call load_config with both the file name and a trusted base directory.
        let mut cfg = load_config(path.to_str().unwrap(), Path::new("./config"))?;
        
        // Apply CLI overrides.
        if let Some(m) = cli.mode {
            cfg.mode = m;
        }
        if let Some(r) = cli.rps {
            cfg.load.rps = r;
        }
        if let Some(c) = cli.concurrency {
            cfg.load.concurrency = c;
        }
        if let Some(rc) = cli.reuse_connection {
            cfg.load.reuse_connection = Some(rc);
        }
        info!("KūKai starting in mode = {}", cfg.mode);
        match cfg.mode.as_str() {
            "commander" => {
                if let Some(cc) = &cfg.commander {
                    commander::run_commander(&cfg.load, cc).await?;
                } else {
                    eprintln!("Commander mode requires [commander] in config.");
                }
            }
            "edge" => {
                // Since cfg.edge is a concrete EdgeConfig (with a default), clone it to pass to run_edge.
                edge::run_edge(&cfg.load, &cfg.edge).await?;
            }
            "standalone" => {
                standalone::run_standalone().await?;
            }
            _ => {
                eprintln!("Unknown mode: {}", cfg.mode);
            }
        }
        Ok(())
    })
}
