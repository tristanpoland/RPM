use clap::Parser;
use rpm::{cli::*, Result};
use std::process;
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Start {
            command,
            name,
            cwd,
            instances,
            autorestart,
            max_memory,
            env,
        } => {
            let config = ProcessConfig::from_args(command, name, cwd, instances, autorestart, max_memory, env)?;
            handle_start(config).await
        }
        Commands::Stop { name } => handle_stop(name).await,
        Commands::Restart { name } => handle_restart(name).await,
        Commands::Delete { name } => handle_delete(name).await,
        Commands::List => handle_list().await,
        Commands::Logs { name, lines, follow } => handle_logs(name, lines, follow).await,
        Commands::Show { name } => handle_show(name).await,
        Commands::Monitor => handle_monitor().await,
        Commands::Daemon { foreground } => {
            #[cfg(windows)]
            {
                if !foreground {
                    println!("Installing RPM as Windows service requires administrator privileges.");
                    println!("Please run this command as Administrator, or use --foreground flag.");
                    println!();
                }
            }
            handle_daemon(foreground).await
        },
        Commands::Kill => handle_kill().await,
        Commands::Reload { name } => handle_reload(name).await,
        Commands::Save => handle_save().await,
        Commands::Resurrect => handle_resurrect().await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    Ok(())
}

async fn handle_start(config: ProcessConfig) -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    client.start_process(config).await?;
    println!("Process started successfully");
    Ok(())
}

async fn handle_stop(name: String) -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    client.stop_process(&name).await?;
    println!("Process '{}' stopped", name);
    Ok(())
}

async fn handle_restart(name: String) -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    client.restart_process(&name).await?;
    println!("Process '{}' restarted", name);
    Ok(())
}

async fn handle_delete(name: String) -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    client.delete_process(&name).await?;
    println!("Process '{}' deleted", name);
    Ok(())
}

async fn handle_list() -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    let processes = client.list_processes().await?;
    
    if processes.is_empty() {
        println!("No processes running");
        return Ok(());
    }

    println!("{:<15} {:<10} {:<15} {:<10} {:<20}", "NAME", "ID", "STATUS", "CPU", "MEMORY");
    println!("{}", "-".repeat(80));
    
    for process in processes {
        println!(
            "{:<15} {:<10} {:<15} {:<10} {:<20}",
            process.name,
            process.id,
            process.status,
            format!("{}%", process.cpu_usage),
            format!("{}MB", process.memory_usage / 1024 / 1024)
        );
    }
    
    Ok(())
}

async fn handle_logs(name: String, lines: usize, follow: bool) -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    let logs = client.get_logs(&name, lines, follow).await?;
    
    for log in logs {
        println!("{}", log);
    }
    
    Ok(())
}

async fn handle_show(name: String) -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    let info = client.get_process_info(&name).await?;
    
    println!("Process Information:");
    println!("Name: {}", info.name);
    println!("ID: {}", info.id);
    println!("Status: {}", info.status);
    println!("CPU: {}%", info.cpu_usage);
    println!("Memory: {}MB", info.memory_usage / 1024 / 1024);
    println!("Command: {}", info.command);
    println!("Started: {}", info.started_at);
    
    Ok(())
}

async fn handle_monitor() -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    client.monitor().await?;
    Ok(())
}

async fn handle_daemon(foreground: bool) -> Result<()> {
    rpm::daemon::start_daemon(foreground).await?;
    Ok(())
}

async fn handle_kill() -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    client.kill_daemon().await?;
    println!("Daemon stopped");
    Ok(())
}

async fn handle_reload(name: String) -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    client.reload_process(&name).await?;
    println!("Process '{}' reloaded", name);
    Ok(())
}

async fn handle_save() -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    client.save_processes().await?;
    println!("Process list saved");
    Ok(())
}

async fn handle_resurrect() -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    client.resurrect_processes().await?;
    println!("Processes resurrected");
    Ok(())
}