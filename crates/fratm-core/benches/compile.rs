//! Compile-time benchmark: how long each stage takes on the examples, and on
//! one large program made of all of them. No external harness, so it runs on
//! stable Rust: `cargo bench -p fratm-core` (or `just bench`).
//!
//! Numbers are the median of several timed rounds, so they are stable enough
//! to compare two commits on the same machine.

use fratm_core::lexer::Lexer;
use fratm_core::parser::Parser;
use fratm_core::{compile, format, CompileOptions};
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const ROUNDS: usize = 15;

/// Median time of one call to `f`, repeated until a round lasts ~20 ms.
fn measure(mut f: impl FnMut()) -> Duration {
    let start = Instant::now();
    let mut per_round = 0u32;
    while start.elapsed() < Duration::from_millis(20) {
        f();
        per_round += 1;
    }
    let mut times: Vec<Duration> = (0..ROUNDS)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..per_round {
                f();
            }
            start.elapsed() / per_round
        })
        .collect();
    times.sort();
    times[ROUNDS / 2]
}

fn row(name: &str, bytes: usize, time: Duration) {
    let mb_per_s = bytes as f64 / time.as_secs_f64() / 1_000_000.0;
    println!(
        "{name:<28} {:>10.1} µs {mb_per_s:>8.1} MB/s",
        time.as_secs_f64() * 1e6
    );
}

fn main() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut files: Vec<PathBuf> = fs::read_dir(&examples)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "fratm"))
        .collect();
    files.sort();
    let sources: Vec<(String, String)> = files
        .iter()
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(p).unwrap())
        })
        .collect();

    println!("{:<28} {:>13} {:>13}", "compile", "time", "throughput");
    for (name, source) in &sources {
        let time = measure(|| {
            black_box(compile(black_box(source), CompileOptions::default()).unwrap());
        });
        row(name, source.len(), time);
    }

    // One big program: every example wrapped in its own block, 20 times over
    let big: String = (0..20)
        .flat_map(|_| sources.iter())
        .filter(|(_, s)| !s.contains("chiamm") && !s.contains("mann for"))
        .map(|(_, s)| format!("{{\n{s}\n}}\n"))
        .collect();
    println!();
    println!(
        "{:<28} {:>13} {:>13}",
        format!("big program ({} KB)", big.len() / 1024),
        "time",
        "throughput"
    );
    let stages: [(&str, &dyn Fn()); 5] = [
        ("lexer", &|| {
            black_box(Lexer::new(black_box(&big)).tokenize());
        }),
        ("lexer + parser", &|| {
            let tokens = Lexer::new(black_box(&big)).tokenize();
            black_box(Parser::new(tokens).parse().unwrap());
        }),
        ("compile", &|| {
            black_box(compile(black_box(&big), CompileOptions::default()).unwrap());
        }),
        ("compile + source map", &|| {
            let options = CompileOptions {
                source_map: true,
                ..Default::default()
            };
            black_box(compile(black_box(&big), options).unwrap());
        }),
        ("format", &|| {
            black_box(format(black_box(&big)).unwrap());
        }),
    ];
    for (name, f) in stages {
        row(name, big.len(), measure(f));
    }
}
