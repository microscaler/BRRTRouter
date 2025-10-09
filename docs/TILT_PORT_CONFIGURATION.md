# Tilt Port Configuration

## 🎯 Default Port

Tilt web UI now runs on **port 10351** by default (changed from 10350 to avoid common conflicts).

## 🔧 Changing the Tilt Port

If port 10351 is already in use, you have several options:

### Option 1: Environment Variable (Recommended)

Set the `TILT_PORT` environment variable before running Tilt:

```bash
# One-time use
TILT_PORT=10352 tilt up

# Or with just
TILT_PORT=10352 just dev-up

# Set permanently in your shell
export TILT_PORT=10352
tilt up
```

### Option 2: Modify Tiltfile

Edit the default port directly in `Tiltfile`:

```python
# Change this line (around line 12)
tilt_port = cfg.get('tilt_port', '10351')  # Change 10351 to your preferred port
```

### Option 3: Shell Alias

Add to your `~/.zshrc` or `~/.bashrc`:

```bash
# Always use port 10352 for this project
alias tilt-brrt='TILT_PORT=10352 tilt'

# Then use:
tilt-brrt up
```

## 🛠️ All Service Ports

Here's a complete reference of all ports used by BRRTRouter local development:

| Service | Port | Protocol | Configurable | Notes |
|---------|------|----------|--------------|-------|
| **Pet Store API** | 8080 | HTTP | Via Tiltfile | Standard HTTP port |
| **Grafana** | 3000 | HTTP | Via Tiltfile | Dashboard UI |
| **PostgreSQL** | 5432 | TCP | Via Tiltfile | Database |
| **Redis** | 6379 | TCP | Via Tiltfile | Cache |
| **Prometheus** | 9090 | HTTP | Via Tiltfile | Metrics |
| **Jaeger UI** | 16686 | HTTP | Via Tiltfile | Tracing |
| **Tilt Web UI** | 10351 | HTTP | `TILT_PORT` | Dev dashboard |

## 🔍 Checking Port Conflicts

Before starting Tilt, check for port conflicts:

```bash
# Check if port 10351 is in use
lsof -i :10351

# Check all BRRTRouter ports at once
lsof -i :8080 -i :3000 -i :5432 -i :6379 -i :9090 -i :16686 -i :10351

# Kill process on specific port (if needed)
kill -9 $(lsof -ti:10351)
```

## 🚀 Accessing the Tilt Dashboard

Once Tilt is running:

### Method 1: Press Space (Recommended)
In the terminal where `tilt up` is running, press the **spacebar** to automatically open the Tilt web UI in your browser.

### Method 2: Direct URL
Open your browser to the configured port:
- Default: http://localhost:10351
- Custom: http://localhost:YOUR_PORT

### Method 3: Command Line
```bash
# Open automatically
open http://localhost:10351

# Or on Linux
xdg-open http://localhost:10351
```

## 📊 Tilt UI Features

The Tilt dashboard shows:
- ✅ Build status for all resources
- 📊 Logs for each service
- 🔄 Live update status
- ⚡ Build times and performance
- 🎯 Resource dependencies
- 🔍 Error highlighting

## 🐛 Troubleshooting

### "Address already in use" Error

```bash
# Find what's using the port
lsof -i :10351

# Option 1: Stop the conflicting process
kill $(lsof -ti:10351)

# Option 2: Use a different port
TILT_PORT=10352 tilt up
```

### Tilt UI Not Opening

```bash
# Check if Tilt is actually running
ps aux | grep tilt

# Check the port Tilt is using
netstat -an | grep LISTEN | grep 103

# Try accessing directly
curl http://localhost:10351
```

### Port Configuration Not Working

```bash
# Verify environment variable is set
echo $TILT_PORT

# Check Tiltfile picked it up
# Look for "Tilt Dashboard" line in Tilt output when starting

# Force reload Tiltfile
tilt down
tilt up
```

## 💡 Pro Tips

### Multiple BRRTRouter Instances

If you need to run multiple BRRTRouter environments simultaneously:

```bash
# Terminal 1 - Main project
cd ~/projects/BRRTRouter
TILT_PORT=10351 tilt up

# Terminal 2 - Fork/branch
cd ~/projects/BRRTRouter-fork
TILT_PORT=10361 tilt up  # Different ports!
```

### Port Range Recommendations

Safe port ranges to avoid common conflicts:
- **10351-10399**: Available for Tilt and dev tools
- **Avoid**: 3000 (Node), 8000 (Django), 8080 (common HTTP), 9000 (PHP-FPM)

### Check All Ports Before Starting

Create a helper script `scripts/check-ports.sh`:

```bash
#!/bin/bash
PORTS=(8080 3000 5432 6379 9090 16686 10351)
ALL_FREE=true

for PORT in "${PORTS[@]}"; do
    if lsof -Pi :$PORT -sTCP:LISTEN -t >/dev/null ; then
        echo "❌ Port $PORT is in use"
        ALL_FREE=false
    else
        echo "✅ Port $PORT is available"
    fi
done

if [ "$ALL_FREE" = false ]; then
    echo ""
    echo "⚠️  Some ports are in use. Consider stopping conflicting services or changing ports."
    exit 1
fi

echo ""
echo "🎉 All ports are available! Ready to start Tilt."
```

Then run before starting:
```bash
./scripts/check-ports.sh && tilt up
```

## 📚 Related Documentation

- [Tilt Official Docs](https://docs.tilt.dev/)
- [Local Development Guide](LOCAL_DEVELOPMENT.md)
- [Port Mapping Fix](PORT_MAPPING_FIX.md)

---

**Last Updated**: October 9, 2025
**Default Tilt Port**: 10351 (configurable via `TILT_PORT`)

