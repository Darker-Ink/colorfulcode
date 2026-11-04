//! `colorfulcode`: prints files with syntax highlighting.

mod args;

use args::{Action, Args, ColorChoice};
use colorfulcode::{ansi, highlight, html, Kind, Language, Theme};
use std::io::{self, BufWriter, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = match args::parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("colorfulcode: {err}\nTry 'colorfulcode --help' for more information.");
            return ExitCode::from(2);
        }
    };
    let mut out = BufWriter::new(io::stdout().lock());
    match run(&args, &mut out).and_then(|ok| out.flush().map(|()| ok)) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        // The reader went away, for example `colorfulcode file | head`.
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("colorfulcode: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Runs the action. Returns `Ok(false)` when a file could not be read.
fn run(args: &Args, out: &mut impl Write) -> io::Result<bool> {
    let theme = args.theme.unwrap_or(&Theme::all()[0]);
    let painter = Painter { theme, mode: use_color(args.color).then(ansi::ColorMode::detect) };
    match args.action {
        Action::Highlight => return print_files(args, &painter, out),
        Action::ListLanguages => list_languages(out)?,
        Action::ListThemes => list_themes(painter.mode, out)?,
        Action::Help => write!(out, "colorfulcode {}\n{}", env!("CARGO_PKG_VERSION"), args::HELP)?,
        Action::Version => writeln!(out, "colorfulcode {}", env!("CARGO_PKG_VERSION"))?,
    }
    Ok(true)
}

fn use_color(choice: ColorChoice) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            io::stdout().is_terminal() && std::env::var_os("NO_COLOR").filter(|v| !v.is_empty()).is_none()
        }
    }
}

/// Paints tokens for the terminal.
struct Painter<'t> {
    theme: &'t Theme,
    /// `None` when colors are off.
    mode: Option<ansi::ColorMode>,
}

impl Painter<'_> {
    fn push(&self, buf: &mut String, text: &str, kind: Kind) {
        let style = self.theme.style(kind);
        match self.mode {
            // Plain text keeps the terminal's own foreground color.
            Some(mode) if style != self.theme.style(Kind::Text) => ansi::paint(buf, text, style, mode),
            _ => buf.push_str(text),
        }
    }
}

fn print_files(args: &Args, painter: &Painter, out: &mut impl Write) -> io::Result<bool> {
    let stdin = [PathBuf::from("-")];
    let files = if args.files.is_empty() { &stdin[..] } else { &args.files };
    let mut ok = true;
    let mut first = true;
    for path in files {
        let code = match read(path) {
            Ok(code) => code,
            Err(err) => {
                out.flush()?;
                eprintln!("colorfulcode: {}: {err}", path.display());
                ok = false;
                continue;
            }
        };
        let language = args
            .language
            .or_else(|| Language::from_path(&path.to_string_lossy()))
            .or_else(|| Language::from_content(&code))
            .unwrap_or_else(Language::plain);
        if args.html {
            let options =
                html::Options { theme: args.theme, wrap: true, lines: args.line_numbers, ..Default::default() };
            writeln!(out, "{}", highlight(&code, language).to_html(&options))?;
            continue;
        }
        if files.len() > 1 {
            let name = if path == Path::new("-") { "<stdin>".into() } else { path.to_string_lossy() };
            let (bold, reset) = if painter.mode.is_some() { ("\x1b[1m", "\x1b[0m") } else { ("", "") };
            if !first {
                writeln!(out)?;
            }
            writeln!(out, "{bold}==> {name} <=={reset}")?;
        }
        first = false;
        print_code(&code, language, painter, args.line_numbers, out)?;
    }
    Ok(ok)
}

/// Reads a file, or stdin for `-`. Invalid UTF-8 is replaced.
fn read(path: &Path) -> io::Result<String> {
    let bytes = if path == Path::new("-") {
        let mut bytes = Vec::new();
        io::stdin().lock().read_to_end(&mut bytes)?;
        bytes
    } else {
        std::fs::read(path)?
    };
    if bytes.len() > u32::MAX as usize {
        return Err(io::Error::other("file is too large"));
    }
    Ok(String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

fn print_code(
    code: &str,
    language: Language,
    painter: &Painter,
    line_numbers: bool,
    out: &mut impl Write,
) -> io::Result<()> {
    if painter.mode.is_none() && !line_numbers {
        out.write_all(code.as_bytes())?;
        if !code.is_empty() && !code.ends_with('\n') {
            out.write_all(b"\n")?;
        }
        return Ok(());
    }
    // `Highlighted::lines` also yields the empty line after a final newline.
    let count = code.lines().count();
    let width = count.to_string().len();
    let mut buf = String::new();
    for (index, line) in highlight(code, language).lines().take(count).enumerate() {
        buf.clear();
        if line_numbers {
            let number = index + 1;
            match painter.mode {
                Some(_) => buf.push_str(&format!("\x1b[2m{number:>width$} │\x1b[0m ")),
                None => buf.push_str(&format!("{number:>width$} │ ")),
            }
        }
        for (text, kind) in line {
            painter.push(&mut buf, text, kind);
        }
        buf.push('\n');
        out.write_all(buf.as_bytes())?;
    }
    Ok(())
}

fn list_languages(out: &mut impl Write) -> io::Result<()> {
    let all = Language::all();
    let id_width = all.iter().map(|l| l.id().len()).max().unwrap_or(0);
    let name_width = all.iter().map(|l| l.name().len()).max().unwrap_or(0);
    for language in all {
        let extensions: Vec<String> = language.extensions().iter().map(|e| format!(".{e}")).collect();
        let line = format!("{:id_width$}  {:name_width$}  {}", language.id(), language.name(), extensions.join(" "));
        writeln!(out, "{}", line.trim_end())?;
    }
    Ok(())
}

/// Lists theme names, each followed by a highlighted sample when colors are on.
fn list_themes(mode: Option<ansi::ColorMode>, out: &mut impl Write) -> io::Result<()> {
    const SAMPLE: &str = "fn main() { let n: u32 = 42; println!(\"hi {n}\"); } // done";
    let rust = Language::find("rust").unwrap_or_else(Language::plain);
    let sample = highlight(SAMPLE, rust);
    let width = Theme::all().iter().map(|t| t.name.len()).max().unwrap_or(0);
    for theme in Theme::all() {
        if mode.is_none() {
            writeln!(out, "{}", theme.name)?;
            continue;
        }
        let painter = Painter { theme, mode };
        let mut line = format!("{:width$}  ", theme.name);
        for (text, kind) in sample.iter() {
            painter.push(&mut line, text, kind);
        }
        writeln!(out, "{line}")?;
    }
    Ok(())
}
