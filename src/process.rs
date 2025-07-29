use crate::{cli::ProcessConfig, Result, RpmError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command as TokioCommand;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub id: String,
    pub name: String,
    pub command: String,
    pub status: ProcessStatus,
    pub pid: Option<u32>,
    pub cpu_usage: f64,
    pub memory_usage: u64,
    pub started_at: DateTime<Utc>,
    pub restarts: u32,
    pub config: ProcessConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessStatus {
    Running,
    Stopped,
    Errored,
    Restarting,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::Running => write!(f, "running"),
            ProcessStatus::Stopped => write!(f, "stopped"),
            ProcessStatus::Errored => write!(f, "errored"),
            ProcessStatus::Restarting => write!(f, "restarting"),
        }
    }
}

pub struct ManagedProcess {
    pub info: ProcessInfo,
    pub child: Option<tokio::process::Child>,
    pub last_restart: Option<Instant>,
    pub log_buffer: Vec<String>,
}

impl ManagedProcess {
    pub fn new(config: ProcessConfig) -> Self {
        let id = Uuid::new_v4().to_string();
        let info = ProcessInfo {
            id: id.clone(),
            name: config.name.clone(),
            command: config.command.clone(),
            status: ProcessStatus::Stopped,
            pid: None,
            cpu_usage: 0.0,
            memory_usage: 0,
            started_at: Utc::now(),
            restarts: 0,
            config,
        };

        ManagedProcess {
            info,
            child: None,
            last_restart: None,
            log_buffer: Vec::new(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.info.status == ProcessStatus::Running {
            return Ok(());
        }

        let mut cmd = TokioCommand::new("sh");
        cmd.arg("-c").arg(&self.info.command);

        #[cfg(windows)]
        {
            cmd = TokioCommand::new("cmd");
            cmd.arg("/C").arg(&self.info.command);
        }

        if let Some(cwd) = &self.info.config.cwd {
            cmd.current_dir(cwd);
        }

        for (key, value) in &self.info.config.env {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        match cmd.spawn() {
            Ok(child) => {
                self.info.pid = child.id();
                self.info.status = ProcessStatus::Running;
                self.info.started_at = Utc::now();
                self.child = Some(child);
                tracing::info!("Started process '{}' with PID {:?}", self.info.name, self.info.pid);
                Ok(())
            }
            Err(e) => {
                self.info.status = ProcessStatus::Errored;
                Err(RpmError::Process(format!("Failed to start process '{}': {}", self.info.name, e)))
            }
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            #[cfg(unix)]
            {
                if let Some(pid) = child.id() {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
            }

            #[cfg(windows)]
            {
                child.kill().await.map_err(|e| {
                    RpmError::Process(format!("Failed to kill process '{}': {}", self.info.name, e))
                })?;
            }

            let _ = child.wait().await;
            self.info.status = ProcessStatus::Stopped;
            self.info.pid = None;
            tracing::info!("Stopped process '{}'", self.info.name);
        }
        Ok(())
    }

    pub async fn restart(&mut self) -> Result<()> {
        self.stop().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        self.info.restarts += 1;
        self.last_restart = Some(Instant::now());
        self.start().await
    }

    pub async fn check_status(&mut self) -> Result<()> {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.info.status = if status.success() {
                        ProcessStatus::Stopped
                    } else {
                        ProcessStatus::Errored
                    };
                    self.info.pid = None;
                    self.child = None;
                    tracing::info!("Process '{}' exited with status: {}", self.info.name, status);
                }
                Ok(None) => {
                    self.update_resource_usage().await?;
                }
                Err(e) => {
                    tracing::error!("Error checking process '{}': {}", self.info.name, e);
                    self.info.status = ProcessStatus::Errored;
                    self.child = None;
                }
            }
        }
        Ok(())
    }

    async fn update_resource_usage(&mut self) -> Result<()> {
        if let Some(pid) = self.info.pid {
            #[cfg(unix)]
            {
                if let Ok(usage) = get_process_usage_unix(pid) {
                    self.info.cpu_usage = usage.0;
                    self.info.memory_usage = usage.1;
                }
            }

            #[cfg(windows)]
            {
                if let Ok(usage) = get_process_usage_windows(pid) {
                    self.info.cpu_usage = usage.0;
                    self.info.memory_usage = usage.1;
                }
            }
        }
        Ok(())
    }

    pub fn should_restart(&self) -> bool {
        if !self.info.config.autorestart {
            return false;
        }

        if self.info.status != ProcessStatus::Errored && self.info.status != ProcessStatus::Stopped {
            return false;
        }

        if let Some(last_restart) = self.last_restart {
            if last_restart.elapsed() < Duration::from_secs(5) {
                return false;
            }
        }

        true
    }
}

pub struct ProcessManager {
    processes: HashMap<String, ManagedProcess>,
    config: crate::config::Config,
}

impl ProcessManager {
    pub async fn new() -> Result<Self> {
        let config = crate::config::Config::load().await?;
        Ok(ProcessManager {
            processes: HashMap::new(),
            config,
        })
    }

