import { Show, For } from 'solid-js';

function PetsList({ pets, loading }) {
  return (
    <div class="bg-gray-50 rounded-xl p-6 shadow-sm">
      <h2 class="text-2xl font-bold text-primary-500 mb-6 flex items-center gap-2">
        <span>🐾</span> Pets Available
      </h2>
      
      <Show 
        when={!loading()}
        fallback={<div class="text-center py-10 text-gray-500">Loading pets...</div>}
      >
        <Show
          when={pets().length > 0}
          fallback={<div class="text-center py-10 text-gray-400 italic">No pets found</div>}
        >
          <ul class="space-y-4">
            <For each={pets()}>
              {(pet) => (
                <li class="item">
                  <div class="flex items-center justify-between">
                    <div>
                      <strong class="text-primary-500 text-lg">{pet.name}</strong>
                      <div class="text-gray-600 text-sm mt-1">
                        ID: {pet.id}
                      </div>
                    </div>
                    <span class={`px-3 py-1 rounded-full text-xs font-semibold uppercase
                      ${pet.status === 'available' 
                        ? 'bg-green-100 text-green-700' 
                        : 'bg-yellow-100 text-yellow-700'}`}>
                      {pet.status}
                    </span>
                  </div>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </Show>
    </div>
  );
}

export default PetsList;

