#!/usr/bin/env bash
# Cleanup all BRRTRouter test containers
# This should be run after tests to clean up any orphaned containers

set -euo pipefail

echo "🧹 Cleaning up BRRTRouter test containers..."

# Find all brrtrouter-e2e containers
containers=$(docker ps -a -q --filter "name=brrtrouter-e2e" 2>/dev/null || true)

if [ -z "$containers" ]; then
    echo "✓ No orphaned containers found"
    exit 0
fi

echo "Found containers to clean up:"
docker ps -a --filter "name=brrtrouter-e2e" --format "table {{.ID}}\t{{.Names}}\t{{.Status}}\t{{.Ports}}"

echo ""
echo "Removing containers..."
echo "$containers" | xargs docker rm -f

echo "✓ Cleanup complete!"
echo ""
echo "Verifying..."
remaining=$(docker ps -a -q --filter "name=brrtrouter-e2e" 2>/dev/null || true)
if [ -z "$remaining" ]; then
    echo "✓ All containers removed successfully"
else
    echo "⚠ Warning: Some containers still remain:"
    docker ps -a --filter "name=brrtrouter-e2e"
fi

