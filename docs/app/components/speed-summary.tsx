import { Measured } from "ferramenta-family";

import { comparison, factor, runDate, summary } from "./benchmark-summary";
import { SpeedChart } from "./speed-chart";

/* The cost side of the speed claim: C prepares a pattern faster on both platforms. */
function CompileTradeoff() {
  const compile = [
    comparison("macos-arm64", "c", "compile"),
    comparison("linux-x86-64", "c", "compile"),
  ]
    .map((pair) => 1 / pair.family_weighted_ratio)
    .sort((a, b) => a - b);
  return (
    <p className="fr-speed-tradeoff">
      <strong>Where C still wins: compiling.</strong> C Oniguruma prepares a pattern{" "}
      {compile[0].toFixed(1)}–{factor(compile[1])} faster, so a pattern used once favors C. Ferroni
      pays off when a pattern runs many times, as in highlighters, parsers and validators.
    </p>
  );
}

/**
 * The speed chart with its one caveat and its provenance. Every table behind
 * it lives in the repository, not on the site.
 */
export function SpeedSummary() {
  return (
    <>
      <SpeedChart />
      <CompileTradeoff />
      <Measured
        on={runDate()}
        machine="Blacksmith VMs, ARM64 macOS and x86-64 Linux"
        revision={
          <a href={`https://github.com/sebastian-software/ferroni/commit/${summary.run.head_sha}`}>
            {summary.run.head_sha.slice(0, 7)}
          </a>
        }
        more={[
          { label: "Corpus", value: `${summary.case_count} tasks, ${summary.input_count} inputs` },
          {
            label: "Repetitions",
            value: `${summary.platforms[0].repetitions} per platform, one VM each`,
          },
        ]}
      >
        <a href="https://github.com/sebastian-software/ferroni/tree/main/benches/production_results">
          All tables and raw data in the repository
        </a>
      </Measured>
    </>
  );
}
