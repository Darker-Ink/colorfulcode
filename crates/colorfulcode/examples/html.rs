//! Renders a snippet as HTML, both with inline styles and with class names.
//!
//! cargo run --example html

use colorfulcode::{highlight, html, Language, Theme};

fn main() {
    let code = "const greet = (name) => `Hello, ${name}!`;";
    let highlighted = highlight(code, Language::find("js").unwrap());

    // Inline styles: no stylesheet needed.
    let theme = Theme::find("dracula").unwrap();
    println!("{}\n", highlighted.to_html(&html::Options { theme: Some(theme), wrap: true, ..Default::default() }));

    // Class names plus a stylesheet generated from any theme.
    println!("<style>{}</style>", theme.to_css(".cc"));
    println!("{}", highlighted.to_html(&html::Options { wrap: true, ..Default::default() }));
}
