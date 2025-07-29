# RPM - Rust Process Manager

A cross-platform process manager similar to PM2, written in Rust with full Windows and Linux compatibility.

## Features

- **Cross-platform**: Works on Windows (as a service) and Unix-like systems (as a daemon)
- **Process Management**: Start, stop, restart, and delete processes
- **Auto-restart**: Automatically restart failed processes
- **Resource Monitoring**: Track CPU and memory usage
- **Logging**: Built-in log management and viewing
- **IPC Communication**: Fast inter-process communication between CLI and daemon
- **Persistence**: Save and restore process configurations
- **Health Checks**: Monitor process health and memory limits

## Installation

### From Source

```bash
git clone <repository-url>
cd rpm
cargo build --release
```

The build will create two binaries:
- `rpm` - The CLI client
- `rpm-daemon` - The daemon process

## Usage

### Starting the Daemon

On Linux/macOS:
```bash
# Start daemon in background
rpm daemon

# Start daemon in foreground (for debugging)
rpm daemon --foreground
```

On Windows:
```bash
# Install and start as Windows service
rpm daemon

# Run in foreground
rpm daemon --foreground
```

### Process Management

```bash
# Start a process
rpm start "node app.js" --name myapp --instances 2

# Start with custom working directory and environment
rpm start "python server.py" --name api --cwd /path/to/app --env "PORT=3000" --env "NODE_ENV=production"

# List all processes
rpm list

# Stop a process
rpm stop myapp

# Restart a process
rpm restart myapp

# Delete a process
rpm delete myapp

# Show detailed process information
rpm show myapp
```

### Monitoring

```bash
# View logs
rpm logs myapp --lines 50

# Follow logs in real-time
rpm logs myapp --follow

# Monitor all processes in real-time
rpm monitor
```

### Configuration Management

```bash
# Save current process list
rpm save

# Restore saved processes
rpm resurrect

# Reload a process configuration
rpm reload myapp
```

### Daemon Control

```bash
# Stop the daemon
rpm kill
```

## Configuration

RPM stores its configuration in platform-specific directories:

- **Linux**: `~/.config/rpm/`
- **macOS**: `~/Library/Application Support/rpm/`
- **Windows**: `%APPDATA%/rpm/`

### Configuration Files

- `config.json` - Main daemon configuration
- `processes.json` - Saved process configurations
- `logs/` - Process log files

### Default Configuration

```json
{
  "daemon_port": 9999,
  "max_processes": 1000,
  "log_max_size": 104857600,
  "log_retention_days": 30,
  "auto_restart_delay": 5,
  "health_check_interval": 5
}
```

## Process Configuration Options

When starting processes, you can specify:

- `--name`: Process name (defaults to command name)
- `--cwd`: Working directory
- `--instances`: Number of instances to start
- `--autorestart`: Enable/disable auto-restart (default: true)
- `--max-memory`: Maximum memory usage in MB
- `--env`: Environment variables (format: `KEY=VALUE`)

## Architecture

RPM consists of two main components:

1. **CLI Client** (`rpm`): Handles user commands and communicates with the daemon
2. **Daemon** (`rpm-daemon`): Manages processes, handles IPC, and provides monitoring

### IPC Communication

- **Unix systems**: Unix domain sockets (`~/.local/share/rpm/rpm.sock`)
- **Windows**: TCP sockets (localhost:9999)

### Cross-Platform Daemon

- **Linux/macOS**: Traditional daemon with proper signal handling
- **Windows**: Native Windows service with automatic installation

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

### Dependencies

Key dependencies include:
- `clap` - Command line argument parsing
- `tokio` - Async runtime
- `serde` - Serialization
- `tracing` - Logging
- `windows-service` - Windows service support (Windows only)
- `daemonize` - Unix daemon support (Unix only)

## License

This project is licensed under the MIT OR Apache-2.0 license.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Platform-Specific Notes

### Windows

- Requires administrator privileges to install as a service
- Uses Windows API for process monitoring
- Stores configuration in `%APPDATA%\rpm\`

### Linux/Unix

- Uses `/proc` filesystem for process monitoring
- Supports proper signal handling (SIGTERM, SIGKILL)
- Stores configuration in `~/.config/rpm/` (XDG Base Directory)

## Troubleshooting

### Common Issues

1. **Daemon won't start**: Check if port 9999 is available (Windows) or socket permissions (Unix)
2. **Processes not auto-restarting**: Verify `autorestart` is enabled and check daemon logs
3. **Permission errors**: Ensure proper permissions for config directories

### Debugging

Run the daemon in foreground mode to see detailed logs:

```bash
rpm daemon --foreground
```

### Log Locations

- **Daemon logs**: Check system journal (Linux) or Event Viewer (Windows)
- **Process logs**: `~/.local/share/rpm/logs/` (Linux) or `%APPDATA%\rpm\logs\` (Windows)