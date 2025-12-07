use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use master_orchestrator::memory_service::MemoryService;
use tempfile::TempDir;
use std::sync::Arc;

async fn setup_memory_service() -> (Arc<MemoryService>, TempDir) {
    let temp_dir = TempDir::new().expect("temp dir");
    let sqlite_path = temp_dir.path().join("memory_kg.db");
    let sled_path = temp_dir.path().join("sled");

    let memory_service = Arc::new(
        MemoryService::new(
            sqlite_path.to_str().expect("sqlite path utf8"),
            sled_path.to_str().expect("sled path utf8"),
        )
        .expect("memory service should initialize"),
    );

    memory_service
        .init_gai_memory()
        .await
        .expect("init_gai_memory should succeed");

    (memory_service, temp_dir)
}

fn memory_benchmarks(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_operations");
    group.sample_size(50);
    group.measurement_time(std::time::Duration::from_secs(30));

    // Benchmark different data sizes for semantic storage
    let data_sizes = vec![100, 1000, 10000];
    
    for size in data_sizes {
        group.bench_with_input(
            BenchmarkId::new("semantic_store", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let (memory_service, _temp) = setup_memory_service().await;
                    let data = "x".repeat(size);
                    memory_service.store_semantic_data("test_key", &data).await.unwrap()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("semantic_retrieve", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let (memory_service, _temp) = setup_memory_service().await;
                    let data = "x".repeat(size);
                    memory_service.store_semantic_data("test_key", &data).await.unwrap();
                    memory_service.get_semantic_data("test_key").await.unwrap()
                });
            },
        );
    }

    // Benchmark concurrent operations
    let concurrent_counts = vec![1, 5, 10];
    
    for count in concurrent_counts {
        group.bench_with_input(
            BenchmarkId::new("concurrent_store", count),
            &count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let (memory_service, _temp) = setup_memory_service().await;
                    
                    let futures = (0..count).map(|i| {
                        let ms = memory_service.clone();
                        let key = format!("key_{}", i);
                        let data = format!("data_{}", i);
                        async move {
                            ms.store_semantic_data(&key, &data).await.unwrap()
                        }
                    });

                    futures_util::future::join_all(futures).await
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("concurrent_retrieve", count),
            &count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let (memory_service, _temp) = setup_memory_service().await;
                    
                    // First store the test data
                    for i in 0..count {
                        let key = format!("key_{}", i);
                        let data = format!("data_{}", i);
                        memory_service.store_semantic_data(&key, &data).await.unwrap();
                    }

                    // Then benchmark concurrent retrieval
                    let futures = (0..count).map(|i| {
                        let ms = memory_service.clone();
                        let key = format!("key_{}", i);
                        async move {
                            ms.get_semantic_data(&key).await.unwrap()
                        }
                    });

                    futures_util::future::join_all(futures).await
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, memory_benchmarks);
criterion_main!(benches);