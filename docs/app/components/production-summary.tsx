// cspell:ignore regexr
import { useId } from "react";

import data from "../data/production-summary.json";
import additionalResults from "../data/regexr-summary.json";
import { BenchmarkFactor } from "./benchmark-factor";
import "./production-summary.css";

function publishedSummary(summary: typeof data): typeof data | undefined {
  return summary.platforms.length > 0 ? summary : undefined;
}
const additional = publishedSummary(additionalResults);

type Platform = (typeof data.platforms)[number];
type Comparison = Platform["comparisons"][number];

const comparators = [
  ["c", "C Oniguruma", "C"],
  ["pcre2_jit", "PCRE2 JIT", "C"],
  ["pcre2", "PCRE2 interpreter", "C"],
  ["regex", "Rust regex", "Rust"],
  ["fancy_regex", "fancy-regex, seek off", "Rust"],
  ["fancy_regex_seek", "fancy-regex, seek on", "Rust"],
  ["onigmo", "Ruby Onigmo", "C"],
  ...(additional === undefined
    ? []
    : [
        ["regexr", "regexr, JIT off", "Rust"],
        ["regexr_jit", "regexr, JIT enabled", "Rust"],
      ]),
];

function comparison(profile: string, engine: string, lane = "steady"): Comparison {
  const source = engine.startsWith("regexr") && additional !== undefined ? additional : data;
  const platform = source.platforms.find((entry) => entry.profile === profile);
  const pair = platform?.comparisons.find(
    (entry) => entry.engine === engine && entry.lane === lane,
  );
  if (pair === undefined) throw new Error("Verified benchmark summary is incomplete.");
  return pair;
}

function CComparison() {
  const arm = comparison("macos-arm64", "c");
  const x86 = comparison("linux-x86-64", "c");
  return (
    <p>
      Against C Oniguruma, Ferroni&rsquo;s standard API is{" "}
      <strong>{arm.family_weighted_ratio.toFixed(2)}× as fast on ARM/macOS</strong> and{" "}
      <strong>{x86.family_weighted_ratio.toFixed(2)}× as fast on x86/Linux</strong> in the aggregate
      for repeated use. Individual cases tell a more varied story: {arm.ferroni_cases} of{" "}
      {arm.case_count} favor Ferroni on ARM. On Linux, {x86.ferroni_cases} favor Ferroni,{" "}
      {x86.comparator_cases} favor C, and {x86.inconclusive_cases} are inconclusive.
    </p>
  );
}

function ReuseTable() {
  const id = useId();
  return (
    <div className="production-summary-scroll" tabIndex={0} role="region" aria-labelledby={id}>
      <table>
        <caption id={id}>Repeated use · Ferroni&rsquo;s relative throughput</caption>
        <thead>
          <tr>
            <th scope="col">Compared with Ferroni</th>
            <th scope="col">ARM64 / macOS</th>
            <th scope="col">x86-64 / Linux</th>
          </tr>
        </thead>
        <tbody>
          {comparators.map(([engine, name, language]) => {
            const armPair = comparison("macos-arm64", engine);
            const x86Pair = comparison("linux-x86-64", engine);
            return (
              <tr key={engine}>
                <th scope="row">
                  {name}
                  <small>{language}</small>
                </th>
                <td>
                  <BenchmarkFactor value={armPair.family_weighted_ratio}>
                    <small>
                      {armPair.case_count}/{data.case_count} cases
                    </small>
                  </BenchmarkFactor>
                </td>
                <td>
                  <BenchmarkFactor value={x86Pair.family_weighted_ratio}>
                    <small>
                      {x86Pair.case_count}/{data.case_count} cases
                    </small>
                  </BenchmarkFactor>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function CostNotes() {
  const armCompile = comparison("macos-arm64", "c", "compile");
  const x86Compile = comparison("linux-x86-64", "c", "compile");
  const extraTime = (profile: string) =>
    Math.round((comparison(profile, "rust_erroraware").family_weighted_ratio - 1) * 100);
  return (
    <>
      <p>
        <strong>Preparation changes the comparison.</strong> C Oniguruma takes about{" "}
        {Math.round((1 - armCompile.family_weighted_ratio) * 100)}% less time to compile on ARM and{" "}
        {Math.round((1 - x86Compile.family_weighted_ratio) * 100)}% less on Linux in this corpus.
        PCRE2 JIT leads every source-backed reuse case on both platforms; fancy-regex has a similar
        aggregate reuse time to Ferroni. The tested Rust <code>regex</code> configuration covers
        fewer of these tasks and offers guaranteed linear-time matching for its supported syntax.
      </p>
      <p>
        <strong>The API path also matters.</strong> Ferroni&rsquo;s standard methods can reuse
        cached match workspace. Fallible <code>*_with</code> searches with empty options currently
        create fresh workspace and take about {extraTime("macos-arm64")}% more time on ARM and{" "}
        {extraTime("linux-x86-64")}% more on Linux for reuse. Both paths return the same checked
        output; replacement uses the standard API in both. The detailed tables show this profile
        separately. Use the fallible methods when your application needs execution errors returned
        explicitly.
      </p>
    </>
  );
}

function RunNote() {
  const date = new Date(data.run.created_at).toLocaleDateString("en-US", {
    month: "long",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  });
  return (
    <p className="production-summary-note">
      <a href={data.run.html_url}>Blacksmith run: {date}</a>. {data.case_count} source-backed regex
      tasks, {data.input_count} test inputs, {data.platforms[0].repetitions} process repetitions on
      one VM per platform. Only engines that return the required output are timed. Scanner calls,
      full highlighting and memory usage are outside this measurement.
    </p>
  );
}

export function ProductionSummary() {
  return (
    <div className="production-summary">
      <CComparison />
      <p>
        <strong>Ferroni is the reference in every cell.</strong> 1× means equal throughput. Green
        values above 1 mean Ferroni is faster; red values below 1 mean Ferroni is slower. For
        example, 0.50× means half the throughput and 2× means twice the throughput.
      </p>
      <ReuseTable />
      <p className="production-summary-note">
        Each row compares its own shared tasks; rows with different case pools are not a
        leaderboard. Coverage appears below each factor. Task families have equal weight. These
        figures describe the selected corpus, not a production traffic mix or a guarantee for every
        pattern.
      </p>
      <CostNotes />
      <RunNote />
      {additional !== undefined && (
        <p className="production-summary-note">
          regexr uses a <a href={additional.run.html_url}>separate paired Blacksmith run</a> with a
          freshly measured Ferroni baseline. Unicode mode is enabled. Its JIT-enabled selector can
          fall back to a non-JIT backend; the detailed evidence records the choice.
        </p>
      )}
    </div>
  );
}
