#!/bin/bash
# Check if all required ports are available for BRRTRouter development

set -e

PORTS=(8080 3000 5432 6379 9090 16686 10351)
ALL_FREE=true

echo "🔍 Checking port availability for BRRTRouter..."
echo ""

for PORT in "${PORTS[@]}"; do
    if lsof -Pi :$PORT -sTCP:LISTEN -t >/dev/null 2>&1 ; then
        PROCESS=$(lsof -Pi :$PORT -sTCP:LISTEN -t | head -1)
        PROCESS_NAME=$(ps -p $PROCESS -o comm= 2>/dev/null || echo "unknown")
        echo "❌ Port $PORT is in use by $PROCESS_NAME (PID: $PROCESS)"
        ALL_FREE=false
    else
        echo "✅ Port $PORT is available"
    fi
done

echo ""

if [ "$ALL_FREE" = false ]; then
    echo "⚠️  Some ports are in use."
    echo ""
    echo "Options:"
    echo "  1. Stop conflicting services"
    echo "  2. Change Tilt port: TILT_PORT=10352 tilt up"
    echo "  3. See docs/TILT_PORT_CONFIGURATION.md for more options"
    echo ""
    exit 1
fi

echo "🎉 All ports are available! Ready to start Tilt."
echo ""
echo "Run: tilt up"
echo "Or:  just dev-up"

