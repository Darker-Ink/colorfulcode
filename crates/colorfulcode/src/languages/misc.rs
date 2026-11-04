//! Plain text and languages that do not fit elsewhere.

use super::{Definition, Engine};
use crate::lexer::Ctx;

pub static LANGUAGES: &[&Definition] = &[&PLAIN];

pub static PLAIN: Definition = Definition::new("text", "Plain Text", Engine::Custom(|_: &mut Ctx| {}))
    .aliases(&["plain", "plaintext", "txt", "none"])
    .extensions(&["txt", "text"]);
