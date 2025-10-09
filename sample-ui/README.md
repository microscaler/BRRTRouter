# BRRTRouter Sample UI

Rich SolidJS + Tailwind CSS dashboard for the BRRTRouter Pet Store API.

## 🚀 Quick Start

```bash
# Install dependencies
yarn install

# Start development server (dev mode with HMR)
yarn dev
# Visit http://localhost:5173

# Build for production (outputs to ../examples/pet_store/static_site/)
yarn build
```

## 📦 What This Does

1. **yarn dev** - Runs Vite dev server with hot reload on port 5173
2. **yarn build** - Compiles SolidJS + Tailwind and outputs directly to `../examples/pet_store/static_site/`
   - No copying needed - Vite builds to the target directory
   - Uses `--emptyOutDir` to clean before building

## 🔄 Integration with Tilt

When you edit files in `sample-ui/src/`:
- Tilt detects changes and runs `yarn build`
- Vite builds directly to `examples/pet_store/static_site/`
- Tilt syncs to container automatically (~1-2 seconds)
- Refresh http://localhost:8080 to see updates

## 🎯 Features

- **SolidJS** - Fine-grained reactivity, no virtual DOM
- **Tailwind CSS** - Utility-first styling with custom theme
- **Vite** - Lightning-fast builds and HMR
- **Yarn** - Fast, reliable package management
- **Live API Integration** - Real data from BRRTRouter API
- **Auto-refresh** - Updates every 30 seconds
- **Responsive Design** - Mobile-first with Tailwind breakpoints

## 📁 Structure

```
sample-ui/
├── src/
│   ├── components/     # UI components
│   ├── App.jsx        # Main app
│   ├── index.jsx      # Entry point
│   └── index.css      # Styles
├── scripts/
│   └── copy-to-petstore.js
├── dist/              # Build output (gitignored)
└── package.json
```

## 🛠️ Development Workflow

```bash
# Option 1: Develop in isolation
yarn dev
# Edit src/*, see changes instantly at http://localhost:5173

# Option 2: Deploy to Tilt
yarn build:copy
# Refresh http://localhost:8080
```

---

Built with ❤️ using SolidJS + Tailwind CSS + Vite + Yarn

