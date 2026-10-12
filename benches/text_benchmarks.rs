//! Benchmarks for text processing applets (grep, sed, awk, sort, etc.)

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

fn armybox_path() -> String {
    std::env::var("ARMYBOX_PATH").unwrap_or_else(|_| "./target/release/armybox".to_string())
}

/// Generate test text file with realistic content
fn generate_log_file(lines: usize) -> NamedTempFile {
    let mut temp = NamedTempFile::new().unwrap();

    let log_entries = [
        "2024-01-01 10:00:00 INFO Starting application",
        "2024-01-01 10:00:01 DEBUG Loading configuration",
        "2024-01-01 10:00:02 WARN Config file not found, using defaults",
        "2024-01-01 10:00:03 ERROR Failed to connect to database",
        "2024-01-01 10:00:04 INFO Retrying connection...",
        "2024-01-01 10:00:05 INFO Connection established",
        "2024-01-01 10:00:06 DEBUG Processing request from 192.168.1.100",
        "2024-01-01 10:00:07 INFO Request completed in 45ms",
        "2024-01-01 10:00:08 DEBUG Cache hit for key: user_123",
        "2024-01-01 10:00:09 TRACE Memory usage: 256MB",
    ];

    for i in 0..lines {
        writeln!(temp, "{}", log_entries[i % log_entries.len()]).unwrap();
    }

    temp
}

/// Generate CSV-like data
fn generate_csv_file(rows: usize) -> NamedTempFile {
    let mut temp = NamedTempFile::new().unwrap();

    writeln!(temp, "id,name,email,age,city").unwrap();
    for i in 0..rows {
        writeln!(
            temp,
            "{},user_{},user_{}@example.com,{},City_{}",
            i,
            i,
            i,
            20 + (i % 50),
            i % 100
        )
        .unwrap();
    }

    temp
}

/// Benchmark grep with various patterns and file sizes
fn bench_grep(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("grep");
    group.sample_size(30);

    for lines in [1000, 10000, 100000] {
        let temp = generate_log_file(lines);
        let path = temp.path().to_str().unwrap().to_string();
        let size = std::fs::metadata(&path).unwrap().len();

        group.throughput(Throughput::Bytes(size));

        // Simple literal search
        group.bench_with_input(BenchmarkId::new("literal", lines), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["grep", "ERROR", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });

        // Regex search
        group.bench_with_input(BenchmarkId::new("regex", lines), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["grep", "-E", "ERROR|WARN", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });

        // Case insensitive
        group.bench_with_input(
            BenchmarkId::new("case_insensitive", lines),
            &path,
            |b, path| {
                b.iter(|| {
                    Command::new(&armybox)
                        .args(["grep", "-i", "error", path])
                        .stdout(Stdio::null())
                        .status()
                        .unwrap()
                })
            },
        );

        // With line numbers
        group.bench_with_input(
            BenchmarkId::new("with_line_numbers", lines),
            &path,
            |b, path| {
                b.iter(|| {
                    Command::new(&armybox)
                        .args(["grep", "-n", "INFO", path])
                        .stdout(Stdio::null())
                        .status()
                        .unwrap()
                })
            },
        );

        // Inverted match
        group.bench_with_input(BenchmarkId::new("inverted", lines), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["grep", "-v", "DEBUG", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark sed substitution
fn bench_sed(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("sed");
    group.sample_size(30);

    for lines in [1000, 10000, 100000] {
        let temp = generate_log_file(lines);
        let path = temp.path().to_str().unwrap().to_string();
        let size = std::fs::metadata(&path).unwrap().len();

        group.throughput(Throughput::Bytes(size));

        // Simple substitution
        group.bench_with_input(BenchmarkId::new("substitute", lines), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["sed", "s/ERROR/CRITICAL/g", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });

        // Delete lines
        group.bench_with_input(BenchmarkId::new("delete", lines), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["sed", "/DEBUG/d", path])
                    .stdout(Stdio::null())
                    .status()
            })
        });
    }

    group.finish();
}

