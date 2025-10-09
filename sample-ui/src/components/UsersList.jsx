import { Show, For } from 'solid-js';

function UsersList({ users, loading }) {
  return (
    <div class="bg-gray-50 rounded-xl p-6 shadow-sm">
      <h2 class="text-2xl font-bold text-primary-500 mb-6 flex items-center gap-2">
        <span>👥</span> User Directory
      </h2>
      
      <Show 
        when={!loading()}
        fallback={<div class="text-center py-10 text-gray-500">Loading users...</div>}
      >
        <Show
          when={users().length > 0}
          fallback={<div class="text-center py-10 text-gray-400 italic">No users found</div>}
        >
          <ul class="space-y-4">
            <For each={users()}>
              {(user) => (
                <li class="item">
                  <strong class="text-primary-500 text-lg block mb-1">{user.username}</strong>
                  <div class="text-gray-600 text-sm space-y-1">
                    <div>📧 {user.email}</div>
                    <div class="text-gray-500">ID: {user.id}</div>
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

export default UsersList;

