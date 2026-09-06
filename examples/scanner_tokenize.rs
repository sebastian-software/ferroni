//! Tokenizing a line with the multi-pattern Scanner API.
//!
//! The Scanner is the operation behind TextMate-based syntax highlighting:
//! many patterns are matched against the same line, and the leftmost match
//! wins. It is API-compatible with vscode-oniguruma, so the same loop drives
//! vscode-textmate and Shiki grammars.
//!
//! Run with:
//! cargo run --example scanner_tokenize

use ferroni::prelude::*;

/// The scope name reported for each pattern, by pattern index.
const SCOPES: &[&str] = &["keyword", "string", "comment", "number", "identifier"];

fn main() -> Result<(), RegexError> {
    let mut scanner = Scanner::new(&[
        r"\b(?:const|let|var|function|return)\b",
        r#""[^"]*""#,
        r"//.*$",
        r"\b\d+(?:\.\d+)?\b",
        r"\b[A-Za-z_]\w*\b",
    ])?;

    let line = r#"const label = "answer" + 42 // "not" a string"#;

    // Walk the line the way a highlighter does: find the next match, emit the
    // token, then continue from the end of that match.
    let mut position = 0;
    while let Some(m) = scanner.find_next_match(line, position, ScannerFindOptions::NONE) {
        let whole = &m.capture_indices[0];
        println!(
            "{:>10}  {:>3}..{:<3} {}",
            SCOPES[m.index],
            whole.start,
            whole.end,
            &line[whole.start..whole.end],
        );

        // Guard against zero-width matches, which would loop forever.
        position = if whole.end > whole.start {
            whole.end
        } else {
            whole.end + 1
        };
    }

    // vscode-textmate and Shiki address text in UTF-16 code units. Wrap the
    // line in an `OnigString` and the offsets come back in the same units.
    let mut scanner = Scanner::new(&["world"])?;
    let text = OnigString::new("hello 🌍 world");
    let m = scanner
        .find_next_match_utf16(&text, 0, ScannerFindOptions::NONE)
        .expect("`world` is in the text");
    println!(
        "\nUTF-16 offsets: {}..{} (the emoji counts as two units)",
        m.capture_indices[0].start, m.capture_indices[0].end,
    );

    Ok(())
}
