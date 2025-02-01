use serde::Deserialize;
use anyhow::Result;

#[derive(Deserialize, Debug, Clone)]
pub struct Target {
    pub address: String,
    pub port: Option<u16>,
    pub weight: Option<f64>,
}

#[derive(Deserialize, Debug)]
pub struct LoadConfig {
    pub rps: usize,
    pub concurrency: usize,
    pub reuse_connection: Option<bool>,
    pub duration_seconds: Option<usize>,
    pub targets: Option<Vec<Target>>,
    pub payload: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct CommanderConfig {
    pub edges: Option<Vec<String>>,
    pub commander_address: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct EdgeConfig {
    pub commander_address: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub mode: String,
    pub load: LoadConfig,
    pub commander: Option<CommanderConfig>,
    pub edge: Option<EdgeConfig>,
    #[allow(dead_code)]
    pub arrow_output: Option<String>,
}

pub fn load_config(path: &str) -> Result<Config> {
    let canonical = std::fs::canonicalize(path)?;
    let content = std::fs::read_to_string(&canonical)?;
    let cfg: Config = toml::from_str(&content)?;
    Ok(cfg)
}
