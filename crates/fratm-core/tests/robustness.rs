//! Robustness tests: the compiler must never panic, hang or overflow the
//! stack, whatever it is given. Every example and golden case is mutated
//! (bytes and keywords deleted, inserted, duplicated, spliced from another
//! file) and each result goes through `compile`, `compile` with a source
//! map and `format`, which may fail with an error but nothing worse.
//!
//! The mutations come from a fixed seed, so a failure always reproduces.
//! Run longer with another seed to look for new bugs:
//! `FRATM_FUZZ_ITERATIONS=200000 FRATM_FUZZ_SEED=7 cargo test --release -p fratm-core --test robustness`

use fratm_core::{compile, format, CompileOptions};
use std::fs;
use std::panic;
use std::path::PathBuf;

/// Fragments that mutations insert: keywords, brackets and operators in
/// the shapes that reach the less travelled parser paths.
const FRAGMENTS: &[&str] = &[
    "chist è ",
    "tien ",
    "facc ",
    "piglie ",
    "si ",
    "sinnò ",
    "mentre che ",
    "pe ",
    "pruvamm ",
    "e si schiatta ",
    "aropp ",
    "iett ",
    "stamm a dì",
    "nu bell ",
    "stu cos",
    "mo vir ",
    "aspett ",
    "caccia ",
    "na famiglie ",
    "vedimm ",
    "caso ",
    "rompe",
    "salta",
    "chiamm ",
    "mann for ",
    " da ",
    " dint ",
    "chè è ",
    "è uno ",
    "damme ",
    "miette ",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    ",",
    ";",
    ".",
    "?.",
    "??",
    "=>",
    "=",
    "+=",
    "++",
    "--",
    "-",
    "+",
    "*",
    "/",
    "%",
    "**",
    "<",
    "===",
    "!",
    "&&",
    "||",
    "?",
    ":",
    "...",
    "#",
    "`",
    "${",
    "\"",
    "'",
    "/re/g",
    "\n",
    "0x1F",
    "1e3",
    ".5",
    "x",
    "\\",
    "//",
    "/*",
    "*/",
    "é",
    "😀",
    "\r\n",
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

fn corpus() -> Vec<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dirs = [
        root.join("../../examples"),
        root.join("tests/golden/cases"),
        root.join("../../packages/playground/src/scola"),
    ];
    let mut files: Vec<PathBuf> = dirs
        .iter()
        .filter(|dir| dir.is_dir())
        .flat_map(|dir| fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "fratm"))
        .collect();
    files.sort();
    files.iter().map(|p| fs::read(p).unwrap()).collect()
}

fn mutate(rng: &mut Rng, corpus: &[Vec<u8>]) -> Vec<u8> {
    let mut s = corpus[rng.below(corpus.len())].clone();
    for _ in 0..1 + rng.below(6) {
        let len = s.len();
        let at = rng.below(len + 1);
        match rng.below(6) {
            0 if len > 0 => {
                let start = rng.below(len);
                let end = (start + 1 + rng.below(20)).min(len);
                s.drain(start..end);
            }
            1 if len > 0 => {
                let start = rng.below(len);
                let end = (start + 1 + rng.below(40)).min(len);
                let chunk = s[start..end].to_vec();
                s.splice(at..at, chunk);
            }
            2 => {
                let other = &corpus[rng.below(corpus.len())];
                if !other.is_empty() {
                    let start = rng.below(other.len());
                    let end = (start + 1 + rng.below(80)).min(other.len());
                    s.splice(at..at, other[start..end].iter().copied());
                }
            }
            3 if len > 0 => s.truncate(rng.below(len)),
            4 if len > 0 => s[rng.below(len)] = (rng.next() & 0x7f) as u8,
            _ => {
                let fragment = FRAGMENTS[rng.below(FRAGMENTS.len())];
                s.splice(at..at, fragment.bytes());
            }
        }
    }
    s
}

fn env_or(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Compiles, maps and formats `source`; only errors are acceptable.
fn exercise(source: &str) {
    let compiled = compile(source, CompileOptions::default());
    let _ = compile(
        source,
        CompileOptions {
            source_map: true,
            ..Default::default()
        },
    );
    if compiled.is_ok() {
        // The formatter may refuse, but it must not produce code that
        // fails to compile or that means something else.
        if let Err(e) = format(source) {
            let message = e.to_string();
            assert!(
                !message.contains("does not compile") && !message.contains("change the meaning"),
                "fratm fmt broke the program: {message}"
            );
        }
    }
}

fn run_mutations(iterations: u64, seed: u64) {
    let corpus = corpus();
    assert!(!corpus.is_empty(), "no .fratm files found");
    let mut rng = Rng(seed | 1);
    panic::set_hook(Box::new(|_| {}));
    for i in 0..iterations {
        let input = mutate(&mut rng, &corpus);
        let Ok(source) = String::from_utf8(input) else {
            continue;
        };
        if let Err(payload) = panic::catch_unwind(|| exercise(&source)) {
            let _ = panic::take_hook();
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            panic!("mutation {i} with seed {seed} panicked: {message}\ninput: {source:?}");
        }
        if i > 0 && i % 50_000 == 0 {
            eprintln!("{i} mutations");
        }
    }
    let _ = panic::take_hook();
}

#[test]
fn mutated_programs_never_crash_the_compiler() {
    let iterations = env_or("FRATM_FUZZ_ITERATIONS", 20_000);
    let seed = env_or("FRATM_FUZZ_SEED", 0x5eed);
    // Debug builds use much more stack per call than release ones
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || run_mutations(iterations, seed))
        .unwrap()
        .join()
        .unwrap();
}
