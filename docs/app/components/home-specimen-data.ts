/**
 * Facts the home page derives from the real output in home-specimens.json
 * (written by examples/website_homepage.rs; never edited by hand).
 */
import specimens from "../data/home-specimens.json";

export { specimens };

export type SpecimenEngine = "c" | "fancy_regex" | "pcre2" | "regex" | "rust";

/** The engines the home page compares; Ruby's Onigmo is on the workload page. */
export const FEATURE_ENGINES: Array<{ id: SpecimenEngine; label: string; language: string }> = [
  { id: "rust", label: "Ferroni", language: "Rust" },
  { id: "c", label: "Oniguruma", language: "C, the original" },
  { id: "pcre2", label: "PCRE2", language: "C" },
  { id: "fancy_regex", label: "fancy-regex", language: "Rust" },
  { id: "regex", label: "regex", language: "Rust" },
];

const { features } = specimens;
const completeRust = FEATURE_ENGINES.filter(
  (engine) => engine.language === "Rust" && features.every((feature) => feature.support[engine.id]),
);
if (completeRust.length !== 1 || completeRust[0].id !== "rust") {
  throw new Error("Revise the syntax section: Ferroni is no longer the one complete Rust engine.");
}

const NUMBERS = [
  "no",
  "one",
  "two",
  "three",
  "four",
  "five",
  "six",
  "seven",
  "eight",
  "nine",
  "ten",
];
/** A count in words, as the copy states it. */
export const spelled = (count: number) => NUMBERS.at(count) ?? String(count);

/** How many selected features there are, and how many Ferroni passes. */
export const featureCount = {
  total: features.length,
  ferroni: features.filter((feature) => feature.support.rust).length,
};

const DEPTH: Record<string, number> = { "(": 1, ")": -1 };

/** The index ranges of a pattern's top-level groups; escaped parentheses do not count. */
function groupRanges(pattern: string) {
  const ranges: Array<[number, number]> = [];
  let depth = 0;
  let open = 0;
  for (const { 0: token, index } of pattern.matchAll(/\\.|[()]/gu)) {
    const before = depth;
    depth += DEPTH[token] ?? 0;
    if (before === 0 && depth === 1) open = index;
    if (before === 1 && depth === 0) ranges.push([open, index + 1]);
  }
  return ranges;
}

export type PatternToken = { start: number; text: string; group?: string; label: string };

/** The date pattern split at its top-level groups, each named group labeled. */
export const dateTokens: PatternToken[] = (() => {
  const { pattern } = specimens.date;
  const cuts = [0, ...groupRanges(pattern).flat(), pattern.length];
  const starts = [...new Set(cuts)].filter((cut) => cut < pattern.length);
  return starts.map((start, index) => {
    const text = pattern.slice(start, starts.at(index + 1) ?? pattern.length);
    const named = /^\(\?<([a-z]+)>/.exec(text)?.[1];
    if (named !== undefined) return { start, text, group: named, label: named };
    if (text.startsWith("(?<=")) return { start, text, group: "look", label: "look behind" };
    return { start, text, label: "" };
  });
})();

export type TextPart = { start: number; text: string; group?: string; look?: boolean };

/** The date text with the lookbehind's label and every capture marked by its group. */
export const dateText: TextPart[] = (() => {
  const { input, captures } = specimens.date;
  const parts: TextPart[] = [
    { start: 0, text: input.slice(0, captures[0].start), group: "look", look: true },
  ];
  let position = captures[0].start;
  for (const capture of captures) {
    if (capture.start > position) {
      parts.push({ start: position, text: input.slice(position, capture.start) });
    }
    parts.push({ start: capture.start, text: capture.value, group: capture.name });
    position = capture.end;
  }
  if (position < input.length) parts.push({ start: position, text: input.slice(position) });
  return parts;
})();

/** The Rust that runs the walkthrough's scanner, built from its patterns. */
export const scannerSource = [
  "use ferroni::prelude::*;",
  "",
  "let mut scanner = Scanner::new(&[",
  ...specimens.scanner.patterns.map(
    (pattern) => `    ${pattern.includes('"') ? `r#"${pattern}"#` : `r"${pattern}"`},`,
  ),
  "])?;",
  "",
  "let mut from = 0;",
  "while let Some(m) = scanner.find_next_match(line, from, ScannerFindOptions::NONE) {",
  "    let hit = &m.captures()[0];",
  "    // m.index is the winning pattern, hit.start..hit.end the match",
  "    from = hit.end;",
  "}",
].join("\n");
