#!/usr/bin/env bash
# BRRTRouter Development Environment Teardown
# Cleans up kind cluster and resources

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║         BRRTRouter Development Environment Teardown                 ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

CLUSTER_NAME="brrtrouter-dev"

# ============================================================================
# Check if Tilt is Running
# ============================================================================

echo -e "${BLUE}🔍 Checking for running Tilt processes...${NC}"

if pgrep -x "tilt" > /dev/null; then
    echo -e "${YELLOW}⚠️  Tilt is still running${NC}"
    echo -e "${BLUE}Please run 'tilt down' or press Ctrl+C in the Tilt terminal first${NC}"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# ============================================================================
# Delete kind Cluster
# ============================================================================

if ! kind get clusters 2>/dev/null | grep -q "^${CLUSTER_NAME}$"; then
    echo -e "${YELLOW}⚠️  kind cluster '${CLUSTER_NAME}' does not exist${NC}"
    echo -e "${GREEN}Nothing to clean up!${NC}"
    exit 0
fi

echo -e "${BLUE}🗑️  Deleting kind cluster '${CLUSTER_NAME}'...${NC}"
kind delete cluster --name "${CLUSTER_NAME}"

echo -e "${GREEN}✓${NC} kind cluster deleted"
echo ""

# ============================================================================
# Local Registry (Preserved by default for fast rebuilds)
# ============================================================================

REG_NAME='kind-registry'

echo -e "${BLUE}📦 Checking local registry...${NC}"

if [ "$(docker inspect -f '{{.State.Running}}' "${REG_NAME}" 2>/dev/null || true)" = 'true' ]; then
    echo -e "${GREEN}✓${NC} Local registry '${REG_NAME}' is running (preserved for fast rebuilds)"
    echo -e "${BLUE}💡 Tip:${NC} Images in the registry will be reused on next setup"
    echo -e "   To remove registry: ${YELLOW}docker rm -f ${REG_NAME}${NC}"
else
    echo -e "${BLUE}ℹ️  Local registry not running${NC}"
fi

echo ""

# ============================================================================
# Clean up Docker Images (Optional)
# ============================================================================

echo -e "${BLUE}🐳 Checking for BRRTRouter Docker images...${NC}"

IMAGES=$(docker images --filter=reference='*brrtrouter-petstore*' --format '{{.Repository}}:{{.Tag}}' | wc -l)

if [ "$IMAGES" -gt 0 ]; then
    echo -e "${YELLOW}Found $IMAGES BRRTRouter Docker image(s)${NC}"
    read -p "Do you want to remove them? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        docker images --filter=reference='*brrtrouter-petstore*' --format '{{.Repository}}:{{.Tag}}' | xargs -r docker rmi
        echo -e "${GREEN}✓${NC} Docker images removed"
    fi
else
    echo -e "${GREEN}✓${NC} No BRRTRouter Docker images found"
fi

echo ""

# ============================================================================
# Clean up Tilt Cache (Optional)
# ============================================================================

if [ -d ".tilt-cache" ]; then
    echo -e "${BLUE}📦 Found Tilt cache directory${NC}"
    read -p "Do you want to remove it? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf .tilt-cache
        echo -e "${GREEN}✓${NC} Tilt cache removed"
    fi
fi

echo ""

# ============================================================================
# Success Message
# ============================================================================

echo -e "${GREEN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║                    Teardown Complete! 🧹                             ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}Development environment has been cleaned up.${NC}"
echo -e "${BLUE}Run './scripts/dev-setup.sh' to set it up again.${NC}"
echo ""

