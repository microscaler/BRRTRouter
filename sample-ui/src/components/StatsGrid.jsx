function StatsGrid({ petCount, userCount, health, loading }) {
  return (
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6 mb-8">
      <div class={`stat-card ${loading() ? 'loading' : ''}`}>
        <h3 class="text-xs uppercase tracking-wider mb-2 opacity-90">Total Pets</h3>
        <div class="text-4xl font-bold mb-1">{loading() ? '...' : petCount()}</div>
        <div class="text-sm opacity-80">Available in store</div>
      </div>
      
      <div class={`stat-card ${loading() ? 'loading' : ''}`}>
        <h3 class="text-xs uppercase tracking-wider mb-2 opacity-90">Total Users</h3>
        <div class="text-4xl font-bold mb-1">{loading() ? '...' : userCount()}</div>
        <div class="text-sm opacity-80">Registered accounts</div>
      </div>
      
      <div class={`stat-card ${loading() ? 'loading' : ''}`}>
        <h3 class="text-xs uppercase tracking-wider mb-2 opacity-90">API Status</h3>
        <div class="text-4xl font-bold mb-1">{health().status === 'ok' ? '✓' : '✗'}</div>
        <div class="text-sm opacity-80">{health().status === 'ok' ? 'Healthy' : 'Error'}</div>
      </div>
      
      <div class={`stat-card ${loading() ? 'loading' : ''}`}>
        <h3 class="text-xs uppercase tracking-wider mb-2 opacity-90">Response Time</h3>
        <div class="text-4xl font-bold mb-1">{loading() ? '...' : health().responseTime}</div>
        <div class="text-sm opacity-80">milliseconds</div>
      </div>
    </div>
  );
}

export default StatsGrid;
