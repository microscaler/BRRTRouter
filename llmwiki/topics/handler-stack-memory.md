# Handler stacks and dispatch memory

What a handler costs in memory before and during a request, how it is measured, and the knobs. Source of the
numbers: Cloud Profiler heap profiles and `/proc/<pid>/smaps` of the PriceWhisperer / Sesame pp pods, 24 Oct 2026.

## Per-handler stack probe (`src/stack_probe.rs`)

- Coroutine stacks are anonymous `mmap` regions; a page is resident only once touched. The lowest resident page of
  the mapping is the handler's deepest point. `mincore(2)` reads it: one syscall, nothing painted, page (4 KiB)
  granularity. Linux only (no-op elsewhere).
- Every `BRRTR_STACK_PROBE_EVERY`-th request (default 16, `0` = off): before it, record the peak since the last
  sample and `madvise(MADV_DONTNEED)` the pages below the stack pointer; after it, record that request's depth.
  Side effect: a rare deep request no longer pins its stack pages for the life of the pod.
- `/metrics` (handlers with at least one sample only: 15 series each): `brrtrouter_handler_stack_used_bytes{handler}` (histogram, 4 KiB .. 4 MiB),
  `brrtrouter_handler_stack_peak_bytes{handler}`, `brrtrouter_handler_stack_size_bytes{handler}` (what is
  actually mapped). `brrtrouter_coroutine_stack_used_bytes` is now the maximum peak (it was a stub reading 0).
- Only the handler's own coroutine: work handed to other coroutines/threads (lifeguard pool workers) is not counted.
- Use: rank handlers by p99/peak, fix the deep paths, then size each with `BRRTR_STACK_SIZE__<HANDLER>`.

## Sizing handlers from the spec (`src/handler_stack.rs`)

Resolved at runtime from the spec the service loaded (no code generation needed), most specific first:

1. `BRRTR_STACK_SIZE__<HANDLER>` (env)
2. `x-brrtrouter-stack-size` on the operation (spec)
3. `x-brrtrouter-stack-size` at the spec root (spec-wide default)
4. `FALLBACK` (256 KiB), or the generator's estimate if larger

Clamped to `BRRTR_STACK_MIN_BYTES`..`BRRTR_STACK_MAX_BYTES` (16..256 KiB). Values: bytes, `0x` hex, `48KiB`/`48K`,
`1MiB`, `small`/`medium`/`large`. Sizes are **bytes** since 24 Oct 2026 (`handler_stack::words` converts for `may`);
before, every handler stack was 8x its setting. Applies to typed, untyped, `register_handler` and worker pools.

`BRRTR_STACK_SIZE` is **not** a handler setting: it is `may`'s own coroutine stack (`RuntimeConfig::stack_size`, still
in `may` words), i.e. the HTTP connection coroutines that route, validate and dispatch. They are not probed, and
PriceWhisperer's market wedge (30 Aug 2026) was a connection coroutine overflowing while building `may_queue`'s
64-slot block of unboxed `HandlerRequest`s on its stack (gone with the boxed channels, but unmeasured), so its value
is left alone.

## Coroutine names

Every handler coroutine is named after its handler (`register_handler` and `spawn_typed` used to spawn unnamed
coroutines, logged as `None`; unnamed typed handlers now take their type name, untyped ones `untyped`).
`register_typed` goes through `spawn_typed_with_stack_size_and_name`, so per-handler stack env vars apply to it.

## Stack units (fixed for handlers, 24 Oct 2026)

`generator::Stack::new(size)` allocates `size * size_of::<usize>()` bytes. Handler spawns pass
`handler_stack::words(bytes)`, so a handler maps what it is configured; the probe's peaks (24-48 KiB in pp) set the
fallback and the services' root defaults. `may`'s global stack (`BRRTR_STACK_SIZE`) is still interpreted as words.

## Coroutine pool

`may` keeps finished coroutines for reuse (default 1000), each with the stack pages it touched: an idle service held
~1000 stacks, ~37 MiB. `BRRTR_COROUTINE_POOL_CAPACITY` (default 100) via `RuntimeConfig::coroutine_pool_capacity`,
applied in `run_app` and the generated `main.rs`.

## Boxed handler channels

`HandlerSender` carries `Box<HandlerRequest>`. `may`'s mpsc allocates 64-slot blocks up front and a `HandlerRequest`
is ~3 KiB inline, so each handler's channel cost ~200-400 KiB idle (38 MB in trader-bff). `send(HandlerRequest)` is
unchanged and a plain `mpsc::Sender<HandlerRequest>` converts with `.into()` (`add_route` takes `impl Into`).

## Code anchors

- `src/stack_probe.rs`; probe hooks in `typed/core.rs` (both typed spawns), `dispatcher/core.rs`
  (`spawn_untyped_with_stack_size_and_name`, `register_handler`), `worker_pool.rs`.
- `HandlerSender` in `dispatcher/core.rs`; `RuntimeConfig::coroutine_pool_capacity` in `runtime_config.rs`.
