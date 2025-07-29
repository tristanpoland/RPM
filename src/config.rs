use crate::{process::ManagedProcess, Result, RpmError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub daemon_port: u16,
    pub max_processes: usize,
    pub log_max_size: u64,
    pub log_retention_days: u32,
    pub auto_restart_delay: u64,
    pub health_check_interval: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            daemon_port: 9999,
            max_processes: 1000,
            log_max_size: 100 * 1024 * 1024, // 100MB
            log_retention_days: 30,
            auto_restart_delay: 5,
            health_check_interval: 5,
        }
    }
}

impl Config {
    pub async fn load() -> Result<Self> {
        let config_path = get_config_path()?;
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await.map_err(|e| {
                RpmError::Config(format!("Failed to read config file: {}", e))
            })?;
            
            serde_json::from_str(&content).map_err(|e| {
                RpmError::Config(format!("Failed to parse config file: {}", e))
            })
        } else {
            let config = Config::default();
            config.save().await?;
            Ok(config)
        }
    }

    pub async fn save(&self) -> Result<()> {
        let config_path = get_config_path()?;
        
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                RpmError::Config(format!("Failed to create config directory: {}", e))
            })?;
        }

        let content = serde_json::to_string_pretty(self).map_err(|e| {
            RpmError::Config(format!("Failed to serialize config: {}", e))
        })?;

        fs::write(&config_path, content).await.map_err(|e| {
            RpmError::Config(format!("Failed to write config file: {}", e))
        })
    }

    pub async fn save_processes(&self, processes: &HashMap<String, ManagedProcess>) -> Result<()> {
        let processes_path = get_processes_path()?;
        
        if let Some(parent) = processes_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                RpmError::Config(format!("Failed to create processes directory: {}", e))
            })?;
        }

        let process_configs: Vec<_> = processes
            .values()
            .map(|p| &p.info.config)
            .collect();

        let content = serde_json::to_string_pretty(&process_configs).map_err(|e| {
            RpmError::Config(format!("Failed to serialize processes: {}", e))
        })?;

        fs::write(&processes_path, content).await.map_err(|e| {
            RpmError::Config(format!("Failed to write processes file: {}", e))
        })
    }

    pub async fn load_processes(&self) -> Result<HashMap<String, ManagedProcess>> {
        let processes_path = get_processes_path()?;
        
        if !processes_path.exists() {
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(&processes_path).await.map_err(|e| {
            RpmError::Config(format!("Failed to read processes file: {}", e))
        })?;

        let process_configs: Vec<crate::cli::ProcessConfig> = serde_json::from_str(&content)
            .map_err(|e| {
                RpmError::Config(format!("Failed to parse processes file: {}", e))
            })?;

        let mut processes = HashMap::new();
        for config in process_configs {
            let process = ManagedProcess::new(config.clone());
            processes.insert(config.name.clone(), process);
        }

        Ok(processes)
    }
}

fn get_config_path() -> Result<PathBuf> {
    let project_dirs = directories::ProjectDirs::from("", "", "rpm")
        .ok_or_else(|| RpmError::Config("Failed to get project directories".to_string()))?;
    
    Ok(project_dirs.config_dir().join("config.json"))
}

fn get_processes_path() -> Result<PathBuf> {
    let project_dirs = directories::ProjectDirs::from("", "", "rpm")
        .ok_or_else(|| RpmError::Config("Failed to get project directories".to_string()))?;
    
    Ok(project_dirs.data_dir().join("processes.json"))
}

pub fn get_logs_dir() -> Result<PathBuf> {
    let project_dirs = directories::ProjectDirs::from("", "", "rpm")
        .ok_or_else(|| RpmError::Config("Failed to get project directories".to_string()))?;
    
    let logs_dir = project_dirs.data_dir().join("logs");
    std::fs::create_dir_all(&logs_dir).map_err(|e| {
        RpmError::Config(format!("Failed to create logs directory: {}", e))
    })?;
    
    Ok(logs_dir)
}

pub fn get_pids_dir() -> Result<PathBuf> {
    let project_dirs = directories::ProjectDirs::from("", "", "rpm")
        .ok_or_else(|| RpmError::Config("Failed to get project directories".to_string()))?;
    
    let pids_dir = project_dirs.runtime_dir()
        .unwrap_or_else(|| project_dirs.data_dir())
        .join("pids");
    
    std::fs::create_dir_all(&pids_dir).map_err(|e| {
        RpmError::Config(format!("Failed to create pids directory: {}", e))
    })?;
    
    Ok(pids_dir)
}