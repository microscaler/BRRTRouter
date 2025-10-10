//! In-process span collection for testing (OpenTelemetry 0.30 compatible)
//!
//! This is a simple in-memory span collector that uses the OpenTelemetry SDK
//! to collect spans for test assertions without requiring external infrastructure.

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::error::OTelSdkError;
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, SdkTracerProvider, SpanProcessor};
use opentelemetry_sdk::trace::SpanData;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{prelude::*, Registry};

/// In-memory span processor for testing
/// Collects spans synchronously without batching for predictable testing
#[derive(Clone, Debug)]
struct InMemorySpanProcessor {
    spans: Arc<RwLock<Vec<SpanData>>>,
}

impl InMemorySpanProcessor {
    fn new(spans: Arc<RwLock<Vec<SpanData>>>) -> Self {
        Self { spans }
    }
}

impl SpanProcessor for InMemorySpanProcessor {
    fn on_start(
        &self,
        _span: &mut opentelemetry_sdk::trace::Span,
        _cx: &opentelemetry::Context,
    ) {
        // No-op for testing
    }

    fn on_end(&self, span: SpanData) {
        self.spans.write().push(span);
    }

    fn force_flush(&self) -> Result<(), OTelSdkError> {
        Ok(())
    }

    fn shutdown(&self) -> Result<(), OTelSdkError> {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> Result<(), OTelSdkError> {
        Ok(())
    }
}

/// Test tracing utility with in-memory span collection
pub struct TestTracing {
    spans: Arc<RwLock<Vec<SpanData>>>,
    tracer_provider: SdkTracerProvider,
}

impl Drop for TestTracing {
    fn drop(&mut self) {
        let _ = self.tracer_provider.force_flush();
        let _ = self.tracer_provider.shutdown();
    }
}

impl TestTracing {
    /// Initialize tracing with in-memory span collection
    pub fn init() -> Self {
        let spans = Arc::new(RwLock::new(Vec::new()));
        let processor = InMemorySpanProcessor::new(spans.clone());

        // Create tracer provider with in-memory processor
        // Using builder pattern with OpenTelemetry 0.30 API
        let tracer_provider = SdkTracerProvider::builder()
            .with_span_processor(processor)
            .with_id_generator(RandomIdGenerator::default())
            .with_sampler(Sampler::AlwaysOn)
            .build();

        // Set up tracing subscriber with OpenTelemetry layer
        let tracer = tracer_provider.tracer("brrtrouter-test");
        let telemetry_layer = OpenTelemetryLayer::new(tracer);
        let subscriber = Registry::default().with(telemetry_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);

        Self {
            spans,
            tracer_provider,
        }
    }

    /// Get all collected spans (returns a clone)
    pub fn spans(&self) -> Vec<SpanData> {
        self.spans.read().clone()
    }

    /// Get spans matching a specific name
    pub fn spans_named(&self, name: &str) -> Vec<SpanData> {
        self.spans
            .read()
            .iter()
            .filter(|s| s.name == name)
            .cloned()
            .collect()
    }

    /// Wait for at least `count` spans to be collected, with timeout
    #[allow(dead_code)]
    pub async fn collected_spans(&mut self, count: usize, timeout: Duration) -> Vec<SpanData> {
        let start = std::time::Instant::now();
        
        loop {
            {
                let spans = self.spans.read();
                if spans.len() >= count {
                    return spans.clone();
                }
            }
            
            if start.elapsed() > timeout {
                return self.spans.read().clone();
            }
            
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Wait for a span with a specific name to appear
    pub fn wait_for_span(&mut self, name: &str) {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(3);
        
        loop {
            {
                let spans = self.spans.read();
                if spans.iter().any(|s| s.name == name) {
                    return;
                }
            }
            
            if start.elapsed() > timeout {
                let spans = self.spans.read();
                let span_names: Vec<&str> = spans.iter().map(|s| s.name.as_ref()).collect();
                panic!(
                    "Span '{}' not found after {:?}. Available spans: {:?}",
                    name, timeout, span_names
                );
            }
            
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Force flush all pending spans
    pub fn force_flush(&self) {
        let _ = self.tracer_provider.force_flush();
    }

    /// Clear all collected spans
    pub fn clear_spans(&mut self) {
        self.spans.write().clear();
    }

    /// Get count of collected spans
    pub fn span_count(&self) -> usize {
        self.spans.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::{info_span, info};

    #[test]
    fn test_collect_spans() {
        let tracing = TestTracing::init();
        
        {
            let _span = info_span!("test_span").entered();
            info!("test message");
        }
        
        tracing.force_flush();
        
        let spans = tracing.spans();
        assert!(!spans.is_empty(), "Should collect at least one span");
        
        let test_spans = tracing.spans_named("test_span");
        assert_eq!(test_spans.len(), 1, "Should have exactly one 'test_span'");
    }

    #[test]
    fn test_wait_for_span() {
        let mut tracing = TestTracing::init();
        
        {
            let _span = info_span!("my_span").entered();
            info!("message");
        }
        
        tracing.force_flush();
        tracing.wait_for_span("my_span");
        
        // Should not panic - span was found
    }

    #[test]
    fn test_clear_spans() {
        let mut tracing = TestTracing::init();
        
        {
            let _span = info_span!("span1").entered();
            info!("message");
        }
        
        tracing.force_flush();
        assert!(tracing.span_count() > 0);
        
        tracing.clear_spans();
        assert_eq!(tracing.span_count(), 0);
    }
}
