// cspell:ignore onigmo erroraware regexr
import data from "../data/production-comparison.json";
import regexrResults from "../data/regexr-comparison.json";
import tokenizerResults from "../data/tokenizer-comparison.json";

export { data };
export type Dataset = {
  cases: CaseEntry[];
  platforms: Platform[];
  run: Pick<typeof data.run, "created_at" | "head_sha" | "html_url">;
};
function publishedDataset(dataset: Dataset): Dataset | undefined {
  return dataset.platforms.length > 0 ? dataset : undefined;
}
export const regexrData = publishedDataset(regexrResults);
export const tokenizerData = publishedDataset(tokenizerResults);

export const engineNames: Record<string, string> = {
  rust_erroraware: "Ferroni error-aware API",
  c: "C Oniguruma",
  onigmo: "Ruby Onigmo",
  pcre2_jit: "PCRE2 JIT",
  pcre2: "PCRE2 interpreter",
  fancy_regex: "fancy-regex, seek off",
  fancy_regex_seek: "fancy-regex, seek on",
  regex: "Rust regex",
  regexr: "regexr, JIT off",
  regexr_jit: "regexr, JIT enabled",
};
export const laneNames: Record<string, string> = {
  steady: "Steady reuse · complete fixture batch",
  compile: "Compile · one pattern",
  first_use: "Compile + first use · first fixture",
};
export const engineLanguages: Record<string, string> = {
  rust_erroraware: "Rust",
  c: "C",
  onigmo: "C",
  pcre2_jit: "C",
  pcre2: "C",
  fancy_regex: "Rust",
  fancy_regex_seek: "Rust",
  regex: "Rust",
  regexr: "Rust",
  regexr_jit: "Rust",
};
export const operationNames: Record<string, string | undefined> = {
  is_match: "Boolean match",
  find: "First match span",
  captures_at: "Captures at offset",
  captures: "First match and captures",
  all_captures: "All matches and captures",
  all_matches: "All match spans",
  replace: "Replacement string",
  graphemes: "Grapheme chunks",
};
export type Platform = {
  comparisons: Comparison[];
  production_comparisons: Comparison[];
} & Omit<(typeof data.platforms)[number], "comparisons" | "production_comparisons">;
export type CaseEntry = (typeof data.cases)[number];
export type Estimate = Platform["estimates"][number];
export type Comparison = {
  family_weighted_ratio: null | number;
} & Omit<(typeof data.platforms)[number]["comparisons"][number], "family_weighted_ratio">;
export type Pair = Comparison["cases"][number];

export function required<T>(value: T | undefined): T {
  if (value === undefined) throw new Error("Verified benchmark data is incomplete.");
  return value;
}

export function time(ns: number) {
  const unit = ns >= 10 ** 6 ? "ms" : ns >= 10 ** 3 ? "µs" : "ns";
  const value = ns / (unit === "ms" ? 10 ** 6 : unit === "µs" ? 10 ** 3 : 1);
  return `${value.toLocaleString("en-US", { maximumFractionDigits: 2 })} ${unit}`;
}

export function ratio(value: null | number) {
  if (value === null) return "—";
  return `${value < 0.01 ? value.toPrecision(3) : value.toFixed(3)}×`;
}

export function interpretation(conclusion: string) {
  if (conclusion === "Ferroni faster in every run interval")
    return "Favors Ferroni standard API in every run";
  if (conclusion === "comparator faster in every run interval")
    return "Favors comparator in every run";
  return "Inconclusive";
}
