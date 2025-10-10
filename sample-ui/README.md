# BRRTRouter Sample UI

A modern, responsive dashboard built with **SolidJS** and **Tailwind CSS** for the Pet Store example.

## ✨ Features

- **Real-time API Data**: Fetches live data from Pet Store API
- **Modern UI**: Built with SolidJS for reactive performance
- **Tailwind CSS**: Beautiful, responsive design
- **Fast Builds**: Vite for lightning-fast development

## 🏗️ Architecture

```
sample-ui/
├── src/
│   ├── App.jsx              # Main application component
│   ├── index.jsx            # Entry point
│   ├── index.css            # Global styles + Tailwind
│   └── components/
│       ├── StatsGrid.jsx    # API statistics cards
│       ├── PetsList.jsx     # Pets table
│       ├── UsersList.jsx    # Users table
│       └── QuickLinks.jsx   # Navigation links
├── index.html               # HTML template
├── vite.config.js           # Vite build config
├── tailwind.config.js       # Tailwind CSS config
├── postcss.config.js        # PostCSS config
└── package.json             # Dependencies & scripts
```

## 🚀 Development

### Local Development
```bash
cd sample-ui
npm install
npm run dev

# Open http://localhost:5173
```

### Build for Pet Store
```bash
# From sample-ui directory
npm run build:petstore

# Or from project root
just build-ui
```

This builds the UI and outputs to `examples/pet_store/static_site/`, where BRRTRouter serves it.

### Build for Production
```bash
npm run build
# Output: sample-ui/dist/
```

## 📦 Integration with Tilt

Tilt automatically builds the UI when you run `just dev-up`:

1. **`build-sample-ui` resource**: Runs `npm install && npm run build:petstore`
2. **Watches**: `src/`, `index.html`, `vite.config.js`, `tailwind.config.js`, `postcss.config.js`
3. **Triggers**: Docker rebuild when UI files change
4. **Live Update**: Syncs to `/app/static_site/` in the container

## 🎨 UI Components

### StatsGrid
Displays key metrics from the Pet Store API:
- Total Pets
- Total Users
- API Health Status
- Request Stats

### PetsList
Interactive table showing:
- Pet names, breeds, ages
- Owner information
- Microchip IDs

### UsersList
User directory with:
- User names and emails
- Roles and status
- Contact information

### QuickLinks
Navigation to:
- API Documentation (Swagger UI)
- Prometheus Metrics
- Grafana Dashboards
- Jaeger Traces

## 🔧 Configuration

### Vite Config (`vite.config.js`)
```javascript
export default defineConfig({
  plugins: [solid()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    open: false
  }
});
```

### Tailwind Config (`tailwind.config.js`)
```javascript
export default {
  content: [
    './index.html',
    './src/**/*.{js,jsx,ts,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        primary: '#4f46e5',
        secondary: '#10b981',
      },
    },
  },
  plugins: [],
};
```

## 📝 Package Scripts

| Script | Description |
|--------|-------------|
| `npm run dev` | Start dev server on http://localhost:5173 |
| `npm run build` | Build to `dist/` |
| `npm run build:petstore` | Build to `../examples/pet_store/static_site/` |
| `npm run preview` | Preview production build locally |

## 🌐 API Integration

The UI connects to the Pet Store API at:
- **Development**: `http://localhost:8080` (proxied by Tilt)
- **Production**: Same origin as the UI (served by BRRTRouter)

### API Endpoints Used
- `GET /pets` - Fetch all pets
- `GET /users` - Fetch all users
- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics

## 🎯 Key Files

### `src/App.jsx`
Main application component that:
- Fetches data from API
- Renders all UI components
- Handles error states
- Manages loading states

### `src/index.css`
Global styles including:
- Tailwind directives (`@tailwind base`, `@layer`, etc.)
- Custom CSS variables
- Global typography
- Animations

### `package.json`
Dependencies:
- **solid-js**: ^1.9.9 - Reactive UI framework
- **vite**: ^7.1.9 - Build tool
- **vite-plugin-solid**: ^2.11.9 - SolidJS support for Vite
- **tailwindcss**: ^3.4.1 - Utility-first CSS framework

## 🚨 Troubleshooting

### UI not updating in Tilt
```bash
# Force rebuild
just build-ui

# Check Tilt logs
tilt logs build-sample-ui
```

### Build errors
```bash
# Clear cache
rm -rf node_modules package-lock.json
npm install
npm run build:petstore
```

### Port 5173 in use
```bash
# Change port in vite.config.js
server: {
  port: 5174,  // or any available port
}
```

## 📚 Resources

- **SolidJS**: https://www.solidjs.com/
- **Tailwind CSS**: https://tailwindcss.com/
- **Vite**: https://vitejs.dev/
- **BRRTRouter**: https://github.com/microscaler/BRRTRouter

---

**Built with ❤️ for BRRTRouter**
