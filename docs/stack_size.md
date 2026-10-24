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

A handler's stack is resolved at runtime (`src/handler_stack.rs`), most specific first:

1. `BRRTR_STACK_SIZE__<HANDLER>` (environment, this handler)
2. `x-brrtrouter-stack-size` on the operation (spec, this handler)
3. `x-brrtrouter-stack-size` at the root of the spec (spec, every handler)
4. 256 KiB (`handler_stack::FALLBACK`), or the heuristic above if larger

then clamped to `BRRTR_STACK_MIN_BYTES`..`BRRTR_STACK_MAX_BYTES`. Sizes are bytes (since 24 Oct 2026; before, every
handler stack was 8x its setting because `may` reads words). The spec values are read from the spec the service
loads, so resizing a handler needs no code generation.

### 1. OpenAPI Vendor Extension

```yaml
x-brrtrouter-stack-size: 128KiB      # root: the default for every operation
paths:
  /reports/{id}:
    get:
      operationId: build_report
      x-brrtrouter-stack-size: 256KiB  # this operation
```

Values: bytes (`65536`), hex (`"0x10000"`), units (`"64KiB"`, `"64K"`, `"1MiB"`) or `small` / `medium` / `large`
(16 / 32 / 64 KiB).

### 2. Per-Handler Environment Variable (Runtime)

Override one handler without a release:

```bash
export BRRTR_STACK_SIZE__LIST_PETS=32768
export BRRTR_STACK_SIZE__LIST_PETS=0x8000
export BRRTR_STACK_SIZE__LIST_PETS=32KiB
```

Handler names are upper-cased after the double underscore.

### 3. `BRRTR_STACK_SIZE` is not a handler setting

`BRRTR_STACK_SIZE` sizes `may`'s own coroutines: the HTTP connection coroutines that parse, route, validate and
dispatch (`RuntimeConfig::stack_size`, in `may` words). It no longer applies to handlers: a value tuned for those
coroutines must not resize every handler, nor the reverse. Size handlers with the spec or the per-handler variable.

### 4. Clamping Range (Runtime)

```bash
export BRRTR_STACK_MIN_BYTES=32768    # default 16 KiB
export BRRTR_STACK_MAX_BYTES=524288   # default 256 KiB
```

Every handler size, from any source, is clamped to this range: a spec or per-handler value above 256 KiB needs
`BRRTR_STACK_MAX_BYTES` raised too.

### Measuring

With `BRRTR_STACK_PROBE_EVERY` (default 16; `1` measures every request) BRRTRouter records each handler's stack depth
from page residency: `brrtrouter_handler_stack_used_bytes{handler}` (histogram), `brrtrouter_handler_stack_peak_bytes`
and `brrtrouter_handler_stack_size_bytes` on `/metrics`. Size a handler at a few times its peak.

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

### From a Global `BRRTR_STACK_SIZE`

`BRRTR_STACK_SIZE` used to be the fallback for every handler. It now sizes only `may`'s connection coroutines, so
handlers that relied on it get the spec values or 256 KiB. To keep a large handler stack, set it where it belongs:

```yaml
x-brrtrouter-stack-size: 128KiB          # spec root, every handler
```
```bash
export BRRTR_STACK_SIZE__COMPLEX_HANDLER=262144   # one handler
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
A: Yes: the spec values are read at runtime from the spec the service loads, and the per-handler variables override them.

**Q: What happens if I exceed the stack size?**  
A: The coroutine will panic with a stack overflow error. The panic is caught and converted to a 500 error response.

**Q: Do stack sizes affect performance?**  
A: Minimal impact. Smaller stacks reduce memory pressure and improve cache locality. Larger stacks enable deeper recursion but use more memory.

**Q: How do I know what stack size my handler needs?**  
A: Read `brrtrouter_handler_stack_peak_bytes{handler}` (see Measuring) and give it a few times that.

**Q: Can I give every handler the same size?**  
A: Yes: `x-brrtrouter-stack-size` at the root of the spec. (`BRRTR_STACK_SIZE` does not do this; it sizes `may`'s connection coroutines.)
