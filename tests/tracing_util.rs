use fake_opentelemetry_collector::FakeCollector;
use opentelemetry::{global, sdk::trace as sdktrace};
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{layer::SubscriberExt, Registry};

pub struct TestTracing {
    pub collector: FakeCollector,
    _handle: std::thread::JoinHandle<()>,
    _guard: tracing::subscriber::DefaultGuard,
}

impl TestTracing {
    pub fn init() -> Self {
        let collector = FakeCollector::default();
        let endpoint = collector.endpoint();
        let handle = collector.run();

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(endpoint),
            )
            .with_trace_config(sdktrace::config())
            .install_simple()
            .expect("install tracer");

        let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
        let subscriber = Registry::default().with(opentelemetry);
        let guard = tracing::subscriber::set_default(subscriber);

        Self {
            collector,
            _handle: handle,
            _guard: guard,
        }
    }
}

impl Drop for TestTracing {
    fn drop(&mut self) {
        global::shutdown_tracer_provider();
    }
}

