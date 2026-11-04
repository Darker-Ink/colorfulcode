//! Measures highlighting throughput over the test samples.
//!
//! cargo run --release --example bench [filter]

use colorfulcode::{highlight_into, html, Language};
use std::time::{Duration, Instant};

fn main() {
    let filter = std::env::args().nth(1).unwrap_or_default();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples");
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    let mut tokens = Vec::new();
    let (mut total_bytes, mut total_time) = (0usize, Duration::ZERO);
    let mut rows = Vec::new();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.contains(&filter) {
            continue;
        }
        let lang = Language::find(name.split('.').next().unwrap()).unwrap();
        // Repeat the sample so each measurement covers about 1 MB.
        let sample = std::fs::read_to_string(&path).unwrap();
        let code = sample.repeat((1_000_000 / sample.len().max(1)).max(1));
        highlight_into(&code, lang, &mut tokens);
        let runs = 5;
        let start = Instant::now();
        for _ in 0..runs {
            highlight_into(&code, lang, &mut tokens);
        }
        let elapsed = start.elapsed() / runs;
        total_bytes += code.len();
        total_time += elapsed;
        rows.push((name, code.len() as f64 / elapsed.as_secs_f64() / 1e6));
    }
    rows.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (name, mbps) in &rows {
        println!("{name:<28} {mbps:>8.1} MB/s");
    }
    println!("{:<28} {:>8.1} MB/s", "overall", total_bytes as f64 / total_time.as_secs_f64() / 1e6);

    // HTML rendering cost on top of tokenizing.
    let code = std::fs::read_to_string(format!("{dir}/tsx.tsx")).unwrap().repeat(800);
    let lang = Language::find("tsx").unwrap();
    highlight_into(&code, lang, &mut tokens);
    let start = Instant::now();
    let out = html::render(&code, &tokens, &html::Options::default());
    let elapsed = start.elapsed();
    println!("html render: {:.1} MB/s ({} bytes out)", code.len() as f64 / elapsed.as_secs_f64() / 1e6, out.len());
}
