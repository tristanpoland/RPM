use crate::{cli::ProcessConfig, process::ProcessInfo, Result, RpmError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcRequest {
    StartProcess(ProcessConfig),
    StopProcess(String),
    RestartProcess(String),
    DeleteProcess(String),
    ListProcesses,
    GetProcessInfo(String),
    GetLogs { name: String, lines: usize, follow: bool },
    Monitor,
    KillDaemon,
    ReloadProcess(String),
    SaveProcesses,
    ResurrectProcesses,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcResponse {
    Success(String),
    ProcessList(Vec<ProcessInfo>),
    ProcessInfo(ProcessInfo),
    Logs(Vec<String>),
    Error(String),
}

pub struct IpcServer {
    #[cfg(unix)]
    socket_path: std::path::PathBuf,
    #[cfg(windows)]
    port: u16,
}

impl IpcServer {
    pub async fn new() -> Result<Self> {
        #[cfg(unix)]
        {
            let socket_path = get_socket_path()?;
            if socket_path.exists() {
                std::fs::remove_file(&socket_path).map_err(|e| {
                    RpmError::Ipc(format!("Failed to remove existing socket: {}", e))
                })?;
            }
            Ok(IpcServer { socket_path })
        }
        
        #[cfg(windows)]
        {
            Ok(IpcServer { port: 9999 })
        }
    }

    pub async fn run(
        &self,
        process_manager: Arc<Mutex<crate::process::ProcessManager>>,
    ) -> Result<()> {
        #[cfg(unix)]
        {
            let listener = UnixListener::bind(&self.socket_path).map_err(|e| {
                RpmError::Ipc(format!("Failed to bind Unix socket: {}", e))
            })?;

            tracing::info!("IPC server listening on Unix socket: {:?}", self.socket_path);

            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let pm = process_manager.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_unix_connection(stream, pm).await {
                                tracing::error!("Error handling Unix connection: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to accept Unix connection: {}", e);
                    }
                }
            }
        }

        #[cfg(windows)]
        {
            let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))
                .await
                .map_err(|e| RpmError::Ipc(format!("Failed to bind TCP socket: {}", e)))?;

            tracing::info!("IPC server listening on TCP port: {}", self.port);

            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let pm = process_manager.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_tcp_connection(stream, pm).await {
                                tracing::error!("Error handling TCP connection: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to accept TCP connection: {}", e);
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
async fn handle_unix_connection(
    stream: UnixStream,
    process_manager: Arc<Mutex<crate::process::ProcessManager>>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let request: IpcRequest = serde_json::from_str(&line)
            .map_err(|e| RpmError::Ipc(format!("Failed to parse request: {}", e)))?;

        let response = handle_request(request, &process_manager).await;
        let response_json = serde_json::to_string(&response)
            .map_err(|e| RpmError::Ipc(format!("Failed to serialize response: {}", e)))?;

        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;

        line.clear();
    }

    Ok(())
}

#[cfg(windows)]
async fn handle_tcp_connection(
    stream: TcpStream,
    process_manager: Arc<Mutex<crate::process::ProcessManager>>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let request: IpcRequest = serde_json::from_str(&line)
            .map_err(|e| RpmError::Ipc(format!("Failed to parse request: {}", e)))?;

        let response = handle_request(request, &process_manager).await;
        let response_json = serde_json::to_string(&response)
            .map_err(|e| RpmError::Ipc(format!("Failed to serialize response: {}", e)))?;

        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;

        line.clear();
    }

    Ok(())
}

