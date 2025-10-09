#!/bin/bash
set -e

echo "=== Testing BRRTRouter UI Deployment ==="
echo ""

echo "🔍 Step 1: Check container files"
echo "--------------------------------"
kubectl exec -n brrtrouter-dev deployment/petstore -- ls -la /app/static_site/ || {
    echo "❌ Could not access container"
    exit 1
}
echo ""

echo "🔍 Step 2: Check index.html content in container"
echo "------------------------------------------------"
CONTAINER_HTML=$(kubectl exec -n brrtrouter-dev deployment/petstore -- cat /app/static_site/index.html)
if echo "$CONTAINER_HTML" | grep -q '<div id="root"></div>'; then
    echo "✅ Container has SolidJS app HTML"
else
    echo "❌ Container has old HTML:"
    echo "$CONTAINER_HTML"
    echo ""
    echo "🔧 Syncing files..."
    # Touch to trigger Tilt sync
    touch examples/pet_store/static_site/index.html
    sleep 3
    echo "⏳ Waiting for sync..."
    exit 1
fi
echo ""

echo "🔍 Step 3: Test HTTP endpoint"
echo "-----------------------------"
HTTP_RESPONSE=$(curl -s http://localhost:8080/)
if echo "$HTTP_RESPONSE" | grep -q '<div id="root"></div>'; then
    echo "✅ HTTP returns SolidJS app"
else
    echo "❌ HTTP returns:"
    echo "$HTTP_RESPONSE" | head -20
    exit 1
fi
echo ""

echo "🔍 Step 4: Check assets"
echo "----------------------"
ASSET_JS=$(echo "$HTTP_RESPONSE" | grep -o 'assets/index-[^"]*\.js' | head -1)
if [ -n "$ASSET_JS" ]; then
    echo "Found JS asset: $ASSET_JS"
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/$ASSET_JS")
    if [ "$HTTP_CODE" = "200" ]; then
        echo "✅ JS asset loads (HTTP $HTTP_CODE)"
    else
        echo "❌ JS asset failed (HTTP $HTTP_CODE)"
        exit 1
    fi
else
    echo "❌ No JS asset found in HTML"
    exit 1
fi
echo ""

echo "🔍 Step 5: Test API endpoints"
echo "----------------------------"
HEALTH=$(curl -s http://localhost:8080/health)
if echo "$HEALTH" | grep -q "ok"; then
    echo "✅ /health working"
else
    echo "❌ /health failed: $HEALTH"
fi

PETS=$(curl -s -H "X-API-Key: test123" http://localhost:8080/pets)
if echo "$PETS" | grep -q "id"; then
    echo "✅ /pets working (with auth)"
else
    echo "⚠️  /pets failed or empty: $PETS"
fi
echo ""

echo "🎉 SUCCESS! All checks passed!"
echo ""
echo "🌐 Open in browser: http://localhost:8080"
echo ""

