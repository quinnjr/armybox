//! Benchmarks for compression applets

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::{NamedTempFile, TempDir};

fn armybox_path() -> String {
    std::env::var("ARMYBOX_PATH").unwrap_or_else(|_| "./target/release/armybox".to_string())
}

/// Generate test data with varying compressibility
fn generate_test_data(size: usize, compressibility: &str) -> Vec<u8> {
    match compressibility {
        "high" => {
            // Highly compressible: repeated pattern
            let pattern = b"AAAAAAAAAAAAAAAA";
            let mut data = Vec::with_capacity(size);
            while data.len() < size {
                data.extend_from_slice(pattern);
            }
            data.truncate(size);
            data
        }
        "medium" => {
            // Medium compressibility: text-like
            let text = "The quick brown fox jumps over the lazy dog. ";
            let mut data = Vec::with_capacity(size);
            while data.len() < size {
                data.extend_from_slice(text.as_bytes());
            }
            data.truncate(size);
            data
        }
        "low" => {
            // Low compressibility: pseudo-random
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut data = Vec::with_capacity(size);
            let mut hasher = DefaultHasher::new();
            for i in 0..size {
                i.hash(&mut hasher);
                data.push((hasher.finish() & 0xFF) as u8);
            }
            data
        }
        _ => vec![0u8; size],
    }
}

/// Benchmark gzip compression
fn bench_gzip_compress(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("gzip_compress");
    group.sample_size(20);

    for (size, compressibility) in [
        (10240, "high"),
        (10240, "medium"),
        (10240, "low"),
        (102400, "medium"),
        (1024000, "medium"),
    ] {
        let data = generate_test_data(size, compressibility);
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&data).unwrap();
        let path = temp.path().to_str().unwrap().to_string();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::new(compressibility, size), &path, |b, path| {
            b.iter(|| {
                // Copy file since gzip removes original
                let temp_copy = NamedTempFile::new().unwrap();
                std::fs::copy(path, temp_copy.path()).unwrap();
                let copy_path = temp_copy.path().to_str().unwrap();

                Command::new(&armybox)
                    .args(["gzip", "-f", copy_path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark gzip decompression
fn bench_gzip_decompress(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("gzip_decompress");
    group.sample_size(20);

    for size in [10240, 102400, 1024000] {
        // Create and compress test data
        let data = generate_test_data(size, "medium");
        let temp_dir = TempDir::new().unwrap();
        let orig_path = temp_dir.path().join("test.txt");
        std::fs::write(&orig_path, &data).unwrap();

        // Compress with system gzip or our gzip
        Command::new(&armybox)
            .args(["gzip", "-k", orig_path.to_str().unwrap()])
            .status()
            .unwrap();

        let gz_path = temp_dir.path().join("test.txt.gz");
        let _gz_size = std::fs::metadata(&gz_path).unwrap().len();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &gz_path, |b, gz_path| {
            b.iter(|| {
                // Copy compressed file
                let temp_copy = NamedTempFile::new().unwrap();
                let copy_path = format!("{}.gz", temp_copy.path().to_str().unwrap());
                std::fs::copy(gz_path, &copy_path).unwrap();

                Command::new(&armybox)
                    .args(["gunzip", "-f", &copy_path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark bzip2 compression
fn bench_bzip2_compress(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("bzip2_compress");
    group.sample_size(10); // bzip2 is slower

    for size in [10240, 102400] {
        let data = generate_test_data(size, "medium");
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&data).unwrap();
        let path = temp.path().to_str().unwrap().to_string();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &path, |b, path| {
            b.iter(|| {
                let temp_copy = NamedTempFile::new().unwrap();
                std::fs::copy(path, temp_copy.path()).unwrap();
                let copy_path = temp_copy.path().to_str().unwrap();

                Command::new(&armybox)
                    .args(["bzip2", "-f", copy_path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark xz compression
fn bench_xz_compress(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("xz_compress");
    group.sample_size(10); // xz is slower

    for size in [10240, 102400] {
        let data = generate_test_data(size, "medium");
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&data).unwrap();
        let path = temp.path().to_str().unwrap().to_string();

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &path, |b, path| {
            b.iter(|| {
                let temp_copy = NamedTempFile::new().unwrap();
                std::fs::copy(path, temp_copy.path()).unwrap();
                let copy_path = temp_copy.path().to_str().unwrap();

                Command::new(&armybox)
                    .args(["xz", "-f", copy_path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark tar creation
fn bench_tar_create(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("tar_create");
    group.sample_size(20);

    // Create test directory structure
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir(&src_dir).unwrap();

    for i in 0..100 {
        let file_path = src_dir.join(format!("file_{}.txt", i));
        let data = generate_test_data(1024, "medium");
        std::fs::write(&file_path, &data).unwrap();
    }

    let src_path = src_dir.to_str().unwrap().to_string();

    group.bench_function("tar_100_files", |b| {
        b.iter(|| {
            let tar_file = NamedTempFile::new().unwrap();
            let tar_path = tar_file.path().to_str().unwrap();

            Command::new(&armybox)
                .args(["tar", "-cf", tar_path, &src_path])
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    // Benchmark tar.gz creation
    group.bench_function("tar_gz_100_files", |b| {
        b.iter(|| {
            let tar_file = NamedTempFile::new().unwrap();
            let tar_path = format!("{}.tar.gz", tar_file.path().to_str().unwrap());

            Command::new(&armybox)
                .args(["tar", "-czf", &tar_path, &src_path])
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    group.finish();
}

/// Compare compression ratios
fn bench_compression_ratios(c: &mut Criterion) {
    let armybox = armybox_path();

    // Create test data
    let size = 102400; // 100KB
    let data = generate_test_data(size, "medium");

    let temp_dir = TempDir::new().unwrap();
    let orig_path = temp_dir.path().join("test.txt");
    std::fs::write(&orig_path, &data).unwrap();

    let mut group = c.benchmark_group("compression_comparison");
    group.sample_size(10);
    group.throughput(Throughput::Bytes(size as u64));

    // gzip
    group.bench_function("gzip", |b| {
        b.iter(|| {
            let temp = NamedTempFile::new().unwrap();
            std::fs::copy(&orig_path, temp.path()).unwrap();
            Command::new(&armybox)
                .args(["gzip", "-f", temp.path().to_str().unwrap()])
                .status()
                .unwrap()
        })
    });

    // bzip2
    group.bench_function("bzip2", |b| {
        b.iter(|| {
            let temp = NamedTempFile::new().unwrap();
            std::fs::copy(&orig_path, temp.path()).unwrap();
            Command::new(&armybox)
                .args(["bzip2", "-f", temp.path().to_str().unwrap()])
                .status()
                .unwrap()
        })
    });

    // xz
    group.bench_function("xz", |b| {
        b.iter(|| {
            let temp = NamedTempFile::new().unwrap();
            std::fs::copy(&orig_path, temp.path()).unwrap();
            Command::new(&armybox)
                .args(["xz", "-f", temp.path().to_str().unwrap()])
                .status()
                .unwrap()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_gzip_compress,
    bench_gzip_decompress,
    bench_bzip2_compress,
    bench_xz_compress,
    bench_tar_create,
    bench_compression_ratios,
);

criterion_main!(benches);
