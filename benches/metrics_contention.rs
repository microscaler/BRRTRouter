use brrtrouter::middleware::MetricsMiddleware;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use std::thread;

/// Benchmark single-threaded metrics recording (baseline)
fn bench_single_thread_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_metrics");
    
    for num_paths in [1, 10, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("path_metrics", num_paths),
            num_paths,
            |b, &num_paths| {
                let metrics = MetricsMiddleware::new();
                let paths: Vec<String> = (0..num_paths)
                    .map(|i| format!("/api/resource{}", i))
                    .collect();
                
                b.iter(|| {
                    for path in &paths {
                        metrics.record_path_metrics(black_box(path), black_box(1000));
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent metrics recording (high contention scenario)
fn bench_concurrent_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_metrics");
    group.sample_size(10); // Reduce sample size due to threading overhead
    
    for num_threads in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("high_contention_same_path", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let metrics = Arc::new(MetricsMiddleware::new());
                    let mut handles = vec![];
                    
                    for _ in 0..num_threads {
                        let metrics_clone = Arc::clone(&metrics);
                        let handle = thread::spawn(move || {
                            for i in 0..1000 {
                                metrics_clone.record_path_metrics(
                                    black_box("/hot/path"),
                                    black_box(1000 + i % 100),
                                );
                            }
                        });
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent metrics with multiple paths (medium contention)
fn bench_concurrent_multiple_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_multiple_paths");
    group.sample_size(10);
    
    for num_threads in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("medium_contention", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let metrics = Arc::new(MetricsMiddleware::new());
                    let mut handles = vec![];
                    
                    for thread_id in 0..num_threads {
                        let metrics_clone = Arc::clone(&metrics);
                        let handle = thread::spawn(move || {
                            for i in 0..1000 {
                                let path = format!("/path{}", (thread_id + i) % 10);
                                metrics_clone.record_path_metrics(
                                    black_box(&path),
                                    black_box(1000 + i % 100),
                                );
                            }
                        });
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark pre-registered paths vs on-the-fly registration
fn bench_pre_registration(c: &mut Criterion) {
    let mut group = c.benchmark_group("pre_registration");
    
    // Benchmark with pre-registration
    group.bench_function("with_pre_registration", |b| {
        let metrics = MetricsMiddleware::new();
        let paths: Vec<String> = (0..100).map(|i| format!("/api/resource{}", i)).collect();
        metrics.pre_register_paths(&paths);
        
        b.iter(|| {
            for path in &paths {
                metrics.record_path_metrics(black_box(path), black_box(1000));
            }
        });
    });
    
    // Benchmark without pre-registration
    group.bench_function("without_pre_registration", |b| {
        let paths: Vec<String> = (0..100).map(|i| format!("/api/resource{}", i)).collect();
        
        b.iter(|| {
            let metrics = MetricsMiddleware::new();
            for path in &paths {
                metrics.record_path_metrics(black_box(path), black_box(1000));
            }
        });
    });
    
    group.finish();
}

/// Benchmark status code recording under contention
fn bench_concurrent_status_codes(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_status_codes");
    group.sample_size(10);
    
    for num_threads in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("status_recording", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let metrics = Arc::new(MetricsMiddleware::new());
                    let mut handles = vec![];
                    
                    for thread_id in 0..num_threads {
                        let metrics_clone = Arc::clone(&metrics);
                        let handle = thread::spawn(move || {
                            for i in 0..1000 {
                                let path = format!("/path{}", thread_id % 5);
                                let status = if i % 3 == 0 { 200 } else { 500 };
                                metrics_clone.record_status(
                                    black_box(&path),
                                    black_box(status),
                                );
                            }
                        });
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark realistic mixed workload
fn bench_realistic_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workload");
    group.sample_size(10);
    
    group.bench_function("simulated_5k_rps", |b| {
        b.iter(|| {
            let metrics = Arc::new(MetricsMiddleware::new());
            
            // Pre-register common paths
            metrics.pre_register_paths(&[
                "/api/users",
                "/api/posts",
                "/api/comments",
                "/health",
                "/metrics",
            ]);
            
            let mut handles = vec![];
            
            // Simulate 16 threads handling requests
            for thread_id in 0..16 {
                let metrics_clone = Arc::clone(&metrics);
                let handle = thread::spawn(move || {
                    for i in 0..312 { // ~5k requests total (16 * 312 ≈ 5k)
                        // Mix of common and unique paths (80/20 rule)
                        let path = if i % 5 < 4 {
                            // 80% hit common paths
                            match thread_id % 5 {
                                0 => "/api/users",
                                1 => "/api/posts",
                                2 => "/api/comments",
                                3 => "/health",
                                _ => "/metrics",
                            }
                        } else {
                            // 20% hit unique paths (simulate dynamic routes)
                            "/api/unique"
                        };
                        
                        let latency = 1000 + (i * thread_id) % 10000;
                        let status = if i % 10 == 0 { 500 } else { 200 };
                        
                        metrics_clone.record_path_metrics(
                            black_box(path),
                            black_box(latency as u64),
                        );
                        metrics_clone.record_status(black_box(path), black_box(status));
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_single_thread_metrics,
    bench_concurrent_metrics,
    bench_concurrent_multiple_paths,
    bench_pre_registration,
    bench_concurrent_status_codes,
    bench_realistic_workload
);
criterion_main!(benches);
