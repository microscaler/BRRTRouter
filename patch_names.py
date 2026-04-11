import re

# 1. Update typed/core.rs
with open("src/typed/core.rs", "r") as f:
    content = f.read()

# For spawn_typed_with_stack_size_and_name
content = re.sub(
    r'(let spawn_result = may::coroutine::Builder::new\(\)\s*\n\s*\.stack_size\([^)]+\))',
    r'\1\n        .name(effective_name.to_string())',
    content
)

with open("src/typed/core.rs", "w") as f:
    f.write(content)

# 2. Update dispatcher/core.rs
with open("src/dispatcher/core.rs", "r") as f:
    content = f.read()

# For register_handler
content = re.sub(
    r'(let spawn_result = may::coroutine::Builder::new\(\)\s*\n\s*\.stack_size\([^)]+\))',
    r'\1\n        .name(effective_name.to_string())',
    content
)

with open("src/dispatcher/core.rs", "w") as f:
    f.write(content)

# 3. Update worker_pool.rs
with open("src/worker_pool.rs", "r") as f:
    content = f.read()

content = re.sub(
    r'(let spawn_result = may::coroutine::Builder::new\(\)\s*\n\s*\.stack_size\([^)]+\))',
    r'\1\n                .name(format!("{}-worker-{}", handler_name_clone, worker_id))',
    content
)

with open("src/worker_pool.rs", "w") as f:
    f.write(content)

