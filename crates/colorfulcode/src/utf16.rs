use crate::Token;

/// Converts UTF-8 byte offsets to UTF-16 code unit offsets.
pub fn convert(code: &str, tokens: &[Token]) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    convert_with(code, tokens, |t| out.push(t));
    out
}

/// Calls `f` with each token converted to UTF-16 offsets.
pub fn convert_with(code: &str, tokens: &[Token], mut f: impl FnMut(Token)) {
    let bytes = code.as_bytes();
    // Pure ASCII input needs no conversion.
    if bytes.is_ascii() {
        tokens.iter().copied().for_each(f);
        return;
    }
    let mut units = 0u32;
    for t in tokens {
        let start = units;
        for &b in &bytes[t.start as usize..t.end as usize] {
            // Count leading bytes; 4 byte sequences become surrogate pairs.
            if b & 0xC0 != 0x80 {
                units += 1;
            }
            if b >= 0xF0 {
                units += 1;
            }
        }
        f(Token { start, end: units, kind: t.kind });
    }
}
