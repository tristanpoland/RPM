use crate::{Result, RpmError};
use std::process::Command;
use tokio::time::Duration;

#[cfg(windows)]
pub use windows_service::run_windows_service;

pub async fn start_daemon(foreground: bool) -> Result<()> {
    if foreground {
        start_daemon_foreground().await
    } else {
        start_daemon_background().await
    }
}

async fn start_daemon_foreground() -> Result<()> {
    tracing::info!("Starting RPM daemon in foreground mode");
    
    let daemon = DaemonManager::new().await?;
    daemon.run().await
}

async fn start_daemon_background() -> Result<()> {
    #[cfg(windows)]
    {
        match windows_service::install_and_start_service().await {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("Failed to install as Windows service: {}", e);
                Err(e)
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        macos_service::install_and_start_launchd_service().await
    }
    #[cfg(target_os = "linux")]
    {
        linux_service::install_and_start_systemd_service().await
    }
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "linux")))]
    {
        unix_daemon::daemonize_and_start().await
    }
    #[cfg(not(any(windows, unix)))]
    {
        Err(RpmError::Daemon(
            "Background service mode is not supported on this platform. Please run in foreground mode with --foreground.".to_string()
        ))
    }
}

pub struct DaemonManager {
    process_manager: crate::process::ProcessManager,
    ipc_server: crate::ipc::IpcServer,
}

impl DaemonManager {
    pub async fn new() -> Result<Self> {
        let process_manager = crate::process::ProcessManager::new().await?;
        let ipc_server = crate::ipc::IpcServer::new().await?;
        
        Ok(DaemonManager {
            process_manager,
            ipc_server,
        })
    }

    pub async fn run(self) -> Result<()> {
        tracing::info!("RPM daemon started");
        
        let process_manager = std::sync::Arc::new(tokio::sync::Mutex::new(self.process_manager));
        let pm_clone = process_manager.clone();
        
        let monitor_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Ok(mut pm) = pm_clone.try_lock() {
                    if let Err(e) = pm.monitor_processes().await {
                        tracing::error!("Error monitoring processes: {}", e);
                    }
                }
            }
        });

        let ipc_task = tokio::spawn(async move {
            if let Err(e) = self.ipc_server.run(process_manager).await {
                tracing::error!("IPC server error: {}", e);
            }
        });

        tokio::select! {
            _ = monitor_task => {
                tracing::info!("Monitor task finished");
            }
            _ = ipc_task => {
                tracing::info!("IPC server finished");
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received shutdown signal");
            }
        }

        tracing::info!("RPM daemon shutting down");
        Ok(())
    }
}

#[cfg(windows)]
mod windows_service {
    use super::*;
    use std::ffi::OsString;
    use ::windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher, Result as WinResult,
    };

    const SERVICE_NAME: &str = "RPMDaemon";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    pub async fn install_and_start_service() -> crate::Result<()> {
        let service_path = std::env::current_exe()
            .map_err(|e| RpmError::Daemon(format!("Failed to get current exe: {}", e)))?;

        let mut cmd = Command::new("sc");
        cmd.args(&[
            "create",
            SERVICE_NAME,
            &format!("binPath=\"{}\" --service", service_path.display()),
            "DisplayName=RPM Process Manager",
            "start=auto",
        ]);

        let output = cmd.output()
            .map_err(|e| RpmError::Daemon(format!("Failed to create service: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !error.contains("already exists") && !stdout.contains("already exists") {
                if error.contains("Access is denied") || stdout.contains("Access is denied") {
                    return Err(RpmError::Daemon(
                        "Failed to create service: Access denied. Please run as Administrator.".to_string()
                    ));
                }
                return Err(RpmError::Daemon(format!("Failed to create service: {} {}", error, stdout)));
            }
        }

        let mut cmd = Command::new("sc");
        cmd.args(&["start", SERVICE_NAME]);
        
        let output = cmd.output()
            .map_err(|e| RpmError::Daemon(format!("Failed to start service: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            if error.contains("Access is denied") || stdout.contains("Access is denied") {
                return Err(RpmError::Daemon(
                    "Failed to start service: Access denied. Please run as Administrator.".to_string()
                ));
            }
            return Err(RpmError::Daemon(format!("Failed to start service: {} {}", error, stdout)));
        }

        println!("Service installed and started successfully");
        Ok(())
    }

    define_windows_service!(ffi_service_main, windows_service_main);

    fn windows_service_main(_arguments: Vec<OsString>) {
        if let Err(e) = run_service() {
            tracing::error!("Service error: {}", e);
        }
    }

    fn run_service() -> WinResult<()> {
        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop => {
                    ServiceControlHandlerResult::Other(0)
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = start_daemon_foreground().await {
                tracing::error!("Daemon error: {}", e);
            }
        });

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        Ok(())
    }

    pub fn run_windows_service() -> WinResult<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
    }
}

