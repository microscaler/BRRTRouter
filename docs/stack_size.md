# Per-Handler Stack Size Configuration

BRRTRouter automatically computes optimal stack sizes for each handler's coroutine based on the OpenAPI specification. This prevents stack overflows for complex handlers while avoiding memory waste for simple ones.

## Overview

Each handler coroutine gets a tailored stack size based on:
- Number of path/query/header parameters
- Request/response schema complexity and nesting depth
- SSE/streaming endpoint requirements
- OpenAPI vendor extension overrides
- Runtime environment variable overrides

## Heuristic Computation

The default stack size is computed using these rules:

### Base Configuration
- **Base stack size**: 16 KiB
- **Minimum stack size**: 16 KiB (configurable via `BRRTR_STACK_MIN_BYTES`)
- **Maximum stack size**: 256 KiB (configurable via `BRRTR_STACK_MAX_BYTES`)

### Parameter-Based Sizing
- **+4 KiB** for every 5 path/query/header parameters
- Cookie parameters are not counted (handled differently)

### Schema Depth-Based Sizing
- **+4 KiB** for schemas with depth > 6
- **+16 KiB** for schemas with depth > 12

Schema depth is computed by analyzing:
- Object property nesting
- Array item schemas
- `allOf`, `anyOf`, `oneOf` compositions
- Referenced schemas (`$ref`)

### Streaming Endpoint Sizing
- **+8 KiB** for Server-Sent Events (SSE) endpoints
- Detected via `x-sse` or `sse` vendor extensions

### Examples

**Simple handler** (no parameters, shallow schema):
```
Base: 16 KiB
Total: 16 KiB
```

**Moderate handler** (7 parameters, depth 4 schema):
```
Base: 16 KiB
Parameters: +8 KiB (ceiling(7/5) * 4 KiB)
Total: 24 KiB
```

**Complex handler** (15 parameters, depth 8 schema, SSE):
```
Base: 16 KiB
Parameters: +12 KiB (ceiling(15/5) * 4 KiB)
Schema depth: +4 KiB (depth > 6)
SSE: +8 KiB
Total: 40 KiB
```

## Configuration Options

Stack sizes can be configured at multiple levels with this precedence (highest to lowest):

### 1. OpenAPI Vendor Extension (Design-Time)

Add `x-brrtrouter-stack-size` to any operation in your OpenAPI spec:

```yaml
paths:
  /complex-endpoint:
    post:
      operationId: complex_handler
      x-brrtrouter-stack-size: 65536  # 64 KiB
      requestBody:
        content:
          application/json:
            schema:
              # ... complex schema
```

This takes absolute precedence and is baked into the generated code.

### 2. Per-Handler Environment Variable (Runtime)

Override the stack size for a specific handler at runtime:

```bash
# Set stack size for 'list_pets' handler to 32 KiB
export BRRTR_STACK_SIZE__LIST_PETS=32768

# Hex format also supported
export BRRTR_STACK_SIZE__LIST_PETS=0x8000
```

Note: Handler names are uppercased and use double underscores as separator.

### 3. Global Environment Variable (Runtime)

Override the default stack size for all handlers:

```bash
# Set global default to 32 KiB
export BRRTR_STACK_SIZE=32768

# Hex format also supported
export BRRTR_STACK_SIZE=0x8000
```

### 4. Clamping Range (Runtime)

Adjust the minimum and maximum allowed stack sizes:

```bash
# Set minimum to 32 KiB
export BRRTR_STACK_MIN_BYTES=32768

# Set maximum to 512 KiB
export BRRTR_STACK_MAX_BYTES=524288
```

All computed and overridden stack sizes are clamped to this range.

## Generated Code

The code generator automatically computes and emits stack sizes. For example, `registry.rs`:

```rust
pub unsafe fn register_all(dispatcher: &mut Dispatcher) {
    dispatcher.register_typed_with_stack_size(
        "list_pets",
        crate::controllers::list_pets::ListPetsController,
        20480,  // 20 KiB - computed from 3 query params
    );
    
    dispatcher.register_typed_with_stack_size(
        "stream_events",
        crate::controllers::stream_events::StreamEventsController,
        24576,  // 24 KiB - base + SSE bonus
    );
}
```

## Debugging Stack Issues

### Stack Overflow

