use thiserror::Error;

pub type Result<T> = std::result::Result<T, RpmError>;

#[derive(Error, Debug)]
pub enum RpmError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    
    #[error("Process error: {0}")]
    Process(String),
    
    #[error("Daemon error: {0}")]
    Daemon(String),
    
    #[error("IPC error: {0}")]
    Ipc(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Process not found: {0}")]
    ProcessNotFound(String),
}