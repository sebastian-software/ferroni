// Shared CSS Scanner workload for benchmark suites.
//
// The patterns are derived from common CSS TextMate grammar tokens and are
// intentionally heavy on `\w`/`[-\w]+` style character classes.

#[allow(dead_code)]
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
