import { createSignal, createEffect, onCleanup } from 'solid-js';
import StatsGrid from './components/StatsGrid';
import PetsList from './components/PetsList';
import UsersList from './components/UsersList';
import QuickLinks from './components/QuickLinks';

const API_KEY = 'test123';
const API_BASE = window.location.origin;

function App() {
  const [pets, setPets] = createSignal([]);
  const [users, setUsers] = createSignal([]);
  const [health, setHealth] = createSignal({ status: 'unknown', responseTime: 0 });
  const [loading, setLoading] = createSignal(true);

  const fetchWithAuth = async (url) => {
    const start = performance.now();
    const response = await fetch(url, {
      headers: { 'X-API-Key': API_KEY, 'Accept': 'application/json' }
    });
    const elapsed = Math.round(performance.now() - start);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return { data: await response.json(), elapsed };
  };

  const loadData = async () => {
    try {
      const [petsResult, usersResult, healthResult] = await Promise.all([
        fetchWithAuth(`${API_BASE}/pets`),
        fetchWithAuth(`${API_BASE}/users`),
        fetch(`${API_BASE}/health`).then(r => r.json())
      ]);
      setPets(petsResult.data);
      setUsers(usersResult.data);
      setHealth({ status: healthResult.status || 'ok', responseTime: petsResult.elapsed });
      setLoading(false);
    } catch (error) {
      console.error('Load error:', error);
      setHealth({ status: 'error', responseTime: 0 });
      setLoading(false);
    }
  };

  createEffect(() => {
    loadData();
    const interval = setInterval(loadData, 30000);
    onCleanup(() => clearInterval(interval));
  });

  return (
    <div class="max-w-7xl mx-auto bg-white rounded-xl shadow-2xl p-8">
      <header class="text-center mb-8 pb-6 border-b-2 border-gray-100">
        <h1 class="text-5xl font-bold text-primary-500 mb-3">🐾 BRRTRouter Pet Store</h1>
        <p class="text-gray-600 text-lg">Live Dashboard - SolidJS + Vite + Tailwind</p>
      </header>
      
      <StatsGrid petCount={pets().length} userCount={users().length} health={health()} loading={loading()} />
      
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-8 mb-8">
        <PetsList pets={pets()} loading={loading()} />
        <UsersList users={users()} loading={loading()} />
      </div>
      
      <QuickLinks />
      
      <footer class="text-center pt-6 border-t-2 border-gray-100 text-gray-600 text-sm">
        <p class="mb-2">Built with ❤️ using BRRTRouter + SolidJS | Powered by Rust + OpenAPI 3.1</p>
        <p class="text-gray-500">🚀 Fast iteration with Tilt + kind</p>
      </footer>
    </div>
  );
}

export default App;

