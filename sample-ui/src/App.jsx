import { createSignal, createEffect, onCleanup, For, Show } from 'solid-js';

const API_KEY = 'test123';
const API_BASE = window.location.origin;
const MOCK_BEARER_TOKEN = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c';

function App() {
  const [pets, setPets] = createSignal([]);
  const [users, setUsers] = createSignal([]);
  const [events, setEvents] = createSignal([]);
  const [searchResults, setSearchResults] = createSignal(null);
  const [webhooks, setWebhooks] = createSignal([]);
  const [openApiSpec, setOpenApiSpec] = createSignal(null);
  const [loading, setLoading] = createSignal(true);
  const [selectedPet, setSelectedPet] = createSignal(null);
  const [selectedUser, setSelectedUser] = createSignal(null);
  const [showApiExplorer, setShowApiExplorer] = createSignal(false);
  const [apiTestResult, setApiTestResult] = createSignal(null);
  const [apiTestLoading, setApiTestLoading] = createSignal(false);
  const [showRequestBodyModal, setShowRequestBodyModal] = createSignal(false);
  const [pendingEndpoint, setPendingEndpoint] = createSignal(null);
  const [requestBody, setRequestBody] = createSignal('');
  const [showParamsModal, setShowParamsModal] = createSignal(false);
  const [paramValues, setParamValues] = createSignal({});
  const [showBearerModal, setShowBearerModal] = createSignal(false);
  const [bearerToken, setBearerToken] = createSignal(MOCK_BEARER_TOKEN);
  const [useBearerToken, setUseBearerToken] = createSignal(false);
  const [showMoneyTester, setShowMoneyTester] = createSignal(false);
  const [moneyTestResult, setMoneyTestResult] = createSignal(null);
  const [moneyTestLoading, setMoneyTestLoading] = createSignal(false);

  const loadData = async () => {
    console.log('Loading data from:', API_BASE);
    try {
      const [petsRes, usersRes, searchRes, specRes] = await Promise.all([
        fetch(`${API_BASE}/pets`, {
          headers: { 'X-API-Key': API_KEY }
        }),
        fetch(`${API_BASE}/users`, {
          headers: { 'X-API-Key': API_KEY }
        }),
        fetch(`${API_BASE}/search?q=test`, {
          headers: { 'X-API-Key': API_KEY }
        }).catch(() => ({ ok: false })),
        fetch(`${API_BASE}/openapi.yaml`).catch(() => ({ ok: false }))
      ]);
      
      const petsData = await petsRes.json();
      const usersData = await usersRes.json();
      
      setPets(petsData);
      setUsers(usersData.users || []);
      
      if (searchRes.ok) {
        const searchData = await searchRes.json();
        setSearchResults(searchData);
      }
      
      if (specRes.ok) {
        const specText = await specRes.text();
        setOpenApiSpec(specText);
      }
      
      setLoading(false);
      console.log('Loading complete!');
    } catch (error) {
      console.error('Load error:', error);
      setLoading(false);
    }
  };

  // Setup SSE for real-time events using fetch with auth headers
  createEffect(() => {
    let isActive = true;
    const receivedEvents = [];
    
    const connectSSE = async () => {
      try {
        const response = await fetch(`${API_BASE}/events`, {
          headers: { 'X-API-Key': API_KEY }
        });
        
        if (!response.ok) {
          console.log('SSE connection failed:', response.status);
          return;
        }
        
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        
        while (isActive) {
          const { done, value } = await reader.read();
          if (done) break;
          
          const chunk = decoder.decode(value);
          const lines = chunk.split('\n');
          
          for (const line of lines) {
            if (line.startsWith('data: ')) {
              const data = line.substring(6);
              try {
                const parsed = JSON.parse(data);
                receivedEvents.unshift({ ...parsed, timestamp: new Date().toLocaleTimeString() });
              } catch {
                receivedEvents.unshift({ message: data, timestamp: new Date().toLocaleTimeString() });
              }
              if (receivedEvents.length > 10) receivedEvents.pop();
              setEvents([...receivedEvents]);
            }
          }
        }
      } catch (error) {
        console.log('SSE error:', error);
      }
    };
    
    connectSSE();
    
    onCleanup(() => {
      isActive = false;
    });
  });

  createEffect(() => {
    loadData();
    const interval = setInterval(loadData, 30000);
    onCleanup(() => clearInterval(interval));
  });

  // Parse OpenAPI spec to extract paths
  const getApiPaths = () => {
    if (!openApiSpec()) return [];
    
    const spec = openApiSpec();
    const paths = [];
    const lines = spec.split('\n');
    let currentPath = null;
    let currentMethod = null;
    let inPaths = false;
    
    for (const line of lines) {
      if (line.trim() === 'paths:') {
        inPaths = true;
        continue;
      }
      
      if (inPaths && line.match(/^[a-z]/)) {
        inPaths = false;
      }
      
      if (inPaths) {
        const pathMatch = line.match(/^  (\/[^:]+):/);
        if (pathMatch) {
          currentPath = pathMatch[1];
        }
        
        const methodMatch = line.match(/^    (get|post|put|delete|patch):/);
        if (methodMatch && currentPath) {
          currentMethod = methodMatch[1].toUpperCase();
          paths.push({
            path: currentPath,
            method: currentMethod,
            fullPath: `${currentMethod} ${currentPath}`
          });
        }
      }
    }
    
    return paths;
  };

  // Prepare endpoint for testing (opens modal for POST/PUT/DELETE)
  const prepareEndpointTest = (path, method) => {
    if (method === 'GET') {
      testEndpoint(path, method);
    } else {
      // For POST/PUT/DELETE, show modal to enter request body
      setPendingEndpoint({ path, method });
      
      // Set example body based on path
      let exampleBody = '{}';
      if (path.includes('/pets')) {
        exampleBody = JSON.stringify({
          name: "Buddy",
          breed: "Golden Retriever", 
          age: 3,
          status: "available",
          vaccinated: true
        }, null, 2);
      } else if (path.includes('/users')) {
        exampleBody = JSON.stringify({
          username: "newuser",
          email: "user@example.com"
        }, null, 2);
      } else if (path.includes('/form')) {
        exampleBody = JSON.stringify({
          field1: "value1",
          field2: "value2"
        }, null, 2);
      } else if (path.includes('/upload')) {
        exampleBody = JSON.stringify({
          filename: "test.txt",
          content: "Hello World"
        }, null, 2);
      } else if (path.includes('/items') || path.includes('/payment')) {
        // Money/currency examples
        exampleBody = JSON.stringify({
          name: "Test Item",
          price: 3.14,
          currency_code: "USD"
        }, null, 2);
      }
      
      setRequestBody(exampleBody);
      setShowRequestBodyModal(true);
    }
  };

  // Test an API endpoint with authentication
  const testEndpoint = async (path, method = 'GET', body = null, params = null) => {
    // Handle path parameters
    let finalPath = path;
    if (path.includes('{') && !params) {
      // Need to collect parameters first - open modal
      const pathParams = path.match(/\{([^}]+)\}/g);
      if (pathParams) {
        const paramObj = {};
        for (const param of pathParams) {
          const paramName = param.slice(1, -1);
          // Set default values
          if (paramName === 'id' || paramName.endsWith('_id')) {
            paramObj[paramName] = '1';
          } else if (paramName === 'coords') {
            paramObj[paramName] = '1;2;3';
          } else if (paramName === 'color') {
            paramObj[paramName] = 'red';
          } else {
            paramObj[paramName] = '';
          }
        }
        setParamValues(paramObj);
        setPendingEndpoint({ path, method, body });
        setShowParamsModal(true);
        return;
      }
    }
    
    // Apply parameters
    if (params) {
      finalPath = path;
      for (const [key, value] of Object.entries(params)) {
        finalPath = finalPath.replace(`{${key}}`, value);
      }
    }
    
    setApiTestLoading(true);
    setApiTestResult(null);
    
    try {
      const startTime = performance.now();
      const options = {
        method: method,
        headers: { 'X-API-Key': API_KEY }
      };
      
      // Add Bearer token if enabled
      if (useBearerToken() && bearerToken()) {
        options.headers['Authorization'] = `Bearer ${bearerToken()}`;
      }
      
      if (body) {
        options.headers['Content-Type'] = 'application/json';
        options.body = body;
      }
      
      const response = await fetch(`${API_BASE}${finalPath}`, options);
      const endTime = performance.now();
      
      let data;
      const contentType = response.headers.get('content-type');
      if (contentType && contentType.includes('application/json')) {
        data = await response.json();
      } else {
        data = await response.text();
      }
      
      setApiTestResult({
        status: response.status,
        statusText: response.statusText,
        data: data,
        duration: Math.round(endTime - startTime),
        headers: Object.fromEntries(response.headers.entries()),
        path: finalPath,
        method: method,
        error: response.status === 401 ? 'Unauthorized - This endpoint requires additional authentication' : null
      });
      
      // Scroll to results
      setTimeout(() => {
        document.getElementById('api-test-results')?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }, 100);
    } catch (error) {
      setApiTestResult({
        error: error.message,
        path: finalPath,
        method: method
      });
      
      // Scroll to results
      setTimeout(() => {
        document.getElementById('api-test-results')?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }, 100);
    } finally {
      setApiTestLoading(false);
    }
  };

  return (
    <div class="min-h-screen bg-gradient-to-br from-indigo-50 via-purple-50 to-pink-50 p-8">
      <div class="max-w-7xl mx-auto">
        <header class="text-center mb-8 bg-white rounded-2xl shadow-lg p-8">
          <h1 class="text-6xl font-bold bg-gradient-to-r from-indigo-600 to-purple-600 bg-clip-text text-transparent mb-3">
            🐾 BRRTRouter Pet Store
          </h1>
          <p class="text-gray-600 text-lg">Live Dashboard - SolidJS + Vite + Tailwind CSS</p>
          <div class="mt-4 flex justify-center gap-4 flex-wrap">
            <button 
              class="px-6 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 transition shadow-md"
              onClick={() => setShowApiExplorer(true)}
            >
              📖 API Explorer ({getApiPaths().length} endpoints)
            </button>
            <button 
              class="px-6 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition shadow-md"
              onClick={() => setShowMoneyTester(true)}
            >
              💰 Money/Currency Tester
            </button>
            <a 
              href="/docs" 
              target="_blank"
              class="px-6 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition shadow-md inline-block"
            >
              📚 Swagger Docs
            </a>
          </div>
        </header>

        {loading() ? (
          <div class="text-center py-20">
            <div class="text-6xl mb-4">⏳</div>
            <p class="text-gray-600 text-xl">Loading...</p>
          </div>
        ) : (
          <div class="space-y-8">
            {/* Main Grid: Pets & Users */}
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
              {/* Pets */}
              <div class="bg-white rounded-xl p-6 shadow-lg hover:shadow-2xl transition-shadow">
                <h2 class="text-2xl font-bold text-indigo-600 mb-4">🐾 Pets ({pets().length})</h2>
                <div class="space-y-3">
                  <For each={pets()}>
                    {pet => (
                      <div 
                        class="bg-gradient-to-r from-indigo-50 to-purple-50 rounded-lg p-4 hover:shadow-md transition-shadow cursor-pointer"
                        onClick={() => setSelectedPet(pet)}
                      >
                        <div class="font-bold text-lg text-indigo-700">{pet.name}</div>
                        <div class="text-sm text-gray-600">
                          {pet.breed} • {pet.age} years {pet.vaccinated && '• 💉 Vaccinated'}
                        </div>
                        <Show when={pet.tags && pet.tags.length > 0}>
                          <div class="flex gap-1 mt-2 flex-wrap">
                            <For each={pet.tags}>
                              {tag => (
                                <span class="px-2 py-0.5 bg-indigo-100 text-indigo-700 text-xs rounded-full">
                                  {tag}
                                </span>
                              )}
                            </For>
                          </div>
                        </Show>
                      </div>
                    )}
                  </For>
                </div>
              </div>

              {/* Users */}
              <div class="bg-white rounded-xl p-6 shadow-lg hover:shadow-2xl transition-shadow">
                <h2 class="text-2xl font-bold text-purple-600 mb-4">👥 Users ({users().length})</h2>
                <div class="space-y-3">
                  <For each={users()}>
                    {user => (
                      <div 
                        class="bg-gradient-to-r from-purple-50 to-pink-50 rounded-lg p-4 hover:shadow-md transition-shadow cursor-pointer"
                        onClick={() => setSelectedUser(user)}
                      >
                        <div class="font-bold text-lg text-purple-700">{user.username}</div>
                        <div class="text-sm text-gray-600">📧 {user.email}</div>
                        <div class="text-xs text-gray-500 mt-1">ID: {user.id}</div>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </div>

            {/* Additional Data */}
            <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
              {/* Real-time Events (SSE) */}
              <Show when={events().length > 0}>
                <div class="bg-white rounded-xl p-6 shadow-lg">
                  <h3 class="text-xl font-bold text-green-600 mb-3">
                    📡 Live Events ({events().length})
                    <span class="ml-2 inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse"></span>
                  </h3>
                  <div class="space-y-2 max-h-64 overflow-auto">
                    <For each={events()}>
                      {event => (
                        <div class="bg-green-50 rounded-lg p-3 text-sm border-l-4 border-green-500">
                          <div class="font-medium text-green-700">
                            {event.message || event.name || event.title || JSON.stringify(event)}
                          </div>
                          <div class="text-gray-500 text-xs mt-1">{event.timestamp}</div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </Show>

              {/* Search Results */}
              <Show when={searchResults()}>
                <div class="bg-white rounded-xl p-6 shadow-lg">
                  <h3 class="text-xl font-bold text-orange-600 mb-3">🔍 Search Results</h3>
                  <div class="bg-orange-50 rounded-lg p-3">
                    <pre class="text-xs text-gray-700 overflow-auto">{JSON.stringify(searchResults(), null, 2)}</pre>
                  </div>
                </div>
              </Show>

              {/* Webhooks */}
              <Show when={webhooks().length > 0}>
                <div class="bg-white rounded-xl p-6 shadow-lg">
                  <h3 class="text-xl font-bold text-blue-600 mb-3">🔗 Webhooks ({webhooks().length})</h3>
                  <div class="space-y-2">
                    <For each={webhooks()}>
                      {webhook => (
                        <div class="bg-blue-50 rounded-lg p-3 text-sm">
                          <div class="font-medium text-blue-700">{webhook.url || webhook.name}</div>
                          <div class="text-gray-600 text-xs">{webhook.event || 'Active'}</div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </Show>
            </div>

            {/* Selected Pet Modal */}
            <Show when={selectedPet()}>
              <div 
                class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
                onClick={() => setSelectedPet(null)}
              >
                <div class="bg-white rounded-2xl p-8 max-w-md w-full m-4 shadow-2xl" onClick={(e) => e.stopPropagation()}>
                  <h3 class="text-3xl font-bold text-indigo-600 mb-4">{selectedPet().name}</h3>
                  <div class="space-y-2 text-gray-700">
                    <p><strong>ID:</strong> {selectedPet().id}</p>
                    <p><strong>Breed:</strong> {selectedPet().breed}</p>
                    <p><strong>Age:</strong> {selectedPet().age} years</p>
                    <p><strong>Status:</strong> {selectedPet().status}</p>
                    <p><strong>Vaccinated:</strong> {selectedPet().vaccinated ? '✅ Yes' : '❌ No'}</p>
                    <Show when={selectedPet().weight}>
                      <p><strong>Weight:</strong> {selectedPet().weight} kg</p>
                    </Show>
                  </div>
                  <button 
                    class="mt-6 w-full bg-indigo-600 text-white py-2 px-4 rounded-lg hover:bg-indigo-700 transition"
                    onClick={() => setSelectedPet(null)}
                  >
                    Close
                  </button>
                </div>
              </div>
            </Show>

            {/* Selected User Modal */}
            <Show when={selectedUser()}>
              <div 
                class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
                onClick={() => setSelectedUser(null)}
              >
                <div class="bg-white rounded-2xl p-8 max-w-md w-full m-4 shadow-2xl" onClick={(e) => e.stopPropagation()}>
                  <h3 class="text-3xl font-bold text-purple-600 mb-4">{selectedUser().username}</h3>
                  <div class="space-y-2 text-gray-700">
                    <p><strong>ID:</strong> {selectedUser().id}</p>
                    <p><strong>Email:</strong> {selectedUser().email}</p>
                    <Show when={selectedUser().role}>
                      <p><strong>Role:</strong> {selectedUser().role}</p>
                    </Show>
                  </div>
                  <button 
                    class="mt-6 w-full bg-purple-600 text-white py-2 px-4 rounded-lg hover:bg-purple-700 transition"
                    onClick={() => setSelectedUser(null)}
                  >
                    Close
                  </button>
                </div>
              </div>
            </Show>

            {/* API Explorer Modal */}
            <Show when={showApiExplorer()}>
              <div 
                class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
                onClick={() => setShowApiExplorer(false)}
              >
                <div class="bg-white rounded-2xl p-8 max-w-4xl w-full max-h-[90vh] overflow-auto shadow-2xl" onClick={(e) => e.stopPropagation()}>
                  <div class="flex justify-between items-center mb-6">
                    <h3 class="text-3xl font-bold text-indigo-600">📖 API Explorer</h3>
                    <button 
                      class="text-gray-500 hover:text-gray-700 text-2xl"
                      onClick={() => setShowApiExplorer(false)}
                    >
                      ✕
                    </button>
                  </div>
                  
                  <div class="mb-6 p-4 bg-indigo-50 rounded-lg">
                    <p class="text-sm text-gray-700">
                      <strong>Total Endpoints:</strong> {getApiPaths().length}
                    </p>
                    <p class="text-sm text-gray-600 mt-2">
                      This API was automatically generated from the OpenAPI 3.1 specification using BRRTRouter.
                    </p>
                  </div>

                  {/* Two-column grid for endpoints */}
                  <div class="grid grid-cols-1 md:grid-cols-2 gap-3 mb-6">
                    <For each={getApiPaths()}>
                      {endpoint => (
                        <div class={`border rounded-lg p-3 hover:shadow-md transition ${
                          endpoint.path.includes('/secure') ? 'border-yellow-300 bg-yellow-50' : 'border-gray-200 bg-white'
                        }`}>
                          <div class="flex items-start gap-2 flex-col">
                            <div class="flex items-center gap-2 w-full">
                              <span class={`px-2 py-1 rounded font-bold text-xs ${
                                endpoint.method === 'GET' ? 'bg-green-100 text-green-700' :
                                endpoint.method === 'POST' ? 'bg-blue-100 text-blue-700' :
                                endpoint.method === 'PUT' ? 'bg-orange-100 text-orange-700' :
                                endpoint.method === 'DELETE' ? 'bg-red-100 text-red-700' :
                                'bg-gray-100 text-gray-700'
                              }`}>
                                {endpoint.method}
                              </span>
                              <code class="text-gray-700 font-mono text-xs flex-1 break-all">{endpoint.path}</code>
                              <Show when={endpoint.path.includes('/secure')}>
                                <button
                                  class={`px-2 py-0.5 text-xs rounded font-medium transition ${
                                    useBearerToken() 
                                      ? 'bg-green-200 text-green-800 hover:bg-green-300' 
                                      : 'bg-yellow-200 text-yellow-800 hover:bg-yellow-300'
                                  }`}
                                  title={useBearerToken() ? "Bearer Token Active - Click to edit" : "Requires Bearer Token - Click to configure"}
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    setShowBearerModal(true);
                                  }}
                                >
                                  🔐
                                </button>
                              </Show>
                            </div>
                            <button 
                              class={`w-full px-3 py-1.5 text-white text-xs rounded transition font-medium ${
                                apiTestLoading() 
                                  ? 'bg-gray-400 cursor-not-allowed' 
                                  : endpoint.method === 'GET' 
                                    ? 'bg-indigo-600 hover:bg-indigo-700 cursor-pointer'
                                    : endpoint.method === 'POST'
                                    ? 'bg-blue-600 hover:bg-blue-700 cursor-pointer'
                                    : endpoint.method === 'PUT'
                                    ? 'bg-orange-600 hover:bg-orange-700 cursor-pointer'
                                    : endpoint.method === 'DELETE'
                                    ? 'bg-red-600 hover:bg-red-700 cursor-pointer'
                                    : 'bg-gray-600 hover:bg-gray-700 cursor-pointer'
                              }`}
                              onClick={(e) => {
                                e.stopPropagation();
                                if (!apiTestLoading()) {
                                  prepareEndpointTest(endpoint.path, endpoint.method);
                                }
                              }}
                            >
                              {apiTestLoading() 
                                ? '⏳ Testing...' 
                                : endpoint.method === 'GET' && endpoint.path.includes('{')
                                ? '📝 Try it (with params)'
                                : endpoint.method === 'GET'
                                ? '🚀 Try it'
                                : `✏️ Try ${endpoint.method}`
                              }
                            </button>
                          </div>
                        </div>
                      )}
                    </For>
                  </div>

                  {/* API Test Results - Fixed at bottom */}
                  <Show when={apiTestResult()}>
                    <div id="api-test-results" class="border-t-4 border-indigo-600 bg-gradient-to-br from-indigo-50 to-purple-50 rounded-lg p-6 shadow-lg">
                      <div class="flex justify-between items-center mb-4">
                        <h4 class="font-bold text-indigo-700 text-lg flex items-center gap-2">
                          <span>🎯</span>
                          <span>
                            <span class={`px-2 py-1 rounded text-xs mr-2 ${
                              apiTestResult().method === 'GET' ? 'bg-green-100 text-green-700' :
                              apiTestResult().method === 'POST' ? 'bg-blue-100 text-blue-700' :
                              apiTestResult().method === 'PUT' ? 'bg-orange-100 text-orange-700' :
                              apiTestResult().method === 'DELETE' ? 'bg-red-100 text-red-700' :
                              'bg-gray-100 text-gray-700'
                            }`}>
                              {apiTestResult().method || 'GET'}
                            </span>
                            <code class="text-sm">{apiTestResult().path}</code>
                          </span>
                        </h4>
                        <button 
                          class="px-3 py-1 bg-red-100 text-red-700 hover:bg-red-200 rounded text-sm font-medium transition"
                          onClick={() => setApiTestResult(null)}
                        >
                          ✕ Clear
                        </button>
                      </div>
                      
                      <Show when={!apiTestResult().error}>
                        <div class="space-y-3">
                          {/* Status */}
                          <div class="flex items-center gap-2">
                            <span class={`px-3 py-1 rounded font-bold text-sm ${
                              apiTestResult().status >= 200 && apiTestResult().status < 300 
                                ? 'bg-green-100 text-green-700' 
                                : apiTestResult().status >= 400 
                                ? 'bg-red-100 text-red-700'
                                : 'bg-yellow-100 text-yellow-700'
                            }`}>
                              {apiTestResult().status} {apiTestResult().statusText}
                            </span>
                            <span class="text-xs text-gray-600">⚡ {apiTestResult().duration}ms</span>
                          </div>

                          {/* Response Headers */}
                          <div>
                            <h5 class="font-semibold text-gray-700 text-sm mb-2">Response Headers:</h5>
                            <div class="bg-gray-50 rounded p-3 max-h-32 overflow-auto">
                              <pre class="text-xs text-gray-700 font-mono">{JSON.stringify(apiTestResult().headers, null, 2)}</pre>
                            </div>
                          </div>

                          {/* Response Data */}
                          <div>
                            <h5 class="font-semibold text-gray-700 text-sm mb-2">Response Body:</h5>
                            <div class="bg-gray-50 rounded p-3 max-h-64 overflow-auto">
                              <pre class="text-xs text-gray-700 font-mono">{
                                typeof apiTestResult().data === 'string' 
                                  ? apiTestResult().data 
                                  : JSON.stringify(apiTestResult().data, null, 2)
                              }</pre>
                            </div>
                          </div>
                        </div>
                      </Show>

                      <Show when={apiTestResult().error}>
                        <div class="bg-red-50 rounded-lg p-4">
                          <p class="text-red-700 font-semibold">Error:</p>
                          <p class="text-red-600 text-sm mt-1">{apiTestResult().error}</p>
                          <Show when={apiTestResult().status === 401 && apiTestResult().path.includes('secure')}>
                            <div class="mt-3 p-3 bg-yellow-50 border border-yellow-200 rounded">
                              <p class="text-yellow-800 text-xs">
                                <strong>ℹ️ Note:</strong> We sent a mock Bearer token with this request. 
                                The <code>/secure</code> endpoint demonstrates BRRTRouter's support for advanced authentication schemes (JWT/OAuth2).
                                You can configure valid tokens in your production setup.
                              </p>
                              <details class="mt-2">
                                <summary class="cursor-pointer text-yellow-700 hover:text-yellow-900 text-xs font-semibold">
                                  Show Mock Token
                                </summary>
                                <code class="block mt-1 p-2 bg-yellow-100 rounded text-xs break-all">
                                  {MOCK_BEARER_TOKEN}
                                </code>
                              </details>
                            </div>
                          </Show>
                        </div>
                      </Show>
                    </div>
                  </Show>

                  <div class="mt-6 pt-6 border-t border-gray-200">
                    <h4 class="font-bold text-gray-700 mb-3">Full OpenAPI Spec:</h4>
                    <div class="bg-gray-50 rounded-lg p-4 max-h-64 overflow-auto">
                      <pre class="text-xs text-gray-700 font-mono">{openApiSpec()}</pre>
                    </div>
                  </div>

                  <button 
                    class="mt-6 w-full bg-indigo-600 text-white py-3 px-4 rounded-lg hover:bg-indigo-700 transition font-medium"
                    onClick={() => {
                      setShowApiExplorer(false);
                      setApiTestResult(null);
                    }}
                  >
                    Close Explorer
                  </button>
                </div>
              </div>
            </Show>

            {/* Request Body Modal */}
            <Show when={showRequestBodyModal()}>
              <div 
                class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-[60]"
                onClick={() => setShowRequestBodyModal(false)}
              >
                <div class="bg-white rounded-2xl p-6 max-w-2xl w-full m-4 shadow-2xl" onClick={(e) => e.stopPropagation()}>
                  <h3 class="text-2xl font-bold text-blue-600 mb-4">
                    {pendingEndpoint()?.method} Request Body
                  </h3>
                  <p class="text-sm text-gray-600 mb-4">
                    <code class="bg-gray-100 px-2 py-1 rounded">{pendingEndpoint()?.method} {pendingEndpoint()?.path}</code>
                  </p>
                  
                  <label class="block text-sm font-medium text-gray-700 mb-2">
                    Request Body (JSON):
                  </label>
                  <textarea
                    class="w-full h-64 p-3 border border-gray-300 rounded-lg font-mono text-sm focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    value={requestBody()}
                    onInput={(e) => setRequestBody(e.target.value)}
                    placeholder='{\n  "key": "value"\n}'
                  />
                  
                  <div class="flex gap-3 mt-4">
                    <button 
                      class="flex-1 bg-blue-600 text-white py-3 px-4 rounded-lg hover:bg-blue-700 transition font-medium"
                      onClick={() => {
                        setShowRequestBodyModal(false);
                        testEndpoint(pendingEndpoint().path, pendingEndpoint().method, requestBody());
                      }}
                    >
                      🚀 Send Request
                    </button>
                    <button 
                      class="flex-1 bg-gray-200 text-gray-700 py-3 px-4 rounded-lg hover:bg-gray-300 transition font-medium"
                      onClick={() => setShowRequestBodyModal(false)}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              </div>
            </Show>

            {/* Path Parameters Modal */}
            <Show when={showParamsModal()}>
              <div 
                class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-[60]"
                onClick={() => setShowParamsModal(false)}
              >
                <div class="bg-white rounded-2xl p-6 max-w-xl w-full m-4 shadow-2xl" onClick={(e) => e.stopPropagation()}>
                  <h3 class="text-2xl font-bold text-indigo-600 mb-4">
                    Enter Path Parameters
                  </h3>
                  <p class="text-sm text-gray-600 mb-4">
                    <code class="bg-gray-100 px-2 py-1 rounded">{pendingEndpoint()?.method} {pendingEndpoint()?.path}</code>
                  </p>
                  
                  <div class="space-y-4">
                    <For each={Object.entries(paramValues())}>
                      {([key, value]) => (
                        <div>
                          <label class="block text-sm font-medium text-gray-700 mb-1">
                            {key}:
                          </label>
                          <input
                            type="text"
                            class="w-full p-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
                            value={value}
                            onInput={(e) => setParamValues({...paramValues(), [key]: e.target.value})}
                            placeholder={`Enter ${key}`}
                          />
                        </div>
                      )}
                    </For>
                  </div>
                  
                  <div class="flex gap-3 mt-6">
                    <button 
                      class="flex-1 bg-indigo-600 text-white py-3 px-4 rounded-lg hover:bg-indigo-700 transition font-medium"
                      onClick={() => {
                        setShowParamsModal(false);
                        testEndpoint(pendingEndpoint().path, pendingEndpoint().method, pendingEndpoint().body, paramValues());
                      }}
                    >
                      🚀 Continue
                    </button>
                    <button 
                      class="flex-1 bg-gray-200 text-gray-700 py-3 px-4 rounded-lg hover:bg-gray-300 transition font-medium"
                      onClick={() => setShowParamsModal(false)}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              </div>
            </Show>

            {/* Money/Currency Tester Modal */}
            <Show when={showMoneyTester()}>
              <div 
                class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-[60] p-4"
                onClick={() => setShowMoneyTester(false)}
              >
                <div class="bg-white rounded-2xl p-8 max-w-4xl w-full max-h-[90vh] overflow-auto shadow-2xl" onClick={(e) => e.stopPropagation()}>
                  <div class="flex justify-between items-center mb-6">
                    <h3 class="text-3xl font-bold text-green-600 flex items-center gap-2">
                      <span>💰</span>
                      <span>Money/Currency Type Tester</span>
                    </h3>
                    <button 
                      class="text-gray-500 hover:text-gray-700 text-2xl"
                      onClick={() => setShowMoneyTester(false)}
                    >
                      ✕
                    </button>
                  </div>
                  
                  <div class="mb-6 p-4 bg-green-50 rounded-lg">
                    <p class="text-sm text-gray-700 mb-2">
                      <strong>Testing Money Types:</strong> This section tests BRRTRouter's format-based type detection for financial amounts.
                    </p>
                    <p class="text-sm text-gray-600">
                      • <code>format: money</code> → <code>rusty_money::Money</code> (e.g., $3.14 USD)<br/>
                      • <code>format: decimal</code> → <code>rust_decimal::Decimal</code> (e.g., 0.08 for 8% tax rate)<br/>
                      • <code>number</code> (no format) → <code>f64</code> (mathematical numbers)
                    </p>
                  </div>

                  <div class="space-y-6">
                    {/* Payment Test */}
                    <div class="border-2 border-green-200 rounded-lg p-6 bg-gradient-to-br from-green-50 to-emerald-50">
                      <h4 class="text-xl font-bold text-green-700 mb-4 flex items-center gap-2">
                        <span>💳</span>
                        <span>Payment Test (format: money)</span>
                      </h4>
                      <p class="text-sm text-gray-600 mb-4">
                        Test money types with amount: <strong>3.14</strong> (to verify clippy fix - $3.14 USD = 314 cents)
                      </p>
                      
                      <div class="bg-white rounded-lg p-4 mb-4">
                        <p class="text-xs text-gray-600 mb-2 font-semibold">Example Request (format: money in OpenAPI):</p>
                        <pre class="text-xs text-gray-700 font-mono overflow-auto">{JSON.stringify({
                          name: "Test Item",
                          price: 3.14,  // format: money → rusty_money::Money
                          currency_code: "USD",
                          applied_amount: 3.14,  // format: money
                          tax_rate: 0.08,  // format: decimal → rust_decimal::Decimal
                          discount_percentage: 0.10  // format: decimal
                        }, null, 2)}</pre>
                        <p class="text-xs text-gray-500 mt-2 italic">
                          Note: The actual API endpoint may not include all these fields. This demonstrates the OpenAPI schema format.
                        </p>
                      </div>

                      <button 
                        class={`w-full px-4 py-3 rounded-lg font-medium transition ${
                          moneyTestLoading() 
                            ? 'bg-gray-400 text-white cursor-not-allowed' 
                            : 'bg-green-600 text-white hover:bg-green-700 cursor-pointer'
                        }`}
                        onClick={async () => {
                          setMoneyTestLoading(true);
                          setMoneyTestResult(null);
                          
                          try {
                            // Generate a UUID for the item ID
                            const itemId = crypto.randomUUID();
                            const paymentData = {
                              name: "Test Payment Item"
                            };
                            
                            // Use POST /items/{id} endpoint (requires ID in path)
                            const response = await fetch(`${API_BASE}/items/${itemId}`, {
                              method: 'POST',
                              headers: {
                                'X-API-Key': API_KEY,
                                'Content-Type': 'application/json'
                              },
                              body: JSON.stringify(paymentData)
                            });
                            
                            const data = await response.json();
                            
                            setMoneyTestResult({
                              success: response.ok,
                              status: response.status,
                              data: data,
                              message: response.ok 
                                ? `✅ Success! Item created/updated. Note: Money types (format: money) are handled by BRRTRouter's code generation.`
                                : `❌ Error: ${response.status} ${response.statusText}`,
                              testType: "Payment (format: money)",
                              requestData: paymentData,
                              endpoint: `/items/${itemId}`
                            });
                          } catch (error) {
                            setMoneyTestResult({
                              success: false,
                              error: error.message,
                              message: `❌ Request failed: ${error.message}`,
                              testType: "Payment (format: money)"
                            });
                          } finally {
                            setMoneyTestLoading(false);
                          }
                        }}
                        disabled={moneyTestLoading()}
                      >
                        {moneyTestLoading() ? '⏳ Testing...' : '🚀 Test Payment (3.14 USD)'}
                      </button>
                    </div>

                    {/* Currency Examples */}
                    <div class="border-2 border-blue-200 rounded-lg p-6 bg-gradient-to-br from-blue-50 to-cyan-50">
                      <h4 class="text-xl font-bold text-blue-700 mb-4 flex items-center gap-2">
                        <span>🌍</span>
                        <span>Multi-Currency Examples</span>
                      </h4>
                      <p class="text-sm text-gray-600 mb-4">
                        Test different currencies with the same amount (3.14):
                      </p>
                      
                      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                        {['USD', 'EUR', 'GBP', 'JPY'].map(currency => (
                          <div class="bg-white rounded-lg p-4 border border-blue-200">
                            <div class="flex items-center justify-between mb-2">
                              <span class="font-semibold text-gray-700">{currency}</span>
                              <span class="text-sm text-gray-500">3.14 {currency}</span>
                            </div>
                            <button 
                              class="w-full px-3 py-2 bg-blue-100 text-blue-700 rounded hover:bg-blue-200 transition text-sm font-medium"
                              onClick={async () => {
                                setMoneyTestLoading(true);
                                setMoneyTestResult(null);
                                
                                try {
                                  // Generate a UUID for the item ID
                                  const itemId = crypto.randomUUID();
                                  const itemData = {
                                    name: `Test Item (${currency})`
                                  };
                                  
                                  // Use POST /items/{id} endpoint (requires ID in path)
                                  const response = await fetch(`${API_BASE}/items/${itemId}`, {
                                    method: 'POST',
                                    headers: {
                                      'X-API-Key': API_KEY,
                                      'Content-Type': 'application/json'
                                    },
                                    body: JSON.stringify(itemData)
                                  });
                                  
                                  const data = await response.json();
                                  
                                  setMoneyTestResult({
                                    success: response.ok,
                                    status: response.status,
                                    data: data,
                                    message: response.ok 
                                      ? `✅ ${currency} test successful! Note: Money types (format: money) with amount 3.14 ${currency} would be handled by BRRTRouter's code generation.`
                                      : `❌ ${currency} test failed: ${response.status}`,
                                    testType: `Currency: ${currency}`,
                                    requestData: { ...itemData, price: 3.14, currency_code: currency },
                                    endpoint: `/items/${itemId}`
                                  });
                                } catch (error) {
                                  setMoneyTestResult({
                                    success: false,
                                    error: error.message,
                                    message: `❌ ${currency} test error: ${error.message}`,
                                    testType: `Currency: ${currency}`
                                  });
                                } finally {
                                  setMoneyTestLoading(false);
                                }
                              }}
                              disabled={moneyTestLoading()}
                            >
                              Test {currency}
                            </button>
                          </div>
                        ))}
                      </div>
                    </div>

                    {/* Test Results */}
                    <Show when={moneyTestResult()}>
                      <div class="border-2 border-indigo-200 rounded-lg p-6 bg-gradient-to-br from-indigo-50 to-purple-50">
                        <h4 class="text-xl font-bold text-indigo-700 mb-4 flex items-center gap-2">
                          <span>📊</span>
                          <span>Test Results</span>
                        </h4>
                        
                        <div class={`p-4 rounded-lg mb-4 ${
                          moneyTestResult().success ? 'bg-green-100 border border-green-300' : 'bg-red-100 border border-red-300'
                        }`}>
                          <p class={`font-semibold ${
                            moneyTestResult().success ? 'text-green-800' : 'text-red-800'
                          }`}>
                            {moneyTestResult().message}
                          </p>
                          <p class="text-sm text-gray-600 mt-2">
                            Test Type: <strong>{moneyTestResult().testType}</strong>
                          </p>
                          {moneyTestResult().status && (
                            <p class="text-sm text-gray-600">
                              Status: <strong>{moneyTestResult().status}</strong>
                            </p>
                          )}
                        </div>

                        <Show when={moneyTestResult().requestData}>
                          <div class="mb-4">
                            <h5 class="font-semibold text-gray-700 text-sm mb-2">Request Data (Example):</h5>
                            <div class="bg-blue-50 rounded p-3 max-h-48 overflow-auto border border-blue-200">
                              <pre class="text-xs text-gray-700 font-mono">{JSON.stringify(moneyTestResult().requestData, null, 2)}</pre>
                            </div>
                            {moneyTestResult().endpoint && (
                              <p class="text-xs text-gray-600 mt-2">
                                Endpoint: <code class="bg-gray-100 px-1 rounded">{moneyTestResult().endpoint}</code>
                              </p>
                            )}
                          </div>
                        </Show>

                        <Show when={moneyTestResult().data}>
                          <div>
                            <h5 class="font-semibold text-gray-700 text-sm mb-2">Response Data:</h5>
                            <div class="bg-white rounded p-3 max-h-64 overflow-auto border border-gray-200">
                              <pre class="text-xs text-gray-700 font-mono">{JSON.stringify(moneyTestResult().data, null, 2)}</pre>
                            </div>
                          </div>
                        </Show>

                        <Show when={moneyTestResult().error}>
                          <div class="bg-red-50 rounded p-3 border border-red-200">
                            <p class="text-sm text-red-700">
                              <strong>Error:</strong> {moneyTestResult().error}
                            </p>
                          </div>
                        </Show>
                      </div>
                    </Show>

                    {/* Info Section */}
                    <div class="border border-gray-200 rounded-lg p-4 bg-gray-50">
                      <h5 class="font-semibold text-gray-700 mb-2">ℹ️ About Money Types</h5>
                      <ul class="text-sm text-gray-600 space-y-1 list-disc list-inside">
                        <li><code>format: money</code> generates <code>rusty_money::Money</code> with currency support</li>
                        <li>Amount <strong>3.14</strong> is stored as <strong>314 cents</strong> (from_minor) to avoid clippy warnings</li>
                        <li><code>format: decimal</code> generates <code>rust_decimal::Decimal</code> for precise decimal math</li>
                        <li>Regular <code>number</code> (no format) uses <code>f64</code> for mathematical calculations</li>
                      </ul>
                    </div>
                  </div>

                  <button 
                    class="mt-6 w-full bg-green-600 text-white py-3 px-4 rounded-lg hover:bg-green-700 transition font-medium"
                    onClick={() => {
                      setShowMoneyTester(false);
                      setMoneyTestResult(null);
                    }}
                  >
                    Close Money Tester
                  </button>
                </div>
              </div>
            </Show>

            {/* Bearer Token Modal */}
            <Show when={showBearerModal()}>
              <div 
                class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-[60]"
                onClick={() => setShowBearerModal(false)}
              >
                <div class="bg-white rounded-2xl p-6 max-w-2xl w-full m-4 shadow-2xl" onClick={(e) => e.stopPropagation()}>
                  <h3 class="text-2xl font-bold text-yellow-600 mb-4 flex items-center gap-2">
                    <span>🔐</span>
                    <span>Bearer Token Configuration</span>
                  </h3>
                  <p class="text-sm text-gray-600 mb-4">
                    Configure a Bearer token (JWT) for endpoints that require advanced authentication.
                  </p>
                  
                  <div class="mb-4 p-4 bg-blue-50 rounded-lg">
                    <label class="flex items-center gap-2 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={useBearerToken()}
                        onChange={(e) => setUseBearerToken(e.target.checked)}
                        class="w-4 h-4 text-blue-600 rounded focus:ring-2 focus:ring-blue-500"
                      />
                      <span class="text-sm font-medium text-gray-700">
                        Enable Bearer Token Authentication
                      </span>
                    </label>
                  </div>
                  
                  <label class="block text-sm font-medium text-gray-700 mb-2">
                    Bearer Token (JWT):
                  </label>
                  <textarea
                    class="w-full h-40 p-3 border border-gray-300 rounded-lg font-mono text-xs focus:ring-2 focus:ring-yellow-500 focus:border-transparent"
                    value={bearerToken()}
                    onInput={(e) => setBearerToken(e.target.value)}
                    placeholder="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
                    disabled={!useBearerToken()}
                  />
                  
                  <div class="mt-3 text-xs text-gray-500">
                    <strong>Default Mock Token:</strong> Standard JWT.io example token (sub: "1234567890", name: "John Doe")
                  </div>
                  
                  <div class="flex gap-3 mt-6">
                    <button 
                      class="flex-1 bg-yellow-600 text-white py-3 px-4 rounded-lg hover:bg-yellow-700 transition font-medium"
                      onClick={() => setShowBearerModal(false)}
                    >
                      ✅ Save & Close
                    </button>
                    <button 
                      class="px-4 bg-gray-200 text-gray-700 py-3 rounded-lg hover:bg-gray-300 transition font-medium"
                      onClick={() => {
                        setBearerToken(MOCK_BEARER_TOKEN);
                        setUseBearerToken(false);
                      }}
                    >
                      Reset
                    </button>
                  </div>
                </div>
              </div>
            </Show>
          </div>
        )}

        <footer class="text-center mt-8 text-gray-600 text-sm">
          <p>Built with ❤️ using BRRTRouter + SolidJS | Powered by Rust + OpenAPI 3.1</p>
          <p class="text-gray-500 mt-1">🚀 Fast iteration with Tilt + kind | ✅ TooManyHeaders Fixed (32 headers)</p>
        </footer>
      </div>
    </div>
  );
}

export default App;
