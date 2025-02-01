use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Minimal target definition for use in load tests.
#[derive(Deserialize, Debug)]
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
    #[allow(dead_code)]
    pub address: Option<String>,
    // Added field to satisfy commander.rs expectations.
    #[serde(default)]
    pub edges: Vec<String>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct EdgeConfig {
    #[serde(default)]
    pub commander_address: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub mode: String,
    pub load: LoadConfig,
    pub commander: Option<CommanderConfig>,
    // Change edge to a concrete type (with a default) so main.rs sees an EdgeConfig.
    #[serde(default)]
    pub edge: EdgeConfig,
}

/// Returns true if `path` is a simple file name (i.e. it has exactly one component)
fn is_simple_file_name<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().components().count() == 1
}

/// Loads a configuration file from a trusted base directory.
/// (The file name is first checked to be simple to avoid path traversal.)
pub fn load_config<P: AsRef<Path>>(file_name: P, base: &Path) -> Result<Config> {
    if !is_simple_file_name(&file_name) {
        return Err(anyhow!("Unsafe file name provided: {:?}", file_name.as_ref()));
    }
    // safe join: file_name is sanitized
    let config_path: PathBuf = base.join(file_name);
    let config_path = config_path.canonicalize()?;
    if !config_path.starts_with(base) {
        return Err(anyhow!("Path traversal attempt detected: {:?}", config_path));
    }
    let content = fs::read_to_string(&config_path)?;
    let cfg: Config = toml::from_str(&content)?;
    Ok(cfg)
}