    pub async fn start_process(&mut self, config: ProcessConfig) -> Result<String> {
        let mut process = ManagedProcess::new(config);
        process.start().await?;
        let id = process.info.id.clone();
        self.processes.insert(process.info.name.clone(), process);
        self.save_state().await?;
        Ok(id)
    }

    pub async fn stop_process(&mut self, name: &str) -> Result<()> {
        if let Some(process) = self.processes.get_mut(name) {
            process.stop().await?;
            self.save_state().await?;
            Ok(())
        } else {
            Err(RpmError::ProcessNotFound(name.to_string()))
        }
    }

    pub async fn restart_process(&mut self, name: &str) -> Result<()> {
        if let Some(process) = self.processes.get_mut(name) {
            process.restart().await?;
            self.save_state().await?;
            Ok(())
        } else {
            Err(RpmError::ProcessNotFound(name.to_string()))
        }
    }

    pub async fn delete_process(&mut self, name: &str) -> Result<()> {
        if let Some(mut process) = self.processes.remove(name) {
            process.stop().await?;
            self.save_state().await?;
            Ok(())
        } else {
            Err(RpmError::ProcessNotFound(name.to_string()))
        }
    }

    pub async fn list_processes(&self) -> Vec<&ProcessInfo> {
        self.processes.values().map(|p| &p.info).collect()
    }

    pub async fn get_process_info(&self, name: &str) -> Result<&ProcessInfo> {
        self.processes
            .get(name)
            .map(|p| &p.info)
            .ok_or_else(|| RpmError::ProcessNotFound(name.to_string()))
    }

    pub async fn get_logs(&self, name: &str, lines: usize) -> Result<Vec<String>> {
        if let Some(process) = self.processes.get(name) {
            let log_count = process.log_buffer.len();
            let start = if log_count > lines { log_count - lines } else { 0 };
            Ok(process.log_buffer[start..].to_vec())
        } else {
            Err(RpmError::ProcessNotFound(name.to_string()))
        }
    }

    pub async fn monitor_processes(&mut self) -> Result<()> {
        let mut to_restart = Vec::new();

        for (name, process) in &mut self.processes {
            process.check_status().await?;
            
            if process.should_restart() {
                to_restart.push(name.clone());
            }

            if let Some(max_memory) = process.info.config.max_memory {
                let memory_mb = process.info.memory_usage / 1024 / 1024;
                if memory_mb > max_memory {
                    tracing::warn!("Process '{}' exceeded memory limit: {}MB > {}MB", 
                                   name, memory_mb, max_memory);
                    to_restart.push(name.clone());
                }
            }
        }

        for name in to_restart {
            tracing::info!("Auto-restarting process '{}'", name);
            if let Err(e) = self.restart_process(&name).await {
                tracing::error!("Failed to restart process '{}': {}", name, e);
            }
        }

        Ok(())
    }

    async fn save_state(&self) -> Result<()> {
        self.config.save_processes(&self.processes).await
    }

    pub async fn load_state(&mut self) -> Result<()> {
        if let Ok(processes) = self.config.load_processes().await {
            self.processes = processes;
        }
        Ok(())
    }
}

#[cfg(unix)]
fn get_process_usage_unix(pid: u32) -> Result<(f64, u64)> {
    use std::fs;
    
    let stat_path = format!("/proc/{}/stat", pid);
    let statm_path = format!("/proc/{}/statm", pid);
    
    let stat_content = fs::read_to_string(stat_path)
        .map_err(|e| RpmError::Process(format!("Failed to read stat: {}", e)))?;
    let statm_content = fs::read_to_string(statm_path)
        .map_err(|e| RpmError::Process(format!("Failed to read statm: {}", e)))?;
    
    let stat_parts: Vec<&str> = stat_content.split_whitespace().collect();
    let memory_pages: u64 = statm_content.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    
    let cpu_usage = 0.0; // Simplified - would need more complex calculation
    let memory_usage = memory_pages * 4096; // Assuming 4KB pages
    
    Ok((cpu_usage, memory_usage))
}

#[cfg(windows)]
fn get_process_usage_windows(pid: u32) -> Result<(f64, u64)> {
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::psapi::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use winapi::um::winnt::PROCESS_QUERY_INFORMATION;
    use std::mem;
    
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
        if handle.is_null() {
            return Ok((0.0, 0));
        }
        
        let mut mem_counters: PROCESS_MEMORY_COUNTERS = mem::zeroed();
        let size = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        
        if GetProcessMemoryInfo(handle, &mut mem_counters, size) != 0 {
            let memory_usage = mem_counters.WorkingSetSize as u64;
            return Ok((0.0, memory_usage)); // CPU usage simplified
        }
    }
    
    Ok((0.0, 0))
}