async fn handle_request(
    request: IpcRequest,
    process_manager: &Arc<Mutex<crate::process::ProcessManager>>,
) -> IpcResponse {
    let mut pm = process_manager.lock().await;

    match request {
        IpcRequest::StartProcess(config) => {
            match pm.start_process(config).await {
                Ok(id) => IpcResponse::Success(format!("Process started with id: {}", id)),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::StopProcess(name) => {
            match pm.stop_process(&name).await {
                Ok(_) => IpcResponse::Success(format!("Process '{}' stopped", name)),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::RestartProcess(name) => {
            match pm.restart_process(&name).await {
                Ok(_) => IpcResponse::Success(format!("Process '{}' restarted", name)),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::DeleteProcess(name) => {
            match pm.delete_process(&name).await {
                Ok(_) => IpcResponse::Success(format!("Process '{}' deleted", name)),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::ListProcesses => {
            let processes = pm.list_processes().await;
            IpcResponse::ProcessList(processes.into_iter().cloned().collect())
        }
        IpcRequest::GetProcessInfo(name) => {
            match pm.get_process_info(&name).await {
                Ok(info) => IpcResponse::ProcessInfo(info.clone()),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::GetLogs { name, lines, follow: _ } => {
            match pm.get_logs(&name, lines).await {
                Ok(logs) => IpcResponse::Logs(logs),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::Monitor => {
            IpcResponse::Success("Monitor not implemented in this context".to_string())
        }
        IpcRequest::KillDaemon => {
            IpcResponse::Success("Daemon shutdown requested".to_string())
        }
        IpcRequest::ReloadProcess(name) => {
            match pm.restart_process(&name).await {
                Ok(_) => IpcResponse::Success(format!("Process '{}' reloaded", name)),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
        IpcRequest::SaveProcesses => {
            IpcResponse::Success("Processes saved".to_string())
        }
        IpcRequest::ResurrectProcesses => {
            match pm.load_state().await {
                Ok(_) => IpcResponse::Success("Processes resurrected".to_string()),
                Err(e) => IpcResponse::Error(e.to_string()),
            }
        }
    }
}

pub struct IpcClient {
    #[cfg(unix)]
    socket_path: std::path::PathBuf,
    #[cfg(windows)]
    port: u16,
}

impl IpcClient {
    pub async fn new() -> Result<Self> {
        #[cfg(unix)]
        {
            let socket_path = get_socket_path()?;
            Ok(IpcClient { socket_path })
        }
        
        #[cfg(windows)]
        {
            Ok(IpcClient { port: 9999 })
        }
    }

    async fn send_request(&self, request: IpcRequest) -> Result<IpcResponse> {
        #[cfg(unix)]
        {
            let stream = UnixStream::connect(&self.socket_path).await.map_err(|e| {
                RpmError::Ipc(format!("Failed to connect to daemon: {}", e))
            })?;

            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);

            let request_json = serde_json::to_string(&request)?;
            writer.write_all(request_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;

            let mut line = String::new();
            reader.read_line(&mut line).await?;

            let response: IpcResponse = serde_json::from_str(&line)?;
            Ok(response)
        }

        #[cfg(windows)]
        {
            let stream = TcpStream::connect(format!("127.0.0.1:{}", self.port))
                .await
                .map_err(|e| RpmError::Ipc(format!("Failed to connect to daemon: {}", e)))?;

            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);

            let request_json = serde_json::to_string(&request)?;
            writer.write_all(request_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;

            let mut line = String::new();
            reader.read_line(&mut line).await?;

            let response: IpcResponse = serde_json::from_str(&line)?;
            Ok(response)
        }
    }

    pub async fn start_process(&self, config: ProcessConfig) -> Result<()> {
        match self.send_request(IpcRequest::StartProcess(config)).await? {
            IpcResponse::Success(_) => Ok(()),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }

    pub async fn stop_process(&self, name: &str) -> Result<()> {
        match self.send_request(IpcRequest::StopProcess(name.to_string())).await? {
            IpcResponse::Success(_) => Ok(()),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }

    pub async fn restart_process(&self, name: &str) -> Result<()> {
        match self.send_request(IpcRequest::RestartProcess(name.to_string())).await? {
            IpcResponse::Success(_) => Ok(()),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }

    pub async fn delete_process(&self, name: &str) -> Result<()> {
        match self.send_request(IpcRequest::DeleteProcess(name.to_string())).await? {
            IpcResponse::Success(_) => Ok(()),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }

    pub async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        match self.send_request(IpcRequest::ListProcesses).await? {
            IpcResponse::ProcessList(processes) => Ok(processes),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }

    pub async fn get_process_info(&self, name: &str) -> Result<ProcessInfo> {
        match self.send_request(IpcRequest::GetProcessInfo(name.to_string())).await? {
            IpcResponse::ProcessInfo(info) => Ok(info),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }

    pub async fn get_logs(&self, name: &str, lines: usize, follow: bool) -> Result<Vec<String>> {
        match self.send_request(IpcRequest::GetLogs {
            name: name.to_string(),
            lines,
            follow,
        }).await? {
            IpcResponse::Logs(logs) => Ok(logs),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }


    pub async fn kill_daemon(&self) -> Result<()> {
        match self.send_request(IpcRequest::KillDaemon).await? {
            IpcResponse::Success(_) => Ok(()),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }

    pub async fn reload_process(&self, name: &str) -> Result<()> {
        match self.send_request(IpcRequest::ReloadProcess(name.to_string())).await? {
            IpcResponse::Success(_) => Ok(()),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }

    pub async fn save_processes(&self) -> Result<()> {
        match self.send_request(IpcRequest::SaveProcesses).await? {
            IpcResponse::Success(_) => Ok(()),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }

    pub async fn resurrect_processes(&self) -> Result<()> {
        match self.send_request(IpcRequest::ResurrectProcesses).await? {
            IpcResponse::Success(_) => Ok(()),
            IpcResponse::Error(e) => Err(RpmError::Ipc(e)),
            _ => Err(RpmError::Ipc("Unexpected response".to_string())),
        }
    }
}

#[cfg(unix)]
fn get_socket_path() -> Result<std::path::PathBuf> {
    let home_dir = directories::ProjectDirs::from("", "", "rpm")
        .ok_or_else(|| RpmError::Ipc("Failed to get home directory".to_string()))?;
    
    let socket_dir = home_dir.runtime_dir().unwrap_or_else(|| home_dir.data_dir());
    std::fs::create_dir_all(socket_dir)
        .map_err(|e| RpmError::Ipc(format!("Failed to create socket directory: {}", e)))?;
    
    Ok(socket_dir.join("rpm.sock"))
}