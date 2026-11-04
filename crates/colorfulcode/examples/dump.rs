//! Prints the tokens of a file, one per line. Handy when working on a language.
//!
//! cargo run --example dump -- <file> [language]

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: dump <file> [language]");
    let code = std::fs::read_to_string(&path).expect("could not read file");
    let language = match args.next() {
        Some(name) => colorfulcode::Language::find(&name).expect("unknown language"),
        None => colorfulcode::Language::from_path(&path).unwrap_or_else(colorfulcode::Language::plain),
    };
    for (text, kind) in colorfulcode::highlight(&code, language).iter() {
        if kind != colorfulcode::Kind::Text || !text.trim().is_empty() {
            println!("{:<12} {:?}", kind.name(), text);
        }
    }
}
