import { useState } from "ardo/react";
import { MonoBenchmarkTable, MonoChoices, MonoSelect } from "ferramenta-family/monografie";

import { comparison, type Lane, type Profile, summary } from "./benchmark-summary";

const engines = [
  { id: "c", label: "C Oniguruma" },
  { id: "onigmo", label: "Ruby Onigmo" },
  { id: "pcre2", label: "PCRE2 interpreter" },
  { id: "pcre2_jit", label: "PCRE2 JIT" },
  { id: "fancy_regex", label: "fancy-regex" },
  { id: "regex", label: "Rust regex" },
];
const platforms: Array<{ value: Profile; label: string }> = [
  { value: "macos-arm64", label: "macOS · ARM64" },
  { value: "linux-x86-64", label: "Linux · x86-64" },
];
const phases: Array<{ value: Lane; label: string }> = [
  { value: "steady", label: "Reused pattern" },
  { value: "compile", label: "Compilation" },
  { value: "first_use", label: "First use" },
];

const panels = platforms.flatMap((platform) => phases.map((phase) => ({ platform, phase })));

function ComparisonPanel({ platform, phase }: (typeof panels)[number]) {
  return (
    <MonoBenchmarkTable caption={`${platform.label} · ${phase.label}`}>
      <thead>
        <tr>
          <th scope="col">Compared engine</th>
          <th scope="col">Ferroni throughput</th>
          <th scope="col">Shared tasks</th>
        </tr>
      </thead>
      <tbody>
        {engines.map((engine) => {
          const pair = comparison(platform.value, engine.id, phase.value);
          return (
            <tr key={engine.id}>
              <th scope="row">{engine.label}</th>
              <td>
                <strong>{pair.family_weighted_ratio.toFixed(2)}×</strong>
              </td>
              <td>
                {pair.case_count} / {summary.case_count}
              </td>
            </tr>
          );
        })}
      </tbody>
    </MonoBenchmarkTable>
  );
}

/** Keep every measured platform and phase in the static page; selection changes visibility. */
export function SpeedChart() {
  const [profile, setProfile] = useState<Profile>("macos-arm64");
  const [lane, setLane] = useState<Lane>("steady");
  return (
    <>
      <MonoChoices>
        <MonoSelect
          label="Platform"
          value={profile}
          options={platforms}
          onChange={(event) => {
            const selected = platforms.find((platform) => platform.value === event.target.value);
            if (selected) setProfile(selected.value);
          }}
        />
        <MonoSelect
          label="Phase"
          value={lane}
          options={phases}
          onChange={(event) => {
            const selected = phases.find((phase) => phase.value === event.target.value);
            if (selected) setLane(selected.value);
          }}
        />
      </MonoChoices>
      <p>
        Ferroni’s throughput relative to each engine. Above 1× favors Ferroni; below 1× favors the
        other engine. Each pair covers its own shared tasks, so these rows are not a universal
        ranking.
      </p>
      {panels.map(({ platform, phase }) => (
        <div
          key={`${platform.value}-${phase.value}`}
          id={`comparison-${platform.value}-${phase.value}`}
          hidden={profile !== platform.value || lane !== phase.value}
        >
          <ComparisonPanel platform={platform} phase={phase} />
        </div>
      ))}
    </>
  );
}
