use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Target {
    pub addr: String,
    pub port: u16,
    pub weight: f32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoadConfig {
    // We're actually using `rps`, so no need for #[allow(dead_code)]
    pub rps: usize,
    pub duration_seconds: usize,
    pub concurrency: usize,
    pub payload: String,
    pub targets: Vec<Target>,
    pub arrow_output: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CommanderConfig {
    pub edges: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EdgeConfig {
    pub commander_address: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub mode: String, // "commander", "edge", or "standalone"
    pub commander: Option<CommanderConfig>,
    pub edge: Option<EdgeConfig>,
    pub load: LoadConfig,
}

pub fn load_config(path: &str) -> anyhow::Result<AppConfig> {
    let txt = fs::read_to_string(path)?;
    let cfg: AppConfig = toml::from_str(&txt)?;
    Ok(cfg)
}
