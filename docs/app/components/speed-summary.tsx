import { comparison, factor, runDate, summary } from "./benchmark-summary";
import { SpeedChart } from "./speed-chart";

export function SpeedSummary() {
  const compile = [
    comparison("macos-arm64", "c", "compile"),
    comparison("linux-x86-64", "c", "compile"),
  ]
    .map((pair) => 1 / pair.family_weighted_ratio)
    .sort((a, b) => a - b);
  return (
    <>
      <SpeedChart />
      <p>
        <strong>Built for reuse.</strong> C Oniguruma prepares patterns {compile[0].toFixed(1)}–
        {factor(compile[1])} faster on the measured aggregate. Check compilation and first use if
        your application rarely reuses a pattern.
      </p>
      <p>
        Measured {runDate()}, on Blacksmith VMs: ARM64 macOS and x86-64 Linux. {summary.case_count}{" "}
        tasks, {summary.input_count} inputs, {summary.platforms[0].repetitions} repetitions per
        platform on separate VMs. Revision{" "}
        <a href={`https://github.com/sebastian-software/ferroni/commit/${summary.run.head_sha}`}>
          {summary.run.head_sha.slice(0, 7)}
        </a>
        .
      </p>
      <p>
        <a href="https://github.com/sebastian-software/ferroni/tree/main/benches/production_results">
          All tables and raw data in the repository
        </a>
      </p>
    </>
  );
}
