//! Command line argument parsing.

use colorfulcode::{Language, Theme};
use std::ffi::OsString;
use std::path::PathBuf;

pub const HELP: &str = "\
Print files with syntax highlighting.

Usage: colorfulcode [options] [file]...

With no file, or when file is -, read standard input. The language comes from
the file name, a shebang, or else a guess from the content (-l text turns
highlighting off).

Options:
  -l, --language <name>  Force a language instead of detecting it
  -t, --theme <name>     Color theme (default: colorful)
  -n, --line-numbers     Show line numbers
      --color <when>     Use colors: auto, always or never (default: auto)
      --html             Print HTML with class names, or inline styles with --theme
      --list-languages   List the supported languages
      --list-themes      List the themes with a preview
  -h, --help             Print this help
  -V, --version          Print the version
";

/// What the program should do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    #[default]
    Highlight,
    ListLanguages,
    ListThemes,
    Help,
    Version,
}

/// When to emit color escape codes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorChoice {
    /// Only when stdout is a terminal and `NO_COLOR` is unset.
    #[default]
    Auto,
    Always,
    Never,
}

/// Parsed command line.
#[derive(Debug, Default)]
pub struct Args {
    pub action: Action,
    /// Files to print. `-` means stdin, and so does an empty list.
    pub files: Vec<PathBuf>,
    pub language: Option<Language>,
    /// The theme given with `--theme`, if any.
    pub theme: Option<&'static Theme>,
    pub line_numbers: bool,
    pub color: ColorChoice,
    pub html: bool,
}

/// Parses the arguments, without the program name.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Args, String> {
    let mut args = args.into_iter();
    let mut out = Args::default();
    let mut only_files = false;
    while let Some(arg) = args.next() {
        let flag = match arg.to_str() {
            Some(s) if !only_files && s.starts_with('-') && s != "-" => s,
            _ => {
                out.files.push(arg.into());
                continue;
            }
        };
        // `--name=value` carries its value inline.
        let (flag, mut inline) = match flag.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_owned())),
            _ => (flag, None),
        };
        let mut value = || match inline.take() {
            Some(v) => Ok(v),
            None => args
                .next()
                .ok_or_else(|| format!("option '{flag}' needs a value"))?
                .into_string()
                .map_err(|_| format!("the value of '{flag}' is not valid UTF-8")),
        };
        match flag {
            "--" => only_files = true,
            "-l" | "--language" => {
                let name = value()?;
                let language = Language::find(&name);
                out.language = Some(language.ok_or(format!("unknown language '{name}', see --list-languages"))?);
            }
            "-t" | "--theme" => {
                let name = value()?;
                out.theme = Some(Theme::find(&name).ok_or(format!("unknown theme '{name}', see --list-themes"))?);
            }
            "--color" => {
                out.color = match value()?.as_str() {
                    "auto" => ColorChoice::Auto,
                    "always" => ColorChoice::Always,
                    "never" => ColorChoice::Never,
                    other => return Err(format!("invalid --color '{other}', expected auto, always or never")),
                }
            }
            "-n" | "--line-numbers" => out.line_numbers = true,
            "--html" => out.html = true,
            "--list-languages" => out.action = Action::ListLanguages,
            "--list-themes" => out.action = Action::ListThemes,
            "-h" | "--help" => out.action = Action::Help,
            "-V" | "--version" => out.action = Action::Version,
            _ => return Err(format!("unknown option '{flag}'")),
        }
        if inline.is_some() {
            return Err(format!("option '{flag}' takes no value"));
        }
        if matches!(out.action, Action::Help | Action::Version) {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(args: &[&str]) -> Result<Args, String> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn options_and_files() {
        let args = parse_str(&["-n", "--theme=nord", "-l", "rust", "a.rs", "-", "--", "-b"]).unwrap();
        assert_eq!(args.action, Action::Highlight);
        assert!(args.line_numbers);
        assert_eq!(args.theme.unwrap().name, "nord");
        assert_eq!(args.language.unwrap().id(), "rust");
        assert_eq!(args.files, ["a.rs", "-", "-b"].map(PathBuf::from));
    }

    #[test]
    fn errors() {
        assert!(parse_str(&["--bogus"]).is_err());
        assert!(parse_str(&["--theme"]).is_err());
        assert!(parse_str(&["--theme", "nope"]).is_err());
        assert!(parse_str(&["--color=sometimes"]).is_err());
        assert!(parse_str(&["--html=yes"]).is_err());
    }

    #[test]
    fn help_wins() {
        assert_eq!(parse_str(&["--help", "--bogus"]).unwrap().action, Action::Help);
        assert_eq!(parse_str(&["-V"]).unwrap().action, Action::Version);
    }
}
