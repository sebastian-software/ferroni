#![no_main]

use ferroni::scanner::{Scanner, ScannerFindOptions};
use libfuzzer_sys::fuzz_target;

const PATTERN_SEPARATOR: u8 = 0x00;
const MAX_PATTERNS: usize = 8;
const MAX_INPUT_BYTES: usize = 16 * 1024;
// The scanner reports one match per call, so a haystack of N bytes cannot
// yield more than N + 1 matches even with zero-width matches.
const MAX_STEPS: usize = 512;

// The Scanner is what a TextMate grammar host drives, with patterns and text
// it does not control. Building one from arbitrary patterns and walking it
// across arbitrary text must not panic, and every reported match has to stay
// inside the haystack and on a character boundary.
fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    // Everything up to the last separator is the pattern list; the rest is the
    // text to scan.
    let Some(split) = data.iter().rposition(|byte| *byte == PATTERN_SEPARATOR) else {
        return;
    };
    let (pattern_bytes, text_bytes) = data.split_at(split);

    let patterns: Vec<String> = pattern_bytes
        .split(|byte| *byte == PATTERN_SEPARATOR)
        .take(MAX_PATTERNS)
        .map(|pattern| String::from_utf8_lossy(pattern).into_owned())
        .collect();
    if patterns.is_empty() {
        return;
    }
    let patterns: Vec<&str> = patterns.iter().map(String::as_str).collect();

    let Ok(mut scanner) = Scanner::new(&patterns) else {
        return;
    };

    let text = String::from_utf8_lossy(&text_bytes[1..]);
    let mut position = 0;
    for _ in 0..MAX_STEPS {
        let Some(found) = scanner.find_next_match(&text, position, ScannerFindOptions::NONE) else {
            break;
        };

        assert!(
            found.index < patterns.len(),
            "match reports pattern {} of {}",
            found.index,
            patterns.len()
        );
        let whole = &found.capture_indices[0];
        assert!(
            whole.start <= whole.end && whole.end <= text.len(),
            "match spans {}..{} of a {} byte text",
            whole.start,
            whole.end,
            text.len()
        );
        assert!(
            text.is_char_boundary(whole.start) && text.is_char_boundary(whole.end),
            "match {}..{} is not on a character boundary",
            whole.start,
            whole.end
        );

        // Zero-width matches are legal, so step past them to keep making
        // progress instead of scanning the same position forever.
        position = if whole.end > position {
            whole.end
        } else {
            let mut next = position + 1;
            while next < text.len() && !text.is_char_boundary(next) {
                next += 1;
            }
            next
        };
        if position > text.len() {
            break;
        }
    }
});
