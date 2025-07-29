# Windows Setup Guide for RPM

## Running RPM on Windows

RPM can run on Windows in two modes:

### 1. Foreground Mode (No Admin Required)
```cmd
rpm daemon --foreground
```
This runs the daemon in the current console window. The daemon will stop when you close the window or press Ctrl+C.

### 2. Windows Service Mode (Admin Required)
```cmd
# Must run as Administrator
rpm daemon
```

## Installing as Windows Service

### Option A: Using Administrator Command Prompt
1. Open Command Prompt as Administrator
   - Press `Win + X`
   - Select "Command Prompt (Admin)" or "PowerShell (Admin)"
2. Navigate to your RPM directory
3. Run: `rpm daemon`

### Option B: Using the Install Script
1. Right-click `install-service.bat`
2. Select "Run as administrator"

### Option C: Using PowerShell as Admin
```powershell
# Open PowerShell as Administrator
Start-Process powershell -Verb runAs
# Then run:
rpm daemon
```

## Checking Service Status

```cmd
# Check if service is running
sc query RPMDaemon

# Start the service
sc start RPMDaemon

# Stop the service
sc stop RPMDaemon

# Remove the service
sc delete RPMDaemon
```

## Using RPM Commands

Once the daemon is running (either mode), you can use all RPM commands normally:

```cmd
# Start a process
rpm start "python app.py" --name myapp

# List processes
rpm list

# Stop a process
rpm stop myapp

# View logs
rpm logs myapp

# Monitor processes
rpm monitor
```

## Troubleshooting

### Port Already in Use
If you get "port already in use" error:
```cmd
# Check what's using port 9999
netstat -ano | findstr :9999

# Kill the process if needed
taskkill /PID <process_id> /F

# Or change the port in config.json
```

### Service Installation Fails
- Make sure you're running as Administrator
- Check if service already exists: `sc query RPMDaemon`
- If it exists, delete it first: `sc delete RPMDaemon`

### Service Won't Start
- Check Event Viewer for error details
- Try running in foreground mode first: `rpm daemon --foreground`
- Check the daemon logs

## Service Logs

Windows service logs are written to:
- Event Viewer: Windows Logs > Application
- Service logs: `%APPDATA%\rpm\logs\`

## Auto-Start Configuration

When installed as a service, RPM will automatically:
- Start when Windows boots
- Restart if it crashes
- Run in the background without a console window
- Restore saved processes on startup