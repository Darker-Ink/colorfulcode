//! Checks that `include/colorfulcode.h` matches the Rust side.

use colorfulcode::Kind;

const HEADER: &str = include_str!("../include/colorfulcode.h");
const SOURCE: &str = include_str!("../src/lib.rs");

/// Names of the functions `source` exports.
fn exported() -> Vec<&'static str> {
    SOURCE
        .lines()
        .filter_map(|line| line.split_once("extern \"C\" fn ")?.1.split_once('(').map(|(name, _)| name))
        .collect()
}

/// Names of the functions the header declares.
fn declared() -> Vec<&'static str> {
    HEADER
        .lines()
        .filter(|line| !line.starts_with([' ', '/', '*', '#']))
        .filter_map(|line| line.split_once('(')?.0.rsplit([' ', '*']).next())
        .filter(|name| name.starts_with("cc_"))
        .collect()
}

#[test]
fn functions_match() {
    let mut exported = exported();
    let mut declared = declared();
    assert!(exported.len() > 20);
    exported.sort_unstable();
    declared.sort_unstable();
    assert_eq!(exported, declared);
}

#[test]
fn constants_match() {
    let mut count = 0;
    for line in SOURCE.lines() {
        if let Some(rest) = line.strip_prefix("pub const ") {
            let (name, value) = rest.split_once(": u32 = ").unwrap();
            let value = value.trim_end_matches(';').replace("1 << ", "1u << ");
            assert!(HEADER.contains(&format!("#define {name} ({value})")), "{name} is missing or differs");
            count += 1;
        }
    }
    assert_eq!(count, 4);
}

#[test]
fn kinds_match() {
    for kind in Kind::ALL {
        let line = format!("CC_KIND_{} = {},", kind.name().to_uppercase(), kind as u8);
        assert!(HEADER.contains(&line), "missing `{line}`");
    }
    assert!(HEADER.contains(&format!("CC_KIND_COUNT = {}\n", Kind::COUNT)));
}
