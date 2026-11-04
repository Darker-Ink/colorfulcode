//! How a native app would draw highlighted code: walk the lines, look up the
//! theme style of each token, and hand text plus color to your UI toolkit.
//! Here the "toolkit" is a terminal.
//!
//! cargo run --example native

use colorfulcode::{highlight, Language, Theme};

fn main() {
    let code = r#"fn greet(name: &str) -> String {
    format!("Hello, {name}!") // say hi
}"#;
    let theme = Theme::find("tokyo-night").unwrap();
    let highlighted = highlight(code, Language::find("rust").unwrap());

    for (number, line) in highlighted.lines().enumerate() {
        print!("{:>3}  ", number + 1);
        for (text, kind) in line {
            let style = theme.style(kind);
            // In a GUI this is where you would create an attributed string,
            // a styled span or a text run with `style.color`, `style.bold`...
            let c = style.color;
            let italic = if style.italic { "\x1b[3m" } else { "" };
            print!("{italic}\x1b[38;2;{};{};{}m{text}\x1b[0m", c.r, c.g, c.b);
        }
        println!();
    }
}
