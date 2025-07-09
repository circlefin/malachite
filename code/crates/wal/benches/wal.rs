use std::fs;
use std::hint::black_box;
use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tempfile::tempdir;

use informalsystems_malachitebft_wal::Log;

/// Benchmark configuration
struct BenchConfig {
    entry_size: usize,
    batch_size: usize,
    sync_interval: usize,
}

impl BenchConfig {
    fn total_size(&self) -> usize {
        self.entry_size * self.batch_size
    }
}

fn wal_benchmarks(c: &mut Criterion) {
    let dir = tempdir().unwrap();

    // Different entry sizes to test
    let entry_sizes = vec![
        64,          // Small entries
        1024,        // 1 KB
        16 * 1024,   // 16 KB
        256 * 1024,  // 256 KB
        1024 * 1024, // 1 MB
    ];

    // Read benchmarks
    let mut read_group = c.benchmark_group("wal_read");

    // Benchmark reading different entry sizes
    for size in &entry_sizes {
        let config = BenchConfig {
            entry_size: *size,
            batch_size: 1000,
            sync_interval: 100,
        };

        read_group.throughput(Throughput::Bytes(config.total_size() as u64));
        read_group.bench_with_input(BenchmarkId::new("sequential_read", size), size, |b, _| {
            let path = get_temp_wal_path(&dir);
            setup_wal_for_reading(&path, &config);
            b.iter(|| bench_sequential_read(&path));
            fs::remove_file(path).unwrap();
        });
    }
    read_group.finish();

    // Write benchmarks
    let mut write_group = c.benchmark_group("wal_write");

    // Benchmark writing different entry sizes
    for size in &entry_sizes {
        let config = BenchConfig {
            entry_size: *size,
            batch_size: 1000,
            sync_interval: 100,
        };

        write_group.throughput(Throughput::Bytes(config.total_size() as u64));
        write_group.bench_with_input(BenchmarkId::new("sequential_write", size), size, |b, _| {
            let path = get_temp_wal_path(&dir);
            b.iter(|| bench_sequential_write(&path, &config));
        });
    }

    // Benchmark different batch sizes
    let batch_sizes = vec![1, 10, 100, 1000, 10000];

    for batch_size in &batch_sizes {
        let config = BenchConfig {
            entry_size: 1024,
            batch_size: *batch_size,
            sync_interval: *batch_size,
        };

        write_group.throughput(Throughput::Bytes(config.total_size() as u64));
        write_group.bench_with_input(
            BenchmarkId::new("batch_write", batch_size),
            batch_size,
            |b, _| {
                let path = get_temp_wal_path(&dir);
                b.iter(|| bench_batch_write(&path, &config));
            },
        );
    }

    // Benchmark different sync intervals
    let sync_intervals = vec![1, 10, 100, 1000];

    for interval in &sync_intervals {
        let config = BenchConfig {
            entry_size: 1024,
            batch_size: 1000,
            sync_interval: *interval,
        };

        write_group.throughput(Throughput::Bytes(config.total_size() as u64));
        write_group.bench_with_input(
            BenchmarkId::new("sync_interval", interval),
            interval,
            |b, &_| {
                let path = get_temp_wal_path(&dir);
                b.iter(|| bench_sync_interval(&path, &config));
            },
        );
    }

    // // Mixed read/write benchmarks
    // let mut mixed_group = c.benchmark_group("wal_mixed");
    //
    // mixed_group.finish();
}

/// Helper function to get a unique WAL path
fn get_temp_wal_path(dir: &tempfile::TempDir) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    dir.path().join(format!("bench_{id}.wal"))
}

/// Benchmark sequential writes
fn bench_sequential_write(path: &PathBuf, config: &BenchConfig) {
    let mut wal = Log::open(path).unwrap();
    let data = vec![0u8; config.entry_size];

    for i in 0..config.batch_size {
        wal.append(&data).unwrap();
        if i % config.sync_interval == 0 {
            wal.flush().unwrap();
        }
    }

    fs::remove_file(path).unwrap()
}

/// Benchmark sequential reads
fn bench_sequential_read(path: &PathBuf) {
    let mut wal = Log::open(path).unwrap();
    black_box(wal.iter().unwrap().collect::<Result<Vec<_>, _>>().unwrap());
}

/// Setup WAL with data for reading benchmarks
fn setup_wal_for_reading(path: &PathBuf, config: &BenchConfig) {
    let mut wal = Log::open(path).unwrap();
    let data = vec![0u8; config.entry_size];

    for i in 0..config.batch_size {
        wal.append(&data).unwrap();
        if i % config.sync_interval == 0 {
            wal.flush().unwrap();
        }
    }
}

/// Benchmark batch writes
fn bench_batch_write(path: &PathBuf, config: &BenchConfig) {
    let mut wal = Log::open(path).unwrap();
    let data = vec![0u8; config.entry_size];

    for _ in 0..config.batch_size {
        wal.append(&data).unwrap();
    }
    wal.flush().unwrap();
    fs::remove_file(path).unwrap();
}

/// Benchmark different sync intervals
fn bench_sync_interval(path: &PathBuf, config: &BenchConfig) {
    let mut wal = Log::open(path).unwrap();
    let data = vec![0u8; config.entry_size];

    for i in 0..config.batch_size {
        wal.append(&data).unwrap();
        if i % config.sync_interval == 0 {
            wal.flush().unwrap();
        }
    }
}

/// Benchmark small writes with frequent syncs
fn bench_small_writes_frequent_sync(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_writes_frequent_sync");
    let dir = tempdir().unwrap();

    group.throughput(Throughput::Bytes(64 * 100));
    group.bench_function("write_sync_every", |b| {
        b.iter(|| {
            let path = get_temp_wal_path(&dir);
            let mut wal = Log::open(&path).unwrap();
            let data = vec![0u8; 64];

            for _ in 0..100 {
                wal.append(&data).unwrap();
                wal.flush().unwrap();
            }
            fs::remove_file(path).unwrap();
        });
    });

    group.finish();
}

/// Benchmark random access patterns
fn bench_random_access(c: &mut Criterion) {
    use rand::Rng;

    let mut group = c.benchmark_group("random_access");
    let dir = tempdir().unwrap();

    group.bench_function("random_sized_writes", |b| {
        b.iter(|| {
            let path = get_temp_wal_path(&dir);
            let mut wal = Log::open(&path).unwrap();
            let mut rng = rand::thread_rng();

            for _ in 0..100 {
                let size = rng.gen_range(64..4096);
                let data = vec![0u8; size];
                wal.append(&data).unwrap();
            }
            wal.flush().unwrap();
            fs::remove_file(path).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    wal_benchmarks,
    bench_small_writes_frequent_sync,
    bench_random_access
);
criterion_main!(benches);
