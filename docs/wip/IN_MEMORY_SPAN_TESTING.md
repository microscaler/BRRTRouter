# In-Memory Span Testing - OpenTelemetry 0.30 Compatible

## Overview

Implemented a custom in-process span collector for testing that's fully compatible with OpenTelemetry 0.30, replacing the incompatible `fake-opentelemetry-collector` crate.

## Why This Approach?

### Problem
- `fake-opentelemetry-collector 0.28` uses OpenTelemetry 0.29
- Incompatible with our OpenTelemetry 0.30 upgrade
- No updated version available yet

### Solution: In-Process Span Collection
Instead of testing the OTLP wire protocol (which is OpenTelemetry's job), we test what matters for our application:
- ✅ Are spans created with correct names?
- ✅ Do spans have correct attributes?
- ✅ Are spans properly nested?
- ✅ Do errors get recorded?

## Implementation

### InMemorySpanProcessor
```rust
/// Custom SpanProcessor that collects spans in memory
struct InMemorySpanProcessor {
    spans: Arc<RwLock<Vec<SpanData>>>,
}

impl SpanProcessor for InMemorySpanProcessor {
    fn on_end(&self, span: SpanData) {
        self.spans.write().push(span);
    }
    // ... other methods
}
```

### TestTracing Utility
```rust
pub struct TestTracing {
    spans: Arc<RwLock<Vec<SpanData>>>,
    tracer_provider: TracerProvider,
}

impl TestTracing {
    pub fn init() -> Self { ... }
    pub fn spans(&self) -> Vec<SpanData> { ... }
    pub fn spans_named(&self, name: &str) -> Vec<SpanData> { ... }
    pub fn wait_for_span(&self, name: &str) { ... }
    pub fn clear_spans(&mut self) { ... }
}
```

## Usage in Tests

```rust
use crate::tracing_util::TestTracing;
use tracing::{info_span, info};

#[test]
fn test_my_feature() {
    let mut tracing = TestTracing::init();
    
    // Code that creates spans
    {
        let _span = info_span!("my_operation").entered();
        info!("doing work");
    }
    
    tracing.force_flush();
    
    // Assert on collected spans
    let spans = tracing.spans_named("my_operation");
    assert_eq!(spans.len(), 1);
    
    // Check span attributes
    let span = &spans[0];
    assert_eq!(span.name, "my_operation");
}
```

## Benefits

### ✅ Works Everywhere
- Local development (macOS, Linux, Windows)
- GitHub Actions (no Docker required)
- Tilt/K8s (unit tests before deploy)
- CI/CD pipelines (fast, reliable)

### ✅ Fast & Reliable
- No network overhead
- No port conflicts
- No timing issues
- Deterministic behavior

### ✅ Simple Debugging
- Spans collected in memory
- Easy to inspect in debugger
- Clear test failures

### ✅ Real OpenTelemetry SDK
- Uses actual `SpanProcessor` trait
- Compatible with `tracing-opentelemetry`
- Tests real span creation

## What We Test

### Unit Tests
- ✅ Span creation and naming
- ✅ Span attributes and context
- ✅ Span nesting and relationships
- ✅ Error recording

### Integration Tests  
Real OTLP testing happens in:
- ✅ Kubernetes deployment (real OTEL collector)
- ✅ Tilt development environment
- ✅ Production monitoring

## Comparison

| Aspect | In-Process Mock | fake-opentelemetry-collector | Real OTLP Container |
|--------|-----------------|------------------------------|---------------------|
| Speed | ⚡ Instant | 🐌 Slow (network) | 🐌 Very Slow |
| CI/CD | ✅ Simple | ❌ Version conflict | ❌ Requires Docker |
| Debugging | ✅ Easy | ⚠️ Moderate | ❌ Difficult |
| Wire Protocol | ❌ Not tested | ✅ Tested | ✅ Tested |
| Reliability | ✅ 100% | ⚠️ Flaky | ❌ Very Flaky |

## Testing Strategy

### What We Test In-Process
- Span creation and structure
- Attribute correctness
- Context propagation
- Error handling

### What We Test in Integration
- OTLP wire format
- gRPC communication
- Collector compatibility
- Production behavior

## Files

1. ✅ `tests/tracing_util.rs` - In-memory span collection
2. ✅ `Cargo.toml` - Added `parking_lot` for RwLock
3. ✅ `docs/IN_MEMORY_SPAN_TESTING.md` - This document

## Future

When `fake-opentelemetry-collector` upgrades to 0.30:
- Keep in-memory testing for unit tests (fast!)
- Add optional OTLP tests for integration (thorough!)
- Best of both worlds

---

**Status**: ✅ Implemented and working  
**Approach**: In-process span collection  
**Compatible With**: OpenTelemetry 0.30  
**CI/CD Ready**: Yes (no Docker required)  
**Date**: October 9, 2025

