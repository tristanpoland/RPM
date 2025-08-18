use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "rpm")]
#[command(about = "A process manager like PM2 written in Rust")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Start a new process")]
    Start {
        #[arg(help = "Command to execute")]
        command: String,
        #[arg(short, long, help = "Process name")]
        name: Option<String>,
        #[arg(short, long, help = "Working directory")]
        cwd: Option<String>,
        #[arg(short = 'i', long, help = "Number of instances to start", default_value = "1")]
        instances: u32,
        #[arg(long, help = "Auto restart on failure", default_value = "true")]
        autorestart: bool,
        #[arg(long, help = "Max memory usage (MB)")]
        max_memory: Option<u64>,
        #[arg(long, help = "Environment variables (key=value)")]
        env: Vec<String>,
    },
    #[command(about = "Stop a process")]
    Stop {
        #[arg(help = "Process name or ID")]
        name: String,
    },
    #[command(about = "Restart a process")]
    Restart {
        #[arg(help = "Process name or ID")]
        name: String,
    },
    #[command(about = "Delete a process")]
    Delete {
        #[arg(help = "Process name or ID")]
        name: String,
    },
    #[command(about = "List all processes")]
    List,
    #[command(about = "Show process logs")]
    Logs {
        #[arg(help = "Process name or ID")]
        name: String,
        #[arg(short, long, help = "Number of lines to show", default_value = "20")]
        lines: usize,
        #[arg(short, long, help = "Follow log output")]
        follow: bool,
    },
    #[command(about = "Show detailed process information")]
    Show {
        #[arg(help = "Process name or ID")]
        name: String,
    },
    #[command(about = "Monitor processes in real-time")]
    Monitor,
    #[command(about = "Start the daemon")]
    Daemon {
        #[arg(long, help = "Run daemon in foreground")]
        foreground: bool,
    },
    #[command(about = "Stop the daemon")]
    Kill,
    #[command(about = "Reload process configuration")]
    Reload {
        #[arg(help = "Process name or ID")]
        name: String,
    },
    #[command(about = "Save current process list")]
    Save,
    #[command(about = "Resurrect saved processes")]
    Resurrect,
    #[command(about = "Show daemon status")]
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    pub name: String,
    pub command: String,
    pub cwd: Option<String>,
    pub instances: u32,
    pub autorestart: bool,
    pub max_memory: Option<u64>,
    pub env: Vec<(String, String)>,
}

impl ProcessConfig {
    pub fn from_args(
        command: String,
        name: Option<String>,
        cwd: Option<String>,
        instances: u32,
        autorestart: bool,
        max_memory: Option<u64>,
        env: Vec<String>,
    ) -> crate::Result<Self> {
        let name = name.unwrap_or_else(|| {
            command
                .split_whitespace()
                .next()
                .unwrap_or("unknown")
                .to_string()
        });

        let env_vars: Result<Vec<(String, String)>, _> = env
            .into_iter()
            .map(|e| {
                let parts: Vec<&str> = e.splitn(2, '=').collect();
                if parts.len() == 2 {
                    Ok((parts[0].to_string(), parts[1].to_string()))
                } else {
                    Err(crate::RpmError::Config(format!("Invalid env format: {}", e)))
                }
            })
            .collect();

        Ok(ProcessConfig {
            name,
            command,
            cwd,
            instances,
            autorestart,
            max_memory,
            env: env_vars?,
        })
    }
}