/// Benchmark awk processing
fn bench_awk(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("awk");
    group.sample_size(30);

    for rows in [1000, 10000, 100000] {
        let temp = generate_csv_file(rows);
        let path = temp.path().to_str().unwrap().to_string();
        let size = std::fs::metadata(&path).unwrap().len();

        group.throughput(Throughput::Bytes(size));

        // Print specific columns
        group.bench_with_input(BenchmarkId::new("print_columns", rows), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["awk", "-F,", "{print $2, $4}", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });

        // Pattern matching
        group.bench_with_input(BenchmarkId::new("pattern_match", rows), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["awk", "-F,", "/user_5/{print $0}", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark sort with various inputs
fn bench_sort(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("sort");
    group.sample_size(20);

    for lines in [1000, 10000, 100000] {
        // Random numeric data
        let mut temp_numeric = NamedTempFile::new().unwrap();
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        for i in 0..lines {
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            writeln!(temp_numeric, "{}", hasher.finish() % 1000000).unwrap();
        }
        let numeric_path = temp_numeric.path().to_str().unwrap().to_string();

        // Random string data
        let mut temp_string = NamedTempFile::new().unwrap();
        for i in 0..lines {
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            writeln!(temp_string, "string_{:016x}", hasher.finish()).unwrap();
        }
        let string_path = temp_string.path().to_str().unwrap().to_string();

        let size = std::fs::metadata(&numeric_path).unwrap().len();
        group.throughput(Throughput::Elements(lines as u64));

        // Numeric sort
        group.bench_with_input(
            BenchmarkId::new("numeric", lines),
            &numeric_path,
            |b, path| {
                b.iter(|| {
                    Command::new(&armybox)
                        .args(["sort", "-n", path])
                        .stdout(Stdio::null())
                        .status()
                        .unwrap()
                })
            },
        );

        // String sort
        group.bench_with_input(
            BenchmarkId::new("string", lines),
            &string_path,
            |b, path| {
                b.iter(|| {
                    Command::new(&armybox)
                        .args(["sort", path])
                        .stdout(Stdio::null())
                        .status()
                        .unwrap()
                })
            },
        );

        // Unique sort
        group.bench_with_input(
            BenchmarkId::new("unique", lines),
            &numeric_path,
            |b, path| {
                b.iter(|| {
                    Command::new(&armybox)
                        .args(["sort", "-u", path])
                        .stdout(Stdio::null())
                        .status()
                        .unwrap()
                })
            },
        );

        // Reverse sort
        group.bench_with_input(
            BenchmarkId::new("reverse", lines),
            &string_path,
            |b, path| {
                b.iter(|| {
                    Command::new(&armybox)
                        .args(["sort", "-r", path])
                        .stdout(Stdio::null())
                        .status()
                        .unwrap()
                })
            },
        );
    }

    group.finish();
}

/// Benchmark uniq
fn bench_uniq(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("uniq");
    group.sample_size(30);

    for lines in [1000, 10000, 100000] {
        // Create file with many duplicates
        let mut temp = NamedTempFile::new().unwrap();
        for i in 0..lines {
            writeln!(temp, "line_{}", i % 100).unwrap(); // 100 unique lines
        }
        let path = temp.path().to_str().unwrap().to_string();

        // Need to sort first for uniq
        let sorted = NamedTempFile::new().unwrap();
        let sorted_path = sorted.path().to_str().unwrap().to_string();
        Command::new(&armybox)
            .args(["sort", &path])
            .stdout(std::fs::File::create(&sorted_path).unwrap())
            .status()
            .unwrap();

        let size = std::fs::metadata(&sorted_path).unwrap().len();
        group.throughput(Throughput::Bytes(size));

        group.bench_with_input(BenchmarkId::new("basic", lines), &sorted_path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["uniq", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });

        group.bench_with_input(BenchmarkId::new("count", lines), &sorted_path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["uniq", "-c", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark cut
fn bench_cut(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("cut");
    group.sample_size(30);

    for rows in [1000, 10000, 100000] {
        let temp = generate_csv_file(rows);
        let path = temp.path().to_str().unwrap().to_string();
        let size = std::fs::metadata(&path).unwrap().len();

        group.throughput(Throughput::Bytes(size));

        // Cut by delimiter
        group.bench_with_input(BenchmarkId::new("delimiter", rows), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["cut", "-d,", "-f2,4", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });

        // Cut by character position
        group.bench_with_input(BenchmarkId::new("characters", rows), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["cut", "-c1-20", path])
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark tr
fn bench_tr(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("tr");
    group.sample_size(30);

    for size in [1024, 10240, 102400] {
        let data = "The Quick Brown Fox Jumps Over The Lazy Dog\n".repeat(size / 44);
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(data.as_bytes()).unwrap();
        let path = temp.path().to_str().unwrap().to_string();

        group.throughput(Throughput::Bytes(data.len() as u64));

        // Lowercase to uppercase
        group.bench_with_input(
            BenchmarkId::new("case_conversion", size),
            &path,
            |b, path| {
                b.iter(|| {
                    Command::new(&armybox)
                        .args(["tr", "a-z", "A-Z"])
                        .stdin(std::fs::File::open(path).unwrap())
                        .stdout(Stdio::null())
                        .status()
                        .unwrap()
                })
            },
        );

        // Delete characters
        group.bench_with_input(BenchmarkId::new("delete", size), &path, |b, path| {
            b.iter(|| {
                Command::new(&armybox)
                    .args(["tr", "-d", "aeiou"])
                    .stdin(std::fs::File::open(path).unwrap())
                    .stdout(Stdio::null())
                    .status()
                    .unwrap()
            })
        });
    }

    group.finish();
}

/// Benchmark find
fn bench_find(c: &mut Criterion) {
    let armybox = armybox_path();

    let mut group = c.benchmark_group("find");
    group.sample_size(20);

    // Benchmark on source directory
    group.bench_function("find_src", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .args(["find", "src", "-name", "*.rs"])
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    group.bench_function("find_type_f", |b| {
        b.iter(|| {
            Command::new(&armybox)
                .args(["find", ".", "-type", "f"])
                .stdout(Stdio::null())
                .status()
                .unwrap()
        })
    });

    group.finish();
}

criterion_group!(
    benches, bench_grep, bench_sed, bench_awk, bench_sort, bench_uniq, bench_cut, bench_tr,
    bench_find,
);

criterion_main!(benches);
