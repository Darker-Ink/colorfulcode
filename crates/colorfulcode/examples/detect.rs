//! Runs language detection on every test sample and reports the accuracy
//! and the time it takes.
//!
//! cargo run --release --example detect [-- --verbose]

use colorfulcode::Language;
use std::time::Instant;

fn main() {
    let verbose = std::env::args().any(|a| a == "--verbose");
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples");
    let mut samples: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name.split('.').next().unwrap().to_string(), std::fs::read_to_string(&p).unwrap())
        })
        .collect();
    samples.sort();

    let start = Instant::now();
    let first = Language::detect(&samples[0].1);
    println!("first call (builds the word tables): {:.2} ms", start.elapsed().as_secs_f64() * 1e3);
    std::hint::black_box(first);

    let mut right = 0;
    for (id, code) in &samples {
        let found = Language::detect(code).map_or("none", |l| l.id());
        // JSX may come out as JavaScript, which lexes it the same way.
        if found == id || (id == "text" && found == "none") || (id == "jsx" && found == "javascript") {
            right += 1;
        } else if verbose {
            println!("  {id} -> {found}");
        }
    }
    println!("{right}/{} samples detected ({:.1}%)", samples.len(), 100.0 * right as f64 / samples.len() as f64);

    let runs = 20;
    let bytes: usize = samples.iter().map(|(_, code)| code.len().min(4096)).sum();
    let start = Instant::now();
    for _ in 0..runs {
        for (_, code) in &samples {
            std::hint::black_box(Language::detect(code));
        }
    }
    let per_sample = start.elapsed().as_secs_f64() / (runs * samples.len()) as f64;
    let average = bytes / samples.len();
    println!("{:.1} us per sample (average {average} bytes scanned)", per_sample * 1e6);

    // The longest input detection looks at: 4 KB.
    let rust = &samples.iter().find(|(id, _)| id == "rust").unwrap().1;
    let big = rust.repeat(4096 / rust.len() + 1);
    let runs = 1000;
    let start = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(Language::detect(&big));
    }
    println!("{:.1} us for 4 KB of Rust", start.elapsed().as_secs_f64() * 1e6 / runs as f64);
}
