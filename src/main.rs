use clap::Parser;
use rpm::{cli::*, ui::*, Result};
use std::process;
use tokio;
use colored::*;

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
        Commands::Status => handle_status().await,
    };

    if let Err(e) = result {
        print_error(&format!("Error: {}", e));
        process::exit(1);
    }

    Ok(())
}

async fn handle_start(config: ProcessConfig) -> Result<()> {
    let spinner = ProgressIndicator::show_spinner(&format!("Starting process '{}'", config.name));
    let client = rpm::ipc::IpcClient::new().await?;
    client.start_process(config).await?;
    spinner.finish_and_clear();
    print_success("Process started successfully");
    Ok(())
}

async fn handle_stop(name: String) -> Result<()> {
    let spinner = ProgressIndicator::show_spinner(&format!("Stopping process '{}'", name));
    let client = rpm::ipc::IpcClient::new().await?;
    client.stop_process(&name).await?;
    spinner.finish_and_clear();
    print_success(&format!("Process '{}' stopped", name));
    Ok(())
}

async fn handle_restart(name: String) -> Result<()> {
    let spinner = ProgressIndicator::show_spinner(&format!("Restarting process '{}'", name));
    let client = rpm::ipc::IpcClient::new().await?;
    client.restart_process(&name).await?;
    spinner.finish_and_clear();
    print_success(&format!("Process '{}' restarted", name));
    Ok(())
}

async fn handle_delete(name: String) -> Result<()> {
    let spinner = ProgressIndicator::show_spinner(&format!("Deleting process '{}'", name));
    let client = rpm::ipc::IpcClient::new().await?;
    client.delete_process(&name).await?;
    spinner.finish_and_clear();
    print_success(&format!("Process '{}' deleted", name));
    Ok(())
}

async fn handle_list() -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    let processes = client.list_processes().await?;
    
    print_header("Process List");
    let process_refs: Vec<&_> = processes.iter().collect();
    println!("{}", TableFormatter::format_process_list(&process_refs));
    
    Ok(())
}

async fn handle_logs(name: String, lines: usize, follow: bool) -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    
    if follow {
        print_header(&format!("Following logs for '{}'", name));
        print_info("Press Ctrl+C to exit");
        println!();
        
        // Get initial logs
        let initial_logs = client.get_logs(&name, lines, false).await?;
        for log in initial_logs {
            println!("{}", format_log_line(&log));
        }
        
        // Follow new logs
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        let mut last_log_count = lines;
        
        loop {
            interval.tick().await;
            
            match client.get_logs(&name, last_log_count + 50, false).await {
                Ok(logs) => {
                    if logs.len() > last_log_count {
                        for log in logs.iter().skip(last_log_count) {
                            println!("{}", format_log_line(log));
                        }
                        last_log_count = logs.len();
                    }
                }
                Err(e) => {
                    print_error(&format!("Error following logs: {}", e));
                    break;
                }
            }
        }
    } else {
        let logs = client.get_logs(&name, lines, false).await?;
        
        if logs.is_empty() {
            print_warning(&format!("No logs found for process '{}'", name));
            return Ok(());
        }
        
        print_header(&format!("Logs for '{}' (last {} lines)", name, lines));
        for log in logs {
            println!("{}", format_log_line(&log));
        }
    }
    
    Ok(())
}

fn format_log_line(log: &str) -> String {
    // Try to parse timestamp and format the log line with colors
    if let Some(timestamp_end) = log.find(']') {
        if log.starts_with('[') {
            let timestamp = &log[1..timestamp_end];
            let message = &log[timestamp_end + 1..].trim_start();
            
            // Color code based on log level
            let colored_message = if message.to_lowercase().contains("error") {
                message.bright_red()
            } else if message.to_lowercase().contains("warn") {
                message.bright_yellow()
            } else if message.to_lowercase().contains("info") {
                message.bright_blue()
            } else if message.to_lowercase().contains("debug") {
                message.bright_black()
            } else {
                message.bright_white()
            };
            
            format!("{} {}", 
                format!("[{}]", timestamp).bright_magenta(), 
                colored_message
            )
        } else {
            log.bright_white().to_string()
        }
    } else {
        log.bright_white().to_string()
    }
}

async fn handle_show(name: String) -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    let info = client.get_process_info(&name).await?;
    
    println!("{}", TableFormatter::format_process_details(&info));
    
    Ok(())
}

async fn handle_monitor() -> Result<()> {
    let client = rpm::ipc::IpcClient::new().await?;
    
    print_header("Process Monitor");
    print_info("Press Ctrl+C to exit");
    println!();
    
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
    loop {
        interval.tick().await;
        
        match client.list_processes().await {
            Ok(processes) => {
                // Clear screen
                print!("\x1B[2J\x1B[1;1H");
                print_header("Process Monitor");
                println!("{}", format!("Last updated: {}", chrono::Utc::now().format("%H:%M:%S UTC")).bright_black());
                println!();
                
                let process_refs: Vec<&_> = processes.iter().collect();
                println!("{}", TableFormatter::format_process_list(&process_refs));
                
                if processes.is_empty() {
                    println!();
                    print_info("No processes running");
                }
            }
            Err(e) => {
                print_error(&format!("Error fetching process list: {}", e));
                break;
            }
        }
    }
    
    Ok(())
}

async fn handle_daemon(foreground: bool) -> Result<()> {
    if foreground {
        print_header("RPM Process Manager");
        print_info("Starting daemon in foreground mode...");
        println!();
    } else {
        print_info("Starting RPM daemon in background...");
    }
    rpm::daemon::start_daemon(foreground).await?;
    Ok(())
}

async fn handle_kill() -> Result<()> {
    let spinner = ProgressIndicator::show_spinner("Stopping daemon");
    let client = rpm::ipc::IpcClient::new().await?;
    client.kill_daemon().await?;
    spinner.finish_and_clear();
    print_success("Daemon stopped");
    Ok(())
}

async fn handle_reload(name: String) -> Result<()> {
    let spinner = ProgressIndicator::show_spinner(&format!("Reloading process '{}'", name));
    let client = rpm::ipc::IpcClient::new().await?;
    client.reload_process(&name).await?;
    spinner.finish_and_clear();
    print_success(&format!("Process '{}' reloaded", name));
    Ok(())
}

async fn handle_save() -> Result<()> {
    let spinner = ProgressIndicator::show_spinner("Saving process list");
    let client = rpm::ipc::IpcClient::new().await?;
    client.save_processes().await?;
    spinner.finish_and_clear();
    print_success("Process list saved");
    Ok(())
}

async fn handle_resurrect() -> Result<()> {
    let spinner = ProgressIndicator::show_spinner("Resurrecting processes");
    let client = rpm::ipc::IpcClient::new().await?;
    client.resurrect_processes().await?;
    spinner.finish_and_clear();
    print_success("Processes resurrected");
    Ok(())
}

async fn handle_status() -> Result<()> {
    match rpm::ipc::IpcClient::new().await {
        Ok(client) => {
            match client.list_processes().await {
                Ok(processes) => {
                    print_header("RPM Daemon Status");
                    print_success("Daemon is running");
                    
                    let running = processes.iter().filter(|p| p.status == rpm::process::ProcessStatus::Running).count();
                    let stopped = processes.iter().filter(|p| p.status == rpm::process::ProcessStatus::Stopped).count();
                    let errored = processes.iter().filter(|p| p.status == rpm::process::ProcessStatus::Errored).count();
                    
                    println!();
                    println!("{:<20} {}", "Total processes:".bright_white(), processes.len().to_string().bright_yellow());
                    println!("{:<20} {}", "Running:".bright_white(), running.to_string().bright_green());
                    println!("{:<20} {}", "Stopped:".bright_white(), stopped.to_string().bright_red());
                    println!("{:<20} {}", "Errored:".bright_white(), errored.to_string().bright_red());
                    
                    if !processes.is_empty() {
                        println!();
                        let process_refs: Vec<&_> = processes.iter().collect();
                        println!("{}", TableFormatter::format_process_list(&process_refs));
                    }
                }
                Err(e) => {
                    print_error(&format!("Error getting daemon status: {}", e));
                }
            }
        }
        Err(_) => {
            print_error("Daemon is not running");
            println!();
            print_info("Start the daemon with: rpm daemon");
        }
    }
    Ok(())
}