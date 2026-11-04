//! Snapshot tests: every sample is highlighted and compared with the stored
//! output in `tests/snapshots`. Run with `UPDATE_SNAPSHOTS=1` to accept changes.

use colorfulcode::{highlight, Kind, Language};
use std::fmt::Write;
use std::path::Path;

fn render(code: &str, lang: Language) -> String {
    let mut out = String::new();
    for (text, kind) in highlight(code, lang).iter() {
        if kind == Kind::Text && text.trim().is_empty() {
            continue;
        }
        let _ = writeln!(out, "{:<12}{:?}", kind.name(), text);
    }
    out
}

#[test]
fn snapshots() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    let mut failures = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(root.join("samples")).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let id = name.split('.').next().unwrap();
        let lang = Language::find(id).unwrap_or_else(|| panic!("sample {name} has no matching language"));
        let code = std::fs::read_to_string(&path).unwrap();
        let actual = render(&code, lang);
        let snap = root.join("snapshots").join(format!("{name}.txt"));
        if update {
            std::fs::write(&snap, &actual).unwrap();
            continue;
        }
        match std::fs::read_to_string(&snap) {
            Ok(expected) if expected == actual => {}
            Ok(_) => failures.push(format!("{name}: output changed")),
            Err(_) => failures.push(format!("{name}: no snapshot")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}\nRun with UPDATE_SNAPSHOTS=1 after reviewing the changes.");
}
