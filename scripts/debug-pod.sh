#!/bin/bash

echo "=== BRRTRouter Pod Diagnostics ==="
echo ""

echo "📊 Pod Status:"
kubectl get pods -n brrtrouter-dev -l app=petstore
echo ""

echo "📋 Pod Events (last 10):"
kubectl get events -n brrtrouter-dev --field-selector involvedObject.name=$(kubectl get pod -n brrtrouter-dev -l app=petstore -o jsonpath='{.items[0].metadata.name}') --sort-by='.lastTimestamp' | tail -10
echo ""

echo "📝 Current Logs (last 50 lines, filtered):"
kubectl logs -n brrtrouter-dev deployment/petstore --tail=50 | grep -v "TooManyHeaders" || echo "No logs or pod not running"
echo ""

echo "🔍 Pod Description:"
kubectl describe pod -n brrtrouter-dev -l app=petstore | grep -A 10 "State:"
echo ""

echo "💾 Resource Usage:"
kubectl top pod -n brrtrouter-dev -l app=petstore 2>/dev/null || echo "Metrics server not available"
echo ""

echo "🔧 Container Status:"
kubectl get pod -n brrtrouter-dev -l app=petstore -o jsonpath='{.items[0].status.containerStatuses[0]}' | jq '.' 2>/dev/null || echo "Pod not found"
echo ""

echo "📂 Files in Container (if running):"
kubectl exec -n brrtrouter-dev deployment/petstore -- ls -la /app/ 2>/dev/null || echo "Pod not ready for exec"
echo ""

echo "🌐 Network Connectivity:"
kubectl exec -n brrtrouter-dev deployment/petstore -- wget -qO- --timeout=2 http://otel-collector:4317 2>/dev/null && echo "✅ OTEL Collector reachable" || echo "❌ OTEL Collector unreachable"
kubectl exec -n brrtrouter-dev deployment/petstore -- wget -qO- --timeout=2 http://prometheus:9090/-/healthy 2>/dev/null && echo "✅ Prometheus reachable" || echo "❌ Prometheus unreachable"
echo ""

echo "=== Quick Fixes ==="
echo ""
echo "If pod is crashlooping:"
echo "  kubectl logs -n brrtrouter-dev deployment/petstore --previous"
echo ""
echo "If you need to restart:"
echo "  kubectl rollout restart deployment/petstore -n brrtrouter-dev"
echo ""
echo "To reduce log noise, edit k8s/petstore-deployment.yaml:"
echo "  env:"
echo "    - name: RUST_LOG"
echo "      value: \"warn,pet_store=info\""
echo ""

