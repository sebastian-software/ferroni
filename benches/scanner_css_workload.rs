// Shared CSS Scanner workload for benchmark suites.
//
// The patterns are derived from common CSS TextMate grammar tokens and are
// intentionally heavy on `\w`/`[-\w]+` style character classes.

pub const CSS_PATTERNS: &[&str] = &[
    // Property names (Unicode-aware \w)
    r"[-a-zA-Z_][-\w]*",
    // Property values with word chars
    r"[-\w]+",
    // Numeric values
    r"[-+]?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][-+]?\d+)?",
    // Units
    r"(?:em|ex|ch|rem|vw|vh|vmin|vmax|cm|mm|in|px|pt|pc|%|s|ms|deg|rad|grad|turn|Hz|kHz|dpi|dpcm|dppx|fr)\b",
    // Hex colors
    r"#(?:[0-9a-fA-F]{3,4}){1,2}\b",
    // String (double-quoted)
    r#""[^"\\]*(?:\\.[^"\\]*)*""#,
    // String (single-quoted)
    r"'[^'\\]*(?:\\.[^'\\]*)*'",
    // URL function
    r"url\(",
    // CSS functions
    r"(?:calc|min|max|clamp|var|env|rgb|rgba|hsl|hsla|hwb|lab|lch|oklch|oklab|color|linear-gradient|radial-gradient|conic-gradient)\(",
    // Important
    r"!\s*important\b",
    // Selectors: class/id
    r"[.#][-\w]+",
    // Pseudo-classes/elements
    r"::?[-\w]+(?:\([^)]*\))?",
    // At-rules
    r"@[-\w]+",
    // Combinators and punctuation
    r"[>+~|]",
    r"[{}();,:]",
    // Attribute selectors
    r"\[[-\w]+(?:[~|^$*]?=)?",
    // Comments
    r"/\*",
    r"\*/",
    // Whitespace run (often matched)
    r"\s+",
    // Catch-all identifier (Unicode-aware)
    r"\w+",
];

pub const CSS_INPUT: &str = "\
.navbar-primary > .nav-item:first-child {\n\
  background-color: oklch(0.65 0.15 250);\n\
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;\n\
  margin: 0.5rem 1rem;\n\
  padding: 12px 24px;\n\
  border: 1px solid #e5e7eb;\n\
  transition: all 150ms cubic-bezier(0.4, 0, 0.2, 1);\n\
  --custom-property: var(--color-primary, #3b82f6);\n\
  width: calc(100% - 2rem);\n\
}\n\
\n\
@media (min-width: 768px) and (prefers-color-scheme: dark) {\n\
  .navbar-primary > .nav-item:hover {\n\
    background-color: oklch(0.45 0.12 250);\n\
    color: #f9fafb;\n\
    transform: translateY(-1px);\n\
    box-shadow: 0 4px 6px -1px rgb(0 0 0 / 0.1);\n\
  }\n\
}\n\
";

// CSS workload taken from issue #10 repro (tm-grammars-style patterns + sample lines).
// This is closer to ferriki's end-to-end warm-path than the synthetic CSS_INPUT string.
pub const CSS_TM_PATTERNS: &[&str] = &[
    // Identifier matching with non-ASCII ranges (hot in tm-grammars CSS).
    r"[-A-Z_a-z[^\x00-\x7F]](?:[-0-9A-Z_a-z[^\x00-\x7F]]|\\(?:\h{1,6}|.))*",
    // Custom property names.
    r"--[-A-Z_a-z[^\x00-\x7F]](?:[-0-9A-Z_a-z[^\x00-\x7F]]|\\(?:\h{1,6}|.))*",
    // Property value patterns.
    r"(?i)(?<![-\w])[-+]?(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)(?:(?<=[0-9])E[-+]?[0-9]+)?(?:(%)|(deg|grad|rad|turn|Hz|kHz|ch|cm|em|ex|fr|in|mm|mozmm|pc|pt|px|q|rem|s|ms|vw|vh|vmin|vmax|dpi|dpcm|dppx))?(?![-\w])",
    // Color hex values.
    r"(#)(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b",
    // Selector pseudo patterns.
    r"(?i)(:)(:*)(?:active|any-link|checked|default|disabled|empty|enabled|first|(?:first|last|only)-(?:child|of-type)|focus|focus-visible|focus-within|fullscreen|hover|indeterminate|in-range|invalid|lang|left|link|matches|not|nth-(?:last-)?(?:child|of-type)|optional|out-of-range|placeholder-shown|read-only|read-write|required|right|root|scope|target|unresolved|valid|visited)",
    // @-rule keywords.
    r#"\G((?!@charset)@\w+)|\G(\s+)|(@charset\S[^;]*)|(?<=@charset)( {2,}|\t+)|(?<=@charset )([^"";]+)|(""[^""]+)$|(?<="")([^;]+)"#,
    // Comment delimiters.
    r"/\*",
    r"\*/",
    // Strings.
    r#""([^"\\]|\\.)*""#,
    r"'([^'\\]|\\.)*'",
    // Whitespace and punctuation.
    r"\s+",
    r"[{}();:,]",
    // Important / URL / functions.
    r"!\s*important(?![-\w])",
    r"(?i)(?<![- \w])(url)(\()",
    r"(?i)(?<![- \w])(calc|rgba?|hsla?|var|min|max|clamp)(\()",
];

pub const CSS_TM_LINES: &[&str] = &[
    ".container > .item:hover {",
    "  background: linear-gradient(135deg, var(--primary), #8b5cf6);",
    "  transition: transform 0.2s ease;",
    "  max-width: 1200px;",
    "@media (max-width: 768px) {",
    "  --custom-property: 1rem;",
    "  color: rgba(255, 255, 255, 0.8);",
    "  font-family: 'Helvetica Neue', sans-serif;",
];
