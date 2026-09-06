#![no_main]

use ferroni::scanner::{Scanner, ScannerFindOptions};
use libfuzzer_sys::fuzz_target;

const PATTERN_SEPARATOR: u8 = 0x00;
const MAX_PATTERNS: usize = 8;
const MAX_INPUT_BYTES: usize = 16 * 1024;
// The scanner reports one match per call, so a haystack of N bytes cannot
// yield more than N + 1 matches even with zero-width matches. The walk gets
// that many steps, capped so a dense pattern over a long text stays bounded;
// the input-derived start offset reaches the later positions instead.
const MAX_STEPS: usize = 4096;

// The Scanner is what a TextMate grammar host drives, with patterns and text
// it does not control. Building one from arbitrary patterns and walking it
// across arbitrary text must not panic, and every reported match has to stay
// inside the haystack and on a character boundary.
fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    // Everything up to the last separator is the pattern list. After it come
    // two bytes that pick the start position, then the text to scan.
    let Some(split) = data.iter().rposition(|byte| *byte == PATTERN_SEPARATOR) else {
        return;
    };
    let (pattern_bytes, rest) = data.split_at(split);
    let [_, offset_low, offset_high, text_bytes @ ..] = rest else {
        return;
    };

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

    let text = String::from_utf8_lossy(text_bytes);

    // A host resumes the scanner from wherever the previous token ended, so
    // the walk starts at an input-chosen position rather than always at 0.
    // Any offset up to and including the text length is legal; snap it
    // forward to a character boundary.
    let mut position = u16::from_le_bytes([*offset_low, *offset_high]) as usize % (text.len() + 1);
    while !text.is_char_boundary(position) {
        position += 1;
    }

    let steps = (text.len() + 1).min(MAX_STEPS);
    for _ in 0..steps {
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
