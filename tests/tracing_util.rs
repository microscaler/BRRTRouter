use fake_opentelemetry_collector::{setup_tracer_provider, ExportedSpan, FakeCollectorServer};
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::time::Duration;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{prelude::*, Registry};

pub struct TestTracing {
    pub fake_collector: Option<FakeCollectorServer>,
    tracer_provider: SdkTracerProvider,
    // Hold the Tokio runtime to keep the OTLP collector running
    _runtime: tokio::runtime::Runtime,
}

impl Drop for TestTracing {
    fn drop(&mut self) {
        let _ = self.tracer_provider.force_flush();
        let _ = self.tracer_provider.shutdown();

        if let Some(fc) = self.fake_collector.take() {
            fc.abort();
        }
    }
}

impl TestTracing {
    /// Initialize OpenTelemetry tracing with a fake OTLP collector for testing.
    pub fn init() -> Self {
        // Create a Tokio runtime for the fake collector and OTLP pipeline
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        // Start the fake OTLP collector and set up a tracer provider using it
        let (fake_collector, tracer_provider) = runtime.block_on(async {
            let fc = FakeCollectorServer::start()
                .await
                .expect("Failed to start fake OTLP collector");
            // Configure an OpenTelemetry trace pipeline to send to the fake collector
            let tp = setup_tracer_provider(&fc).await;
            (fc, tp)
        });
        // Get a tracer from the provider and build a tracing subscriber with an OpenTelemetry layer
        let tracer = tracer_provider.tracer("BRRTRouterTest");
        let telemetry_layer = OpenTelemetryLayer::new(tracer);
        let subscriber = Registry::default().with(telemetry_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
        Self {
            fake_collector: Some(fake_collector),
            tracer_provider,
            _runtime: runtime,
        }
    }

    /// Retrieve exported spans from the fake collector (waits until at least `count` spans are received or `timeout`).
    pub async fn collected_spans(&mut self, count: usize, timeout: Duration) -> Vec<ExportedSpan> {
        self.fake_collector
            .as_mut()
            .unwrap()
            .exported_spans(count, timeout)
            .await
    }

    pub fn spans(&mut self) -> Vec<ExportedSpan> {
        self._runtime.block_on(async {
            self.fake_collector
                .as_mut()
                .unwrap()
                .exported_spans(1, Duration::from_secs(3))
                .await
        })
    }
    pub fn wait_for_span(&mut self, name: &str) {
        self._runtime.block_on(async {
            let spans = self
                .fake_collector
                .as_mut()
                .unwrap()
                .exported_spans(1, Duration::from_secs(3))
                .await;

            assert!(
                spans.iter().any(|s| s.name == name),
                "span `{name}` not found"
            );
        });
    }
}
