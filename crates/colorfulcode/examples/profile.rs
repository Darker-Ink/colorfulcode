//! Highlights a sample many times; meant to be run under a profiler.
//!
//! cargo build --release --example profile && valgrind --tool=callgrind target/release/examples/profile tsx

fn main() {
    let id = std::env::args().nth(1).unwrap_or_else(|| "tsx".into());
    let lang = colorfulcode::Language::find(&id).expect("unknown language");
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples");
    let path = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().split('.').next() == Some(lang.id()))
        .expect("no sample");
    let code = std::fs::read_to_string(path).unwrap().repeat(200);
    let mut tokens = Vec::new();
    for _ in 0..5 {
        colorfulcode::highlight_into(&code, lang, &mut tokens);
    }
    println!("{} tokens", tokens.len());
}
