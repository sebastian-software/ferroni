// cspell:ignore rustc
import { useId, useState } from "react";

import {
  type Comparison,
  data,
  engineNames,
  laneNames,
  type Platform,
  ratio,
  required,
} from "./production-comparison-model";
import { CaseTable } from "./production-comparison-table";
import "./production-comparison.css";

function Choice({
  id,
  label,
  value,
  options,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  options: Record<string, string>;
  onChange: (value: string) => void;
}) {
  return (
    <label htmlFor={id}>
      {label}
      <select
        id={id}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        {Object.entries(options).map(([key, name]) => (
          <option key={key} value={key}>
            {name}
          </option>
        ))}
      </select>
    </label>
  );
}

function ComparisonIntro({
  production,
  repetitions,
}: {
  production: Comparison;
  repetitions: number;
}) {
  const conclusions = production.cases.map((entry) => entry.conclusion);
  const count = (value: string) => conclusions.filter((entry) => entry === value).length;
  return (
    <>
      <p aria-live="polite">
        <strong>{production.case_count} common production cases</strong> across{" "}
        {production.family_count} families. Pair-specific family-weighted ratio:{" "}
        <strong>{ratio(production.family_weighted_ratio)}</strong>. Controlled probes are excluded
        from this aggregate. Different supported case pools do not form an engine ranking.
      </p>
      <p>
        Cases favoring Ferroni's standard API in every run:{" "}
        {count("Ferroni faster in every run interval")}; favoring the comparator:{" "}
        {count("comparator faster in every run interval")}; inconclusive: {count("inconclusive")}.
      </p>
      <p>
        Ratios are comparator time / Ferroni standard API time. Above 1 favors the standard API;
        below 1 favors the comparator. Times are medians of {repetitions} process mean estimates;
        ratios are medians of the ratios paired by repetition. Run ranges retain every process
        result.
      </p>
      <p>
        Green factors favor Ferroni, red factors favor the comparator. Inconclusive cases stay
        neutral even when their point estimate falls above or below 1.
      </p>
    </>
  );
}

function RunnerContext({ context, engine }: { context: Platform["context"]; engine: string }) {
  const entries = [
    ["Recorded host", `${context.platform}; ${context.cpu_count} visible CPUs`],
    ["Measurement started", context.started_utc],
    ["Toolchain", context.rustc],
    ["Build", `${context.build_profile}; ${context.pcre2_build}`],
    ["Ferroni baseline", (context.profiles as Record<string, string>).rust],
    ["Comparator", (context.profiles as Record<string, string>)[engine]],
    [
      "Budget per lane",
      `${context.repetitions} processes, ${context.samples} samples, ${context.warmup_seconds} s warmup, ${context.measure_seconds} s measurement`,
    ],
    ["Host limits", context.host_note],
  ];
  return (
    <details className="production-context">
      <summary>Runner, build and engine profile</summary>
      <dl>
        {entries.map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
    </details>
  );
}

function EvidenceLinks() {
  return (
    <p>
      Measured revision{" "}
      <a href={`https://github.com/sebastian-software/ferroni/commit/${data.run.head_sha}`}>
        <code>{data.run.head_sha.slice(0, 7)}</code>
      </a>
      {" · "}
      <a href={data.run.html_url}>Blacksmith workflow and job logs</a>
      {" · "}
      <a href="https://github.com/sebastian-software/ferroni/tree/main/benches/production_results">
        Retained raw evidence and generated tables
      </a>
    </p>
  );
}

export function ProductionComparison() {
  const id = useId();
  const [profile, setProfile] = useState("macos-arm64");
  const [lane, setLane] = useState("steady");
  const [engine, setEngine] = useState("c");
  const platform = required(data.platforms.find((entry) => entry.profile === profile));
  const matches = (entry: Comparison) => entry.engine === engine && entry.lane === lane;
  const comparison = required(platform.comparisons.find(matches));
  const production = required(platform.production_comparisons.find(matches));
  return (
    <div className="production-comparison">
      <div className="production-controls">
        <Choice
          id={`${id}-platform`}
          label="Platform"
          value={profile}
          options={{ "macos-arm64": "ARM64 · macOS", "linux-x86-64": "x86-64 · Linux" }}
          onChange={setProfile}
        />
        <Choice
          id={`${id}-lane`}
          label="Phase"
          value={lane}
          options={laneNames}
          onChange={setLane}
        />
        <Choice
          id={`${id}-engine`}
          label="Compare Ferroni standard API with"
          value={engine}
          options={engineNames}
          onChange={setEngine}
        />
      </div>
      <ComparisonIntro production={production} repetitions={platform.context.repetitions} />
      <CaseTable platform={platform} comparison={comparison} lane={lane} engine={engine} id={id} />
      <RunnerContext context={platform.context} engine={engine} />
      <EvidenceLinks />
    </div>
  );
}
