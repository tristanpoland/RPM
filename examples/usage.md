# RPM Usage Examples

## Basic Usage

### 1. Start the daemon
```bash
# Start daemon in background (as service on Windows, daemon on Unix)
rpm daemon

# Or start in foreground for debugging
rpm daemon --foreground
```

### 2. Start processes
```bash
# Start a simple process
rpm start "python examples/demo.py" --name demo

# Start with custom configuration
rpm start "node server.js" --name myapp --cwd /path/to/app --instances 2 --env "PORT=3000" --env "NODE_ENV=production"

# Start with memory limit
rpm start "java -jar app.jar" --name javaapp --max-memory 512
```

### 3. Manage processes
```bash
# List all processes
rpm list

# Stop a process
rpm stop demo

# Restart a process  
rpm restart demo

# Delete a process
rpm delete demo
```

### 4. Monitor processes
```bash
# View logs
rpm logs demo --lines 20

# Follow logs in real-time
rpm logs demo --follow

# Monitor all processes
rpm monitor

# Show detailed process info
rpm show demo
```

### 5. Process persistence
```bash
# Save current process configuration
rpm save

# Restore saved processes (useful after reboot)
rpm resurrect
```

### 6. Stop the daemon
```bash
rpm kill
```

## Example Scenarios

### Web Server Management
```bash
# Start multiple instances of a web server
rpm start "node app.js" --name webapp --instances 4 --env "PORT=3000"

# Monitor the web server
rpm monitor

# View web server logs
rpm logs webapp --follow
```

### Development Workflow
```bash
# Start development services
rpm start "npm run dev" --name frontend --cwd /path/to/frontend
rpm start "python manage.py runserver" --name backend --cwd /path/to/backend
rpm start "redis-server" --name redis

# List all services
rpm list

# Stop all when done
rpm stop frontend
rpm stop backend  
rpm stop redis
```

### Production Deployment
```bash
# Start production app with memory limit and auto-restart
rpm start "java -jar myapp.jar" --name myapp --max-memory 1024 --autorestart true

# Save configuration for persistence across reboots
rpm save

# Monitor the application
rpm monitor
```

## Windows-Specific Usage

On Windows, the daemon runs as a Windows service:

```cmd
# Install and start service (requires admin privileges)
rpm daemon

# The service will auto-start on boot
# View service status in Services.msc

# Stop the service
rpm kill
```

## Unix-Specific Usage

On Unix systems, the daemon runs as a background daemon:

```bash
# Start daemon (will fork to background)
rpm daemon

# Check if daemon is running
ps aux | grep rpm-daemon

# Stop daemon
rpm kill
```

## Configuration

RPM stores configuration in:
- Windows: `%APPDATA%\rpm\`
- Linux: `~/.config/rpm/`
- macOS: `~/Library/Application Support/rpm/`

Key files:
- `config.json` - Daemon configuration
- `processes.json` - Saved process configurations
- `logs/` - Process log files