"""Setup local Docker registry for Kind (localhost:5001). Replaces setup-kind-registry.sh."""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

REG_NAME = "kind-registry"
REG_PORT = "5001"


def _docker_run(args: list[str], **kwargs: Any) -> subprocess.CompletedProcess[str]:
    """Run a docker subprocess; turn FileNotFoundError into a failed result."""
    try:
        return subprocess.run(["docker", *args], **kwargs)
    except FileNotFoundError:
        print(
            "[ERROR] docker executable not found when invoking subprocess.",
            file=sys.stderr,
        )
        return subprocess.CompletedProcess(["docker", *args], 127, "", "docker not found")


def run(project_root: Path) -> int:
    """Create/start kind-registry, connect to kind network, optional ConfigMap. Returns 0 or 1."""
    if not shutil.which("docker"):
        print(
            "[ERROR] docker not found in PATH. Install Docker Desktop or the docker CLI.",
            file=sys.stderr,
        )
        return 1

    # 1. Create or start registry
    inspect = _docker_run(["inspect", REG_NAME], capture_output=True, text=True)
    if inspect.returncode != 0:
        print(f"📦 Creating local registry: {REG_NAME} (host port {REG_PORT})")
        cr = _docker_run(
            [
                "run",
                "-d",
                "--restart=always",
                "-p",
                f"127.0.0.1:{REG_PORT}:5000",
                "--network",
                "bridge",
                "--name",
                REG_NAME,
                "registry:2",
            ],
            check=False,
        )
        if cr.returncode != 0:
            print(
                f"[ERROR] docker run failed (exit {cr.returncode}): {cr.stderr or cr.stdout}",
                file=sys.stderr,
            )
            return 1
        print(f"   Created and started {REG_NAME}")
    else:
        state = _docker_run(
            ["inspect", "-f", "{{.State.Running}}", REG_NAME],
            capture_output=True,
            text=True,
        )
        if (state.stdout or "").strip() != "true":
            print(f"📦 Starting existing registry: {REG_NAME}")
            st = _docker_run(["start", REG_NAME], check=False)
            if st.returncode != 0:
                print(
                    f"[ERROR] docker start failed (exit {st.returncode}): {st.stderr or st.stdout}",
                    file=sys.stderr,
                )
                return 1
            print(f"   Started {REG_NAME}")
        else:
            print(f"📦 Registry already running: {REG_NAME}")

    # 2. Connect to kind network
    net = _docker_run(
        ["network", "inspect", "kind"],
        capture_output=True,
        text=True,
    )
    if net.returncode != 0:
        print("⚠️  Docker network 'kind' not found. Create a Kind cluster first:")
        print("   kind create cluster --config kind-config.yaml")
        return 1
    nets = _docker_run(
        [
            "inspect",
            "-f",
            "{{json .NetworkSettings.Networks.kind}}",
            REG_NAME,
        ],
        capture_output=True,
        text=True,
    )
    if (nets.stdout or "").strip() == "null":
        cn = _docker_run(["network", "connect", "kind", REG_NAME], check=False)
        if cn.returncode != 0:
            print(
                f"[ERROR] docker network connect failed (exit {cn.returncode}): "
                f"{cn.stderr or cn.stdout}",
                file=sys.stderr,
            )
            return 1
        print(f"🔗 Connected {REG_NAME} to kind")
    else:
        print("🔗 Registry already on kind network")

    # 3. ConfigMap (optional)
    if (
        shutil.which("kubectl")
        and subprocess.run(["kubectl", "cluster-info"], capture_output=True).returncode == 0
    ):
        cm = f"""apiVersion: v1
kind: ConfigMap
metadata:
  name: local-registry-hosting
  namespace: kube-public
data:
  localRegistryHosting.v1: |
    host: "localhost:{REG_PORT}"
    help: "https://kind.sigs.k8s.io/docs/user/local-registry/"
"""
        try:
            subprocess.run(
                ["kubectl", "apply", "-f", "-"],
                input=cm,
                capture_output=True,
                text=True,
                check=False,
            )
        except FileNotFoundError:
            print(
                "[WARN] kubectl disappeared from PATH; skipping local-registry ConfigMap.",
                file=sys.stderr,
            )
    print(f"✅ Local registry ready: push images to localhost:{REG_PORT}/<image>:<tag>")
    return 0
