#!/usr/bin/env bash
# BRRTRouter Development Environment Setup
# Creates kind cluster and prepares for Tilt

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║         BRRTRouter Development Environment Setup                    ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# ============================================================================
# Check Prerequisites
# ============================================================================

echo -e "${BLUE}🔍 Checking prerequisites...${NC}"

check_command() {
    if command -v "$1" &> /dev/null; then
        echo -e "${GREEN}✓${NC} $1 is installed"
        return 0
    else
        echo -e "${RED}✗${NC} $1 is not installed"
        return 1
    fi
}

MISSING_DEPS=0

check_command docker || MISSING_DEPS=1
check_command kind || MISSING_DEPS=1
check_command kubectl || MISSING_DEPS=1
check_command tilt || MISSING_DEPS=1
check_command cargo || MISSING_DEPS=1

if [ $MISSING_DEPS -eq 1 ]; then
    echo ""
    echo -e "${RED}❌ Missing required dependencies!${NC}"
    echo ""
    echo -e "${YELLOW}Installation instructions:${NC}"
    echo ""
    echo -e "  ${BLUE}Docker:${NC}"
    echo "    macOS: brew install --cask docker"
    echo "    Linux: https://docs.docker.com/engine/install/"
    echo ""
    echo -e "  ${BLUE}kind:${NC}"
    echo "    macOS: brew install kind"
    echo "    Linux: curl -Lo ./kind https://kind.sigs.k8s.io/dl/latest/kind-linux-amd64 && chmod +x ./kind && sudo mv ./kind /usr/local/bin/kind"
    echo ""
    echo -e "  ${BLUE}kubectl:${NC}"
    echo "    macOS: brew install kubectl"
    echo "    Linux: https://kubernetes.io/docs/tasks/tools/install-kubectl-linux/"
    echo ""
    echo -e "  ${BLUE}Tilt:${NC}"
    echo "    macOS: brew install tilt"
    echo "    Linux: curl -fsSL https://raw.githubusercontent.com/tilt-dev/tilt/master/scripts/install.sh | bash"
    echo ""
    echo -e "  ${BLUE}Rust/Cargo:${NC}"
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
    exit 1
fi

echo ""

# ============================================================================
# Check Docker
# ============================================================================

echo -e "${BLUE}🐳 Checking Docker...${NC}"

if ! docker info &> /dev/null; then
    echo -e "${RED}✗${NC} Docker daemon is not running"
    echo -e "${YELLOW}Please start Docker and try again${NC}"
    exit 1
fi

echo -e "${GREEN}✓${NC} Docker is running"
echo ""

# ============================================================================
# Create kind Cluster
# ============================================================================

CLUSTER_NAME="brrtrouter-dev"

if kind get clusters 2>/dev/null | grep -q "^${CLUSTER_NAME}$"; then
    echo -e "${YELLOW}⚠️  kind cluster '${CLUSTER_NAME}' already exists${NC}"
    read -p "Do you want to delete and recreate it? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${BLUE}🗑️  Deleting existing cluster...${NC}"
        kind delete cluster --name "${CLUSTER_NAME}"
    else
        echo -e "${GREEN}✓${NC} Using existing cluster"
        kubectl cluster-info --context "kind-${CLUSTER_NAME}"
        echo ""
        echo -e "${GREEN}✅ Setup complete! Run 'tilt up' to start development${NC}"
        exit 0
    fi
fi

echo -e "${BLUE}🚀 Creating kind cluster '${CLUSTER_NAME}'...${NC}"
kind create cluster --config kind-config.yaml --wait 60s

# Verify cluster is ready
echo -e "${BLUE}⏳ Waiting for cluster to be ready...${NC}"
kubectl wait --for=condition=Ready nodes --all --timeout=120s

echo -e "${GREEN}✓${NC} kind cluster is ready"
echo ""

# ============================================================================
# Set kubectl Context
# ============================================================================

echo -e "${BLUE}🔧 Setting kubectl context...${NC}"
kubectl config use-context "kind-${CLUSTER_NAME}"
echo -e "${GREEN}✓${NC} kubectl context set to kind-${CLUSTER_NAME}"
echo ""

# ============================================================================
# Display Cluster Info
# ============================================================================

echo -e "${BLUE}📊 Cluster Information:${NC}"
kubectl cluster-info --context "kind-${CLUSTER_NAME}"
echo ""

# ============================================================================
# Success Message
# ============================================================================

echo -e "${GREEN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║                    Setup Complete! 🎉                                ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}Next steps:${NC}"
echo ""
echo -e "  1. Start Tilt:"
echo -e "     ${YELLOW}tilt up${NC}"
echo ""
echo -e "  2. Or use justfile:"
echo -e "     ${YELLOW}just dev-up${NC}"
echo ""
echo -e "  3. Access services:"
echo -e "     • Pet Store API:  ${YELLOW}http://localhost:8080${NC}"
echo -e "     • Grafana:        ${YELLOW}http://localhost:3000${NC} (admin/admin)"
echo -e "     • Prometheus:     ${YELLOW}http://localhost:9090${NC}"
echo -e "     • Jaeger UI:      ${YELLOW}http://localhost:16686${NC}"
echo ""
echo -e "  4. Run tests:"
echo -e "     ${YELLOW}just curls${NC}  # Test all endpoints"
echo ""
echo -e "${BLUE}💡 Tip:${NC} Press 'space' in the terminal after 'tilt up' to open the web UI"
echo ""

