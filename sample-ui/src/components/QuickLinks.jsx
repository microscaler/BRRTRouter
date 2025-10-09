function QuickLinks() {
  const API_BASE = window.location.origin;
  
  const links = [
    { emoji: '📚', label: 'API Docs', url: `${API_BASE}/docs` },
    { emoji: '📊', label: 'Metrics', url: `${API_BASE}/metrics` },
    { emoji: '💚', label: 'Health', url: `${API_BASE}/health` },
    { emoji: '📋', label: 'OpenAPI Spec', url: `${API_BASE}/openapi.yaml` },
    { emoji: '📈', label: 'Grafana', url: 'http://localhost:3000' },
    { emoji: '🔍', label: 'Jaeger', url: 'http://localhost:16686' },
  ];
  
  return (
    <div class="bg-blue-50 rounded-xl p-6 mb-8">
      <h2 class="text-xl font-bold text-primary-500 mb-5">🔗 Quick Links</h2>
      <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-4">
        {links.map(link => (
          <a
            href={link.url}
            target="_blank"
            rel="noopener noreferrer"
            class="bg-white p-5 rounded-lg text-center text-primary-500 font-semibold 
                   shadow-sm hover:shadow-lg hover:-translate-y-1 hover:bg-primary-500 
                   hover:text-white transition-all duration-200"
          >
            <div class="text-3xl mb-2">{link.emoji}</div>
            <div class="text-sm">{link.label}</div>
          </a>
        ))}
      </div>
    </div>
  );
}

export default QuickLinks;

