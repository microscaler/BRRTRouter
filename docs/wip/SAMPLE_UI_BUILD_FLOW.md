# Sample UI Build & Deployment Flow

## 📦 Complete Build Pipeline

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. EDIT SOURCE                                                  │
│    sample-ui/src/App.jsx                                        │
│    sample-ui/src/components/*.jsx                               │
│    sample-ui/src/index.css (Tailwind)                           │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. TILT DETECTS CHANGE                                          │
│    Watches: sample-ui/src/, tailwind.config.js, etc.           │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. YARN BUILD (local_resource: build-sample-ui)                │
│    cd sample-ui && yarn install && yarn build:copy             │
│                                                                 │
│    ├─ yarn install          → Installs deps (if needed)        │
│    ├─ vite build            → Builds SolidJS + Tailwind        │
│    │   ├─ PostCSS processes Tailwind                           │
│    │   ├─ PurgeCSS removes unused classes                      │
│    │   ├─ SolidJS compiles to optimized JS                     │
│    │   └─ Vite bundles & minifies                              │
│    └─ Output: sample-ui/dist/                                  │
│        ├─ index.html                                            │
│        └─ assets/                                               │
│            ├─ index-[hash].js   (~20-30KB gzipped)             │
│            └─ index-[hash].css  (~5-10KB with Tailwind purge)  │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. COPY TO PET STORE (yarn copy)                               │
│    node sample-ui/scripts/copy-to-petstore.js                  │
│                                                                 │
│    ├─ Cleans examples/pet_store/static_site/ (except dummy.txt)│
│    ├─ Copies sample-ui/dist/* → pet_store/static_site/         │
│    └─ Result:                                                   │
│        examples/pet_store/static_site/                          │
│        ├─ index.html                                            │
│        └─ assets/                                               │
│            ├─ index-[hash].js                                   │
│            └─ index-[hash].css                                  │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. DOCKER BUILD (custom_build: brrtrouter-petstore)           │
│    dockerfiles/Dockerfile.dev:                                   │
│                                                                 │
│    COPY ./examples/pet_store/static_site /app/static_site      │
│                                                                 │
│    └─ Copies built files into container at /app/static_site    │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ 6. TILT LIVE UPDATE (live_update)                              │
│    Syncs files to running container without rebuild:           │
│                                                                 │
│    sync('./examples/pet_store/static_site/', '/app/static_site/')│
│                                                                 │
│    └─ Fast incremental updates (~1 second)                     │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│ 7. PETSTORE SERVICE SERVES FILES                               │
│    BRRTRouter static file handler:                             │
│                                                                 │
│    GET /              → /app/static_site/index.html            │
│    GET /assets/*.js   → /app/static_site/assets/*.js           │
│    GET /assets/*.css  → /app/static_site/assets/*.css          │
│                                                                 │
│    └─ Accessible at http://localhost:8080                      │
└─────────────────────────────────────────────────────────────────┘
```

## ⏱️ Performance Breakdown

| Stage | Time | Notes |
|-------|------|-------|
| Tilt detects change | < 100ms | Filesystem watcher |
| Yarn install (cached) | < 500ms | Only if deps changed |
| Vite build (Tailwind) | 2-3s | SolidJS + Tailwind purge |
| Copy to pet_store | < 500ms | File operations |
| Tilt sync to container | ~1s | Live update |
| **Total (edit → deployed)** | **3-5s** | Full cycle |

## 🎯 Key Files in the Pipeline

### Source Files
```
sample-ui/
├── src/
│   ├── App.jsx              ← Your edits
│   ├── components/*.jsx     ← Your edits
│   └── index.css            ← Tailwind directives
├── tailwind.config.js       ← Theme config
├── postcss.config.js        ← PostCSS pipeline
└── vite.config.js           ← Build config
```

### Build Output (Gitignored)
```
sample-ui/dist/              ← Generated by Vite
├── index.html
└── assets/
    ├── index-abc123.js      ← Hashed for cache busting
    └── index-abc123.css
```

### Deployment Target
```
examples/pet_store/static_site/  ← Copy destination
├── index.html
└── assets/
    ├── index-abc123.js
    └── index-abc123.css
```

### Container
```
/app/static_site/            ← Inside container
├── index.html
└── assets/
    ├── index-abc123.js
    └── index-abc123.css
```

## 🔧 Configuration Files

### .dockerignore
```
# Sample UI source (we only need the built output)
sample-ui/src/
sample-ui/node_modules/
sample-ui/dist/
sample-ui/scripts/
```
**Why**: Reduces Docker build context. We only need the final output in `examples/pet_store/static_site/`.

### .gitignore
```
# Sample UI build artifacts
sample-ui/node_modules/
sample-ui/dist/
sample-ui/yarn-error.log
sample-ui/.yarn/
```
**Why**: Build artifacts are ephemeral and regenerated on each build.

### Tiltfile
```python
local_resource(
    'build-sample-ui',
    'cd sample-ui && yarn install && yarn build:copy',
    deps=[
        'sample-ui/src/',
        'sample-ui/index.html',
        'sample-ui/vite.config.js',
        'sample-ui/tailwind.config.js',
        'sample-ui/postcss.config.js',
    ],
    labels=['ui'],
    allow_parallel=True,
)
```
**Why**: Triggers rebuild on source or config changes.

## 🔍 Debugging the Flow

### Check Build Output
```bash
# After yarn build:copy
ls -la sample-ui/dist/
ls -la examples/pet_store/static_site/

# Should see index.html and assets/ in both
```

### Check Container Files
```bash
# Verify files are in container
kubectl exec -n brrtrouter-dev deployment/petstore -- ls -la /app/static_site/
kubectl exec -n brrtrouter-dev deployment/petstore -- cat /app/static_site/index.html
```

### Check Tilt Logs
```bash
# In Tilt UI, click "build-sample-ui" to see build logs
# Look for:
# ✓ built in 2.5s
# ✓ Copied to examples/pet_store/static_site/
```

### Manual Build Test
```bash
cd sample-ui
yarn build:copy

# Should output:
# vite v7.x.x building for production...
# ✓ 4 modules transformed.
# dist/index.html   1.2 KB
# dist/assets/...   25 KB
# 📦 Copying built files to pet store...
# ✨ Done! Tilt will sync changes automatically.
```

## 🚫 What NOT to Do

### ❌ Don't Edit Generated Files
```bash
# WRONG: Editing these will be overwritten
vim examples/pet_store/static_site/index.html
vim examples/pet_store/static_site/assets/*.js
```

### ✅ Instead: Edit Source Files
```bash
# CORRECT: Edit the source
vim sample-ui/src/App.jsx
# Let Tilt rebuild and copy
```

### ❌ Don't Manually Copy
```bash
# WRONG: Manual copying
cp sample-ui/dist/* examples/pet_store/static_site/
```

### ✅ Instead: Use the Script
```bash
# CORRECT: Use the copy script
cd sample-ui && yarn build:copy
# Or let Tilt handle it automatically
```

## 📋 Verification Checklist

After making UI changes:

- [ ] Edit `sample-ui/src/*.jsx`
- [ ] Tilt shows "build-sample-ui" building
- [ ] Tilt shows "build-sample-ui" success (green)
- [ ] Check `examples/pet_store/static_site/` has new files
- [ ] Tilt shows "petstore" syncing
- [ ] Refresh http://localhost:8080
- [ ] See your changes!

## 🎯 Common Issues

### Issue: "Could not resolve ./index.css"
**Cause**: Missing CSS file  
**Fix**: Created `src/index.css` with Tailwind directives

### Issue: Build output not appearing
**Cause**: Copy script not running  
**Fix**: Check `yarn build:copy` runs successfully

### Issue: Changes not showing in browser
**Cause**: Browser cache or Tilt not syncing  
**Fix**: Hard refresh (Cmd+Shift+R) or check Tilt logs

### Issue: "Module not found: solid-js"
**Cause**: Dependencies not installed  
**Fix**: `cd sample-ui && yarn install`

---

**Status**: ✅ Complete Build Pipeline  
**Performance**: 3-5 second iteration loop  
**Automation**: Fully automated via Tilt  
**Date**: October 9, 2025