If you encounter stack overflows (panics mentioning stack):

1. **Increase per-handler stack size**:
   ```bash
   export BRRTR_STACK_SIZE__YOUR_HANDLER=131072  # 128 KiB
   ```

2. **Add vendor extension** to OpenAPI spec:
   ```yaml
   x-brrtrouter-stack-size: 131072
   ```
   Then regenerate code.

3. **Increase global maximum**:
   ```bash
   export BRRTR_STACK_MAX_BYTES=524288  # 512 KiB
   ```

### Memory Usage

If you're concerned about memory usage:

1. **Reduce maximum stack size**:
   ```bash
   export BRRTR_STACK_MAX_BYTES=65536  # 64 KiB
   ```

2. **Monitor stack usage** (requires `stack_usage` feature):
   ```bash
   cargo build --features stack_usage
   ```

3. **Review generated stack sizes** in `src/registry.rs`

## Best Practices

1. **Start with defaults**: The heuristic computation handles most cases well.

2. **Use vendor extensions** for known complex handlers rather than runtime overrides.

3. **Profile before tuning**: Only adjust stack sizes if you observe issues.

4. **Consider request size**: Handlers with large JSON payloads may need more stack.

5. **Watch for recursion**: Deeply recursive logic needs larger stacks.

6. **Test with production data**: Stack requirements depend on actual request/response sizes.

## Implementation Details

### Code Generation

Stack sizes are computed during code generation in `src/generator/stack_size.rs`:

```rust
use brrtrouter::generator::compute_stack_size;

let stack_size_bytes = compute_stack_size(&route_meta);
```

### Runtime

Handlers are spawned with computed stack sizes in `src/typed/core.rs`:

```rust
unsafe fn spawn_typed_with_stack_size<H>(
    handler: H,
    stack_size_bytes: usize,
) -> mpsc::Sender<HandlerRequest>
where
    H: Handler + Send + 'static,
{
    may::coroutine::Builder::new()
        .stack_size(stack_size_bytes)
        .spawn(move || {
            // handler logic
        })
}
```

Environment variable overrides are applied via `get_stack_size_with_overrides()`.

## Testing

The stack size computation includes comprehensive tests:

```bash
# Run stack size tests
cargo test --lib generator::stack_size

# Run environment variable override tests
cargo test --lib typed::core::tests
```

## Migration Guide

### From Global Stack Size

If you previously used `BRRTR_STACK_SIZE` globally:

1. **No changes needed**: Global override still works as fallback.

2. **For better performance**: Remove global override and let heuristics compute per-handler sizes.

3. **For specific handlers**: Use per-handler overrides:
   ```bash
   # Old: Global override
   export BRRTR_STACK_SIZE=65536
   
   # New: Per-handler overrides
   export BRRTR_STACK_SIZE__COMPLEX_HANDLER=65536
   # Others use computed defaults
   ```

### Adding to Existing Projects

1. **Regenerate code** with latest BRRTRouter:
   ```bash
   cargo run --bin brrtrouter-gen -- generate \
       --spec openapi.yaml \
       --output my-service \
       --force
   ```

2. **Review generated stack sizes** in `src/registry.rs`

3. **Test your handlers** to ensure no stack overflows

4. **Adjust if needed** using environment variables or vendor extensions

## FAQ

**Q: Why not use a single large stack size for all handlers?**  
A: Memory efficiency. With many handlers, fixed large stacks waste memory. Per-handler sizing optimizes both safety and memory usage.

**Q: Can stack sizes change between deployments?**  
A: Yes, if you use environment variables. Vendor extensions in the OpenAPI spec create fixed sizes in generated code.

**Q: What happens if I exceed the stack size?**  
A: The coroutine will panic with a stack overflow error. The panic is caught and converted to a 500 error response.

**Q: Do stack sizes affect performance?**  
A: Minimal impact. Smaller stacks reduce memory pressure and improve cache locality. Larger stacks enable deeper recursion but use more memory.

**Q: How do I know what stack size my handler needs?**  
A: Start with computed defaults. If you see stack overflows, increase gradually. Monitor with `stack_usage` feature if needed.

**Q: Can I disable per-handler stack sizing?**  
A: Yes, set a global `BRRTR_STACK_SIZE` which overrides all computed sizes (but vendor extensions still take precedence).
