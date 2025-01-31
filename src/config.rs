use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::io;

#[derive(Debug, Deserialize, Clone)]
pub struct Target {
    pub addr: String,
    pub port: u16,
    pub weight: f32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoadConfig {
    pub rps: usize,
    pub duration_seconds: usize,
    pub concurrency: usize,
    pub payload: String,
    pub targets: Vec<Target>,
    pub arrow_output: String,
    pub reuse_connection: Option<bool>,
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
    pub mode: String,
    pub commander: Option<CommanderConfig>,
    pub edge: Option<EdgeConfig>,
    pub load: LoadConfig,
}

fn is_safe_path(path: &Path) -> io::Result<PathBuf> {
    // Get the current directory
    let current_dir = std::env::current_dir()?;
    
    // Get absolute paths
    let absolute_current = current_dir.canonicalize()?;
    let absolute_path = path.canonicalize()?;
    
    // Convert to string representation for comparison
    let current_str = absolute_current.to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in current path"))?;
    let path_str = absolute_path.to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in target path"))?;
    
    // Check if the path is within or equal to the current directory
    if path_str.starts_with(current_str) {
        Ok(absolute_path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Path is outside of allowed directory"
        ))
    }
}

pub fn load_config(path: &str) -> anyhow::Result<AppConfig> {
    let path = Path::new(path);
    
    // Verify path is safe before proceeding
    let canonical_path = is_safe_path(path)
        .map_err(|e| anyhow::anyhow!("Invalid or unsafe path: {}", e))?;
        
    // Read and parse config
    let contents = std::fs::read_to_string(canonical_path)?;
    let cfg: AppConfig = toml::from_str(&contents)?;
    Ok(cfg)
}