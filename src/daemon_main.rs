use clap::Parser;
use tokio;

#[derive(Parser, Debug)]
#[command(name = "rpm-daemon")]
#[command(about = "RPM Process Manager Daemon")]
struct Args {
    #[arg(long, short = 'f', help = "Run in foreground mode")]
    foreground: bool,
    
    #[arg(long, help = "Run as system service (internal use)")]
    service: bool,
    
    #[arg(long, help = "Install and start as system service")]
    install: bool,
}

#[tokio::main]
async fn main() -> rpm::Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt::init();
    
    if args.service {
        #[cfg(windows)]
        {
            return rpm::daemon::run_windows_service()
                .map_err(|e| rpm::RpmError::Daemon(format!("Service error: {}", e)));
        }
        #[cfg(unix)]
        {
            return rpm::daemon::start_daemon(true).await;
        }
        #[cfg(not(any(windows, unix)))]
        {
            return Err(rpm::RpmError::Daemon(
                "Service mode is not supported on this platform".to_string()
            ));
        }
    }
    
    if args.install {
        return rpm::daemon::start_daemon(false).await;
    }

    if args.foreground {
        return rpm::daemon::start_daemon(true).await;
    }

    #[cfg(any(windows, unix))]
    {
        println!("Starting RPM daemon as background service...");
        println!("Use --foreground or -f to run in foreground mode");
        rpm::daemon::start_daemon(false).await
    }
    
    #[cfg(not(any(windows, unix)))]
    {
        println!("Background service mode is not supported on this platform.");
        println!("Running in foreground mode...");
        rpm::daemon::start_daemon(true).await
    }
}