#[cfg(target_os = "macos")]
mod macos_service {
    use super::*;
    use std::fs;
    use std::path::Path;

    pub async fn install_and_start_launchd_service() -> crate::Result<()> {
        let service_name = "com.rpm.daemon";
        let current_exe = std::env::current_exe()
            .map_err(|e| RpmError::Daemon(format!("Failed to get current exe: {}", e)))?;

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--service</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/rpm-daemon.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/rpm-daemon.err</string>
</dict>
</plist>"#,
            service_name,
            current_exe.display()
        );

        let plist_path = format!("/Library/LaunchDaemons/{}.plist", service_name);
        
        fs::write(&plist_path, plist_content)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    RpmError::Daemon("Failed to write launchd plist: Permission denied. Please run with sudo.".to_string())
                } else {
                    RpmError::Daemon(format!("Failed to write launchd plist: {}", e))
                }
            })?;

        let output = Command::new("launchctl")
            .args(&["load", &plist_path])
            .output()
            .map_err(|e| RpmError::Daemon(format!("Failed to load service: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(RpmError::Daemon(format!("Failed to load service: {} {}", error, stdout)));
        }

        let output = Command::new("launchctl")
            .args(&["start", service_name])
            .output()
            .map_err(|e| RpmError::Daemon(format!("Failed to start service: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(RpmError::Daemon(format!("Failed to start service: {} {}", error, stdout)));
        }

        println!("macOS launchd service installed and started successfully");
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod linux_service {
    use super::*;
    use std::fs;

    pub async fn install_and_start_systemd_service() -> crate::Result<()> {
        let service_name = "rpm-daemon";
        let current_exe = std::env::current_exe()
            .map_err(|e| RpmError::Daemon(format!("Failed to get current exe: {}", e)))?;

        let systemd_content = format!(
            r#"[Unit]
Description=RPM Process Manager Daemon
After=network.target
Wants=network.target

[Service]
Type=simple
ExecStart={} --service
Restart=always
RestartSec=5
User=root
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
"#,
            current_exe.display()
        );

        let service_path = format!("/etc/systemd/system/{}.service", service_name);
        
        fs::write(&service_path, systemd_content)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    RpmError::Daemon("Failed to write systemd service file: Permission denied. Please run with sudo.".to_string())
                } else {
                    RpmError::Daemon(format!("Failed to write systemd service file: {}", e))
                }
            })?;

        let output = Command::new("systemctl")
            .args(&["daemon-reload"])
            .output()
            .map_err(|e| RpmError::Daemon(format!("Failed to reload systemd: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(RpmError::Daemon(format!("Failed to reload systemd: {}", error)));
        }

        let output = Command::new("systemctl")
            .args(&["enable", service_name])
            .output()
            .map_err(|e| RpmError::Daemon(format!("Failed to enable service: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(RpmError::Daemon(format!("Failed to enable service: {}", error)));
        }

        let output = Command::new("systemctl")
            .args(&["start", service_name])
            .output()
            .map_err(|e| RpmError::Daemon(format!("Failed to start service: {}", e)))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(RpmError::Daemon(format!("Failed to start service: {} {}", error, stdout)));
        }

        println!("Linux systemd service installed and started successfully");
        Ok(())
    }
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "linux")))]
mod unix_daemon {
    use super::*;
    use daemonize::Daemonize;
    use std::fs::File;

    pub async fn daemonize_and_start() -> crate::Result<()> {
        let home_dir = directories::ProjectDirs::from("", "", "rpm")
            .ok_or_else(|| RpmError::Daemon("Failed to get home directory".to_string()))?;
        
        let daemon_dir = home_dir.data_dir();
        std::fs::create_dir_all(daemon_dir)
            .map_err(|e| RpmError::Daemon(format!("Failed to create daemon directory: {}", e)))?;

        let stdout = File::create(daemon_dir.join("daemon.out"))
            .map_err(|e| RpmError::Daemon(format!("Failed to create stdout file: {}", e)))?;
        let stderr = File::create(daemon_dir.join("daemon.err"))
            .map_err(|e| RpmError::Daemon(format!("Failed to create stderr file: {}", e)))?;
        let pidfile = daemon_dir.join("daemon.pid");

        let daemonize = Daemonize::new()
            .pid_file(&pidfile)
            .chown_pid_file(true)
            .working_directory(daemon_dir)
            .stdout(stdout)
            .stderr(stderr);

        match daemonize.start() {
            Ok(_) => {
                println!("Daemon started successfully");
                
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| RpmError::Daemon(format!("Failed to create runtime: {}", e)))?;
                
                rt.block_on(async {
                    if let Err(e) = start_daemon_foreground().await {
                        tracing::error!("Daemon error: {}", e);
                    }
                });
                
                Ok(())
            }
            Err(e) => Err(RpmError::Daemon(format!("Failed to start daemon: {}", e))),
        }
    }
}