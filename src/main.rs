mod config;
mod commander;
mod edge;
mod standalone;

use anyhow::Result;
use clap::Parser;
use config::load_config;
use std::path::PathBuf;
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
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Unable to set global default tracing subscriber");

    let cli = Cli::parse();

    let rt = Runtime::new()?;
    rt.block_on(async {
        let path = cli.config.unwrap_or_else(|| PathBuf::from("kukai_config.toml"));
        let mut cfg = load_config(path.to_str().unwrap())?;

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
                if let Some(ec) = &cfg.edge {
                    edge::run_edge(&cfg.load, ec).await?;
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
