use tokio;

#[tokio::main]
async fn main() -> rpm::Result<()> {
    tracing_subscriber::fmt::init();
    
    #[cfg(windows)]
    {
        if std::env::args().any(|arg| arg == "--service") {
            return rpm::daemon::run_windows_service()
                .map_err(|e| rpm::RpmError::Daemon(format!("Service error: {}", e)));
        }
    }

    rpm::daemon::start_daemon(true).await
}