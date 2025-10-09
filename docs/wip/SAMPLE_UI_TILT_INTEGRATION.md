# Sample UI + Tilt Integration

## 🎯 Overview

The `sample-ui/` SolidJS application is now fully integrated into Tilt's build pipeline. Changes to the UI automatically trigger rebuilds and syncs to the container.

## 🔄 How It Works

### Build Pipeline

```
┌─────────────────────────┐
│ Edit sample-ui/src/*.jsx│
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│ Tilt detects change     │
│ (watches sample-ui/src/)│
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│ Runs: build-sample-ui   │
│ - yarn install          │
│ - yarn build            │
│ - yarn copy             │
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│ Files copied to:        │
│ examples/pet_store/     │
│ static_site/            │
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│ Tilt detects static_    │
│ site changes            │
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│ Syncs to container      │
│ /app/static_site/       │
└────────────┬────────────┘
             │
             ▼
┌─────────────────────────┐
│ Refresh browser         │
│ http://localhost:8080   │
└─────────────────────────┘
```

### Tiltfile Configuration

```python
# 0. Build sample-ui (SolidJS) and copy to static site
local_resource(
    'build-sample-ui',
    'cd sample-ui && yarn install && yarn build:copy',
    deps=['sample-ui/src/', 'sample-ui/index.html', 'sample-ui/vite.config.js'],
    labels=['ui'],
    allow_parallel=True,
)
```

## 📊 Build Order

Tilt builds resources in this order:

1. **build-sample-ui** ⚡ (parallel)
2. **build-brrtrouter** ⚡ (parallel)
3. **gen-petstore** → depends on brrtrouter
4. **build-petstore** → depends on gen-petstore
5. **petstore deployment** → depends on build-petstore

## 🎯 What Triggers Rebuilds

### UI Changes
Watching:
- `sample-ui/src/**/*` - Any source file changes
- `sample-ui/index.html` - HTML entry point changes  
- `sample-ui/vite.config.js` - Build config changes

**Not** watching:
- `sample-ui/node_modules/` - Dependencies (gitignored)
- `sample-ui/dist/` - Build output (gitignored)
- `sample-ui/package.json` - Only triggers on manual tilt up

### Auto Actions
1. Change `sample-ui/src/App.jsx`
2. Tilt runs `yarn build:copy` (~2-5 seconds)
3. Files land in `examples/pet_store/static_site/`
4. Tilt syncs to container (~1 second)
5. **Total: ~3-6 seconds** from edit to deployed

## 🚀 Development Modes

### Mode 1: Full Tilt (Recommended for Integration Testing)

```bash
tilt up

# Edit sample-ui/src/App.jsx
# Tilt rebuilds automatically
# Refresh http://localhost:8080
```

**Pros:**
- Tests full integration
- Sees actual API data
- Production-like environment

**Cons:**
- Slower feedback (3-6 seconds)
- Requires full stack running

### Mode 2: Direct Vite Dev (Recommended for UI Development)

```bash
cd sample-ui
yarn dev

# Edit src/App.jsx
# Vite hot-reloads instantly
# View at http://localhost:5173
```

**Pros:**
- Instant feedback (< 100ms)
- Hot module replacement
- Fast iteration

**Cons:**
- May need to mock API responses
- Not production-like

### Mode 3: Hybrid (Best of Both Worlds)

```bash
# Terminal 1: Run Tilt for backend
tilt up

# Terminal 2: Run Vite for frontend  
cd sample-ui
yarn dev

# Edit UI at http://localhost:5173
# Test with API at http://localhost:8080
```

**Pros:**
- Fast UI iteration
- Real API available
- Cross-origin testing

## 📁 File Structure

```
sample-ui/
├── src/
│   ├── App.jsx              ← Edit here
│   ├── index.jsx
│   ├── index.css
│   └── components/
│       ├── StatsGrid.jsx    ← Edit here
│       ├── PetsList.jsx     ← Edit here
│       ├── UsersList.jsx    ← Edit here
│       └── QuickLinks.jsx   ← Edit here
├── dist/                    ← Build output (gitignored)
│   ├── index.html
│   └── assets/
│       ├── index.js
│       └── index.css
└── scripts/
    └── copy-to-petstore.js  ← Copies dist → pet_store
```

## 🔍 Debugging

### Check Tilt Build Status

```bash
# View Tilt UI
tilt up
# Press 'space' to open web UI

# Check build-sample-ui logs
# Click on "build-sample-ui" in Tilt UI
```

### Manual Build

```bash
cd sample-ui
yarn build:copy

# Check output
ls -la ../examples/pet_store/static_site/
```

### Check Container Files

```bash
kubectl exec -n brrtrouter-dev deployment/petstore -- ls -la /app/static_site/
```

## 🎨 UI Development Tips

### 1. Component-Driven Development

```bash
# Develop in isolation
cd sample-ui
yarn dev

# Create/edit components
vim src/components/NewComponent.jsx

# See changes instantly at localhost:5173
```

### 2. API Integration Testing

```bash
# Full stack
tilt up

# Make UI changes
vim sample-ui/src/App.jsx

# Wait ~5 seconds
# Refresh localhost:8080
```

### 3. Production Build Testing

```bash
# Build and deploy
cd sample-ui
yarn build:copy

# Check build output
ls -la dist/

# Test in Tilt
open http://localhost:8080
```

## ⚡ Performance

| Operation | Time | Notes |
|-----------|------|-------|
| Vite HMR (dev mode) | < 100ms | Instant feedback |
| Yarn build | 2-3s | Vite production build |
| Copy to pet_store | < 500ms | File copy operation |
| Tilt sync to container | ~1s | Live update |
| **Total (edit → deployed)** | **3-6s** | Full integration |

## 🛠️ Customization

### Change Watched Files

Edit `Tiltfile`:

```python
local_resource(
    'build-sample-ui',
    'cd sample-ui && yarn build:copy',
    deps=[
        'sample-ui/src/',
        'sample-ui/index.html',
        'sample-ui/vite.config.js',
        'sample-ui/package.json',  # Add this to watch package.json
    ],
    ...
)
```

### Skip UI Build

```bash
# Disable build-sample-ui temporarily
tilt up -- build-sample-ui=false
```

### Force Rebuild

```bash
# In Tilt UI, click "build-sample-ui" → "Force Update"
# Or restart Tilt:
tilt down && tilt up
```

## 🎯 Benefits

1. **Automatic** - No manual build step
2. **Fast** - 3-6 second feedback loop
3. **Integrated** - Part of full dev environment
4. **Parallel** - Builds while Rust compiles
5. **Reliable** - Yarn for dependency management
6. **Modern** - Vite for fast builds

## 🔮 Future Enhancements

- [ ] Add TypeScript for type safety
- [ ] Add Vitest for component testing
- [ ] Add hot reload without page refresh
- [ ] Add source maps for debugging
- [ ] Add bundle analysis
- [ ] Add CSS modules or Tailwind

---

**Status**: ✅ Fully Integrated
**Last Updated**: October 9, 2025
**Integration**: Tilt + Yarn + Vite + SolidJS

