use colorfulcode::Language;
use std::collections::HashMap;

#[test]
fn names_are_unique() {
    let mut seen: HashMap<String, &str> = HashMap::new();
    let mut dupes = Vec::new();
    for lang in Language::all() {
        let names = std::iter::once(lang.id()).chain(lang.aliases().iter().copied());
        let exts = lang.extensions().iter().map(|e| format!(".{e}"));
        let files = lang.filenames().iter().map(|f| format!("file:{f}"));
        for key in names.map(str::to_string).chain(exts).chain(files) {
            let key = key.to_ascii_lowercase();
            if let Some(other) = seen.insert(key.clone(), lang.id()) {
                if other != lang.id() {
                    dupes.push(format!("{key} ({other}, {})", lang.id()));
                }
            }
        }
    }
    assert!(dupes.is_empty(), "duplicate names: {dupes:#?}");
}

#[test]
fn ids_are_lowercase() {
    for lang in Language::all() {
        assert_eq!(lang.id(), lang.id().to_ascii_lowercase());
        assert!(!lang.id().contains(' '));
    }
}

#[test]
fn lookups() {
    assert_eq!(Language::find("TS").unwrap().id(), "typescript");
    assert_eq!(Language::find(".rs").unwrap().id(), "rust");
    assert_eq!(Language::from_path("a/b/App.tsx").unwrap().id(), "tsx");
    assert_eq!(Language::from_content("#!/usr/bin/env python3\n").unwrap().id(), "python");
    assert_eq!(Language::from_content("#!/bin/bash\n").unwrap().id(), "bash");
    assert!(Language::find("definitely-not-a-language").is_none());
}
