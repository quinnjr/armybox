//! Benchmarks for core applet dispatch and argument parsing

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

/// Path to the armybox binary
fn armybox_path() -> String {
    std::env::var("ARMYBOX_PATH").unwrap_or_else(|_| "./target/release/armybox".to_string())
}

/// Benchmark applet dispatch overhead
fn bench_dispatch_overhead(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("dispatch_overhead");

    // Benchmark `true` - minimal applet
    group.bench_function("true", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .arg("true")
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    // Benchmark `false` - minimal applet
    group.bench_function("false", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .arg("false")
                .stdout(Stdio::null())
                .status()
        })
    });

    // Benchmark `echo` with small output
    group.bench_function("echo_small", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .args(["echo", "hello"])
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    group.finish();
}

/// Benchmark echo with various output sizes
fn bench_echo_throughput(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("echo_throughput");

    for size in [10, 100, 1000, 10000].iter() {
        let data = "x".repeat(*size);
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["echo", "-n", data])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark cat with various file sizes
fn bench_cat_throughput(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("cat_throughput");
    group.sample_size(50);

    for size in [1024, 10240, 102400, 1024000].iter() {
        let mut temp = NamedTempFile::new().unwrap();
        let data = vec![b'x'; *size];
        temp.write_all(&data).unwrap();
        let path = temp.path().to_str().unwrap().to_string();

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["cat", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark wc with various file sizes
fn bench_wc_throughput(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("wc_throughput");
    group.sample_size(50);

    for size in [1024, 10240, 102400, 1024000].iter() {
        let mut temp = NamedTempFile::new().unwrap();
        // Create file with words and lines
        let lines = *size / 50; // ~50 chars per line
        for _ in 0..lines {
            writeln!(temp, "the quick brown fox jumps over the lazy dog").unwrap();
        }
        let path = temp.path().to_str().unwrap().to_string();

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["wc", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark head/tail with various sizes
fn bench_head_tail(c: &mut Criterion) {
    let armybox = armybox_path();

    // Create a large file with many lines
    let mut temp = NamedTempFile::new().unwrap();
    for i in 0..100000 {
        writeln!(temp, "Line number {} with some content here", i).unwrap();
    }
    let path = temp.path().to_str().unwrap().to_string();

    let mut group = c.benchmark_group("head_tail");
    group.sample_size(50);

    for n in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("head", n), &(*n, &path), |b, (n, path)| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["head", "-n", &n.to_string(), path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });

        group.bench_with_input(BenchmarkId::new("tail", n), &(*n, &path), |b, (n, path)| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["tail", "-n", &n.to_string(), path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark ls with various directory sizes
fn bench_ls(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("ls");

    // Benchmark ls on current directory
    group.bench_function("ls_simple", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .arg("ls")
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    // Benchmark ls -la
    group.bench_function("ls_la", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .args(["ls", "-la"])
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    // Benchmark ls -laR (recursive)
    group.bench_function("ls_recursive", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .args(["ls", "-laR", "src"])
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    group.finish();
}

/// Benchmark checksum computation
fn bench_checksums(c: &mut Criterion) {
    let armybox = armybox_path();

    // Create test file
    let mut temp = NamedTempFile::new().unwrap();
    let data = vec![b'x'; 1024 * 1024]; // 1MB
    temp.write_all(&data).unwrap();
    let path = temp.path().to_str().unwrap().to_string();

    let mut group = c.benchmark_group("checksums");
    group.sample_size(20);
    group.throughput(Throughput::Bytes(1024 * 1024));

    group.bench_function("md5sum", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .args(["md5sum", &path])
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    group.bench_function("sha256sum", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .args(["sha256sum", &path])
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_dispatch_overhead,
    bench_echo_throughput,
    bench_cat_throughput,
    bench_wc_throughput,
    bench_head_tail,
    bench_ls,
    bench_checksums,
);

criterion_main!(benches);
