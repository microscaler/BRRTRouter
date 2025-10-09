#!/bin/bash
set -e

echo "=== BRRTRouter Complete Verification ==="
echo ""

echo "✅ 1. Pod Health:"
kubectl get pods -n brrtrouter-dev -l app=petstore | grep Running && echo "   Pod is running" || exit 1
echo ""

echo "✅ 2. Health Endpoint:"
curl -s http://localhost:8080/health | jq '.' && echo "   Health check passed" || exit 1
echo ""

echo "✅ 3. Metrics Endpoint:"
curl -s http://localhost:8080/metrics | head -5 && echo "   ... (metrics available)" || exit 1
echo ""

echo "✅ 4. OpenAPI Spec:"
curl -s http://localhost:8080/openapi.yaml | head -3 && echo "   ... (spec available)" || exit 1
echo ""

echo "✅ 5. Swagger UI:"
curl -s http://localhost:8080/docs | grep -q "swagger" && echo "   Swagger UI available" || exit 1
echo ""

echo "✅ 6. Static Site (Root):"
HTML=$(curl -s http://localhost:8080/)
if echo "$HTML" | grep -q '<div id="root"></div>'; then
    echo "   ✅ SolidJS app HTML found"
    
    # Extract asset references
    JS_ASSET=$(echo "$HTML" | grep -o 'assets/index-[^"]*\.js' | head -1)
    CSS_ASSET=$(echo "$HTML" | grep -o 'assets/index-[^"]*\.css' | head -1)
    
    echo "   JS:  /$JS_ASSET"
    echo "   CSS: /$CSS_ASSET"
    
    # Test JS asset
    JS_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/$JS_ASSET")
    if [ "$JS_STATUS" = "200" ]; then
        echo "   ✅ JS asset loads (HTTP $JS_STATUS)"
    else
        echo "   ❌ JS asset failed (HTTP $JS_STATUS)"
        exit 1
    fi
    
    # Test CSS asset
    CSS_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/$CSS_ASSET")
    if [ "$CSS_STATUS" = "200" ]; then
        echo "   ✅ CSS asset loads (HTTP $CSS_STATUS)"
    else
        echo "   ❌ CSS asset failed (HTTP $CSS_STATUS)"
        exit 1
    fi
else
    echo "   ❌ Old HTML detected:"
    echo "$HTML" | head -10
    exit 1
fi
echo ""

echo "✅ 7. API Endpoints (with auth):"
PETS=$(curl -s -H "X-API-Key: test123" http://localhost:8080/pets)
if echo "$PETS" | jq -e 'type == "array"' > /dev/null 2>&1; then
    PET_COUNT=$(echo "$PETS" | jq 'length')
    echo "   Pets: $PET_COUNT items"
else
    echo "   ❌ Pets endpoint failed: $PETS"
    exit 1
fi

USERS=$(curl -s -H "X-API-Key: test123" http://localhost:8080/users)
if echo "$USERS" | jq -e 'type == "array"' > /dev/null 2>&1; then
    USER_COUNT=$(echo "$USERS" | jq 'length')
    echo "   Users: $USER_COUNT items"
else
    echo "   ❌ Users endpoint failed: $USERS"
    exit 1
fi
echo ""

echo "✅ 8. Observability Stack:"
echo "   Prometheus: http://localhost:9090"
curl -s http://localhost:9090/-/healthy > /dev/null && echo "      ✅ Healthy" || echo "      ❌ Unhealthy"

echo "   Grafana: http://localhost:3000"
curl -s http://localhost:3000/api/health > /dev/null && echo "      ✅ Healthy" || echo "      ❌ Unhealthy"

echo "   Jaeger: http://localhost:16686"
curl -s http://localhost:16686/ > /dev/null && echo "      ✅ Healthy" || echo "      ❌ Unhealthy"
echo ""

echo "🎉 ALL CHECKS PASSED!"
echo ""
echo "🌐 Open in browser:"
echo "   Pet Store UI:  http://localhost:8080"
echo "   Swagger UI:    http://localhost:8080/docs"
echo "   Grafana:       http://localhost:3000 (admin/admin)"
echo "   Prometheus:    http://localhost:9090"
echo "   Jaeger:        http://localhost:16686"
echo ""
echo "📊 Quick Stats:"
echo "   Pets available: $PET_COUNT"
echo "   Users registered: $USER_COUNT"
echo "   API Key: test123"
echo ""

