#![no_main]

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::oniguruma::{
    ONIG_OPTION_DONT_CAPTURE_GROUP, ONIG_OPTION_EXTEND, ONIG_OPTION_FIND_LONGEST,
    ONIG_OPTION_IGNORECASE, ONIG_OPTION_MULTILINE, ONIG_OPTION_NONE, ONIG_OPTION_SINGLELINE,
    OnigOptionType, OnigSyntaxType,
};
use ferroni::regcomp::onig_new;
use ferroni::regsyntax::{
    OnigSyntaxJava, OnigSyntaxOniguruma, OnigSyntaxPerl_NG, OnigSyntaxPosixExtended,
    OnigSyntaxPython, OnigSyntaxRuby,
};
use libfuzzer_sys::fuzz_target;

// Grammar patterns are small; this keeps a single mutation from spending the
// whole run inside one enormous pattern.
const MAX_PATTERN_BYTES: usize = 4 * 1024;

/// The syntaxes a TextMate-style consumer can reach.
static SYNTAXES: &[&OnigSyntaxType] = &[
    &OnigSyntaxOniguruma,
    &OnigSyntaxRuby,
    &OnigSyntaxPerl_NG,
    &OnigSyntaxJava,
    &OnigSyntaxPython,
    &OnigSyntaxPosixExtended,
];

static OPTIONS: &[OnigOptionType] = &[
    ONIG_OPTION_NONE,
    ONIG_OPTION_IGNORECASE,
    ONIG_OPTION_EXTEND,
    ONIG_OPTION_MULTILINE,
    ONIG_OPTION_SINGLELINE,
    ONIG_OPTION_FIND_LONGEST,
    ONIG_OPTION_DONT_CAPTURE_GROUP,
];

// The parser and the compiler must reject a malformed pattern with an error
// rather than panicking, overflowing, or running away, whatever bytes it is
// handed. The first two bytes pick a syntax and an option set so one corpus
// entry can reach every dialect.
fuzz_target!(|data: &[u8]| {
    if data.len() < 2 || data.len() > MAX_PATTERN_BYTES + 2 {
        return;
    }

    let syntax = SYNTAXES[data[0] as usize % SYNTAXES.len()];
    let option = OPTIONS[data[1] as usize % OPTIONS.len()];
    let pattern = &data[2..];

    let _ = onig_new(pattern, option, &ONIG_ENCODING_UTF8, syntax);
});
