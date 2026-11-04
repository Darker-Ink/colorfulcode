/// The category of a highlighted piece of source code.
///
/// Every token produced by the highlighter has exactly one `Kind`. The set is
/// intentionally small so that themes and native renderers only need to map a
/// handful of values to colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Kind {
    /// Anything without special meaning (identifiers, whitespace, prose).
    Text = 0,
    Comment,
    Keyword,
    String,
    /// Escape sequences inside strings such as `\n`.
    Escape,
    Number,
    /// Language constants such as `true`, `null` or `ALL_CAPS` names.
    Constant,
    Function,
    Type,
    Variable,
    Property,
    /// Built in functions and values such as `print` or `console`.
    Builtin,
    Operator,
    Punctuation,
    /// Markup tag names such as `div` in `<div>`.
    Tag,
    /// Markup attribute names.
    Attribute,
    /// Preprocessor directives, decorators, annotations and attributes.
    Meta,
    Regex,
    Heading,
    Emphasis,
    Strong,
    Link,
    Inserted,
    Deleted,
}

impl Kind {
    /// Number of kinds. Useful for building lookup tables indexed by `kind as usize`.
    pub const COUNT: usize = 24;

    /// Every kind, ordered by its numeric value.
    pub const ALL: [Kind; Kind::COUNT] = [
        Kind::Text,
        Kind::Comment,
        Kind::Keyword,
        Kind::String,
        Kind::Escape,
        Kind::Number,
        Kind::Constant,
        Kind::Function,
        Kind::Type,
        Kind::Variable,
        Kind::Property,
        Kind::Builtin,
        Kind::Operator,
        Kind::Punctuation,
        Kind::Tag,
        Kind::Attribute,
        Kind::Meta,
        Kind::Regex,
        Kind::Heading,
        Kind::Emphasis,
        Kind::Strong,
        Kind::Link,
        Kind::Inserted,
        Kind::Deleted,
    ];

    /// Lowercase name of the kind, for example `"keyword"`.
    pub const fn name(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Comment => "comment",
            Kind::Keyword => "keyword",
            Kind::String => "string",
            Kind::Escape => "escape",
            Kind::Number => "number",
            Kind::Constant => "constant",
            Kind::Function => "function",
            Kind::Type => "type",
            Kind::Variable => "variable",
            Kind::Property => "property",
            Kind::Builtin => "builtin",
            Kind::Operator => "operator",
            Kind::Punctuation => "punctuation",
            Kind::Tag => "tag",
            Kind::Attribute => "attribute",
            Kind::Meta => "meta",
            Kind::Regex => "regex",
            Kind::Heading => "heading",
            Kind::Emphasis => "emphasis",
            Kind::Strong => "strong",
            Kind::Link => "link",
            Kind::Inserted => "inserted",
            Kind::Deleted => "deleted",
        }
    }

    /// CSS class used by the HTML renderer, for example `"cc-keyword"`.
    pub const fn class(self) -> &'static str {
        match self {
            Kind::Text => "cc-text",
            Kind::Comment => "cc-comment",
            Kind::Keyword => "cc-keyword",
            Kind::String => "cc-string",
            Kind::Escape => "cc-escape",
            Kind::Number => "cc-number",
            Kind::Constant => "cc-constant",
            Kind::Function => "cc-function",
            Kind::Type => "cc-type",
            Kind::Variable => "cc-variable",
            Kind::Property => "cc-property",
            Kind::Builtin => "cc-builtin",
            Kind::Operator => "cc-operator",
            Kind::Punctuation => "cc-punctuation",
            Kind::Tag => "cc-tag",
            Kind::Attribute => "cc-attribute",
            Kind::Meta => "cc-meta",
            Kind::Regex => "cc-regex",
            Kind::Heading => "cc-heading",
            Kind::Emphasis => "cc-emphasis",
            Kind::Strong => "cc-strong",
            Kind::Link => "cc-link",
            Kind::Inserted => "cc-inserted",
            Kind::Deleted => "cc-deleted",
        }
    }

    /// Kind to borrow a style from when a theme does not define this one.
    pub const fn fallback(self) -> Option<Kind> {
        match self {
            Kind::Text => None,
            Kind::Escape => Some(Kind::Constant),
            Kind::Constant => Some(Kind::Number),
            Kind::Builtin => Some(Kind::Function),
            Kind::Attribute => Some(Kind::Property),
            Kind::Property => Some(Kind::Variable),
            Kind::Tag => Some(Kind::Keyword),
            Kind::Meta => Some(Kind::Keyword),
            Kind::Regex => Some(Kind::String),
            Kind::Heading => Some(Kind::Keyword),
            Kind::Link => Some(Kind::String),
            Kind::Inserted => Some(Kind::String),
            Kind::Deleted => Some(Kind::Keyword),
            Kind::Punctuation => Some(Kind::Operator),
            _ => Some(Kind::Text),
        }
    }

    /// Looks up a kind by its numeric value.
    pub const fn from_u8(value: u8) -> Option<Kind> {
        if (value as usize) < Kind::COUNT {
            Some(Kind::ALL[value as usize])
        } else {
            None
        }
    }

    /// Looks up a kind by its lowercase name.
    pub fn from_name(name: &str) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|k| k.name() == name)
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
