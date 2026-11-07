import { type ReactNode, useState } from "ardo/react";
import {
  MonoBarChart,
  type MonoBarRow,
  MonoSegmented,
  MonoTableView,
} from "ferramenta-family/monografie";

import { comparison, type Lane, type Profile } from "./benchmark-summary";

/** The best-known alternatives; the workload page lists every engine. */
const ENGINES = [
  { id: "c", label: "Oniguruma", language: "C, the original" },
  { id: "pcre2_jit", label: "PCRE2 with JIT", language: "C" },
  { id: "fancy_regex", label: "fancy-regex", language: "Rust" },
  { id: "regex", label: "regex", language: "Rust" },
];
const LANES: Array<{ value: Lane; label: string; phrase: string }> = [
  { value: "steady", label: "Reused pattern", phrase: "reusing a compiled pattern" },
  { value: "compile", label: "Compilation", phrase: "compiling" },
  { value: "first_use", label: "First use", phrase: "a first search" },
];
const PLATFORMS: Array<{ value: Profile; label: string }> = [
  { value: "macos-arm64", label: "macOS arm64" },
  { value: "linux-x86-64", label: "Linux x86-64" },
];

const lanePhrase = (lane: Lane) => LANES.find((entry) => entry.value === lane)?.phrase ?? lane;
const platformLabel = (profile: Profile) =>
  PLATFORMS.find((entry) => entry.value === profile)?.label ?? profile;

function rowsFor(profile: Profile, lane: Lane): MonoBarRow[] {
  return [
    { id: "ferroni", label: "Ferroni", sub: "Rust", value: 1, own: true },
    ...ENGINES.map((engine) => {
      const pair = comparison(profile, engine.id, lane);
      return {
        id: engine.id,
        label: engine.label,
        sub: `${engine.language}, ${String(pair.case_count)} tasks`,
        value: pair.family_weighted_ratio,
      };
    }),
  ];
}

/** The tooltip: the ratio in words, then how the shared tasks split. */
function describeFor(profile: Profile, lane: Lane) {
  return function describe(row: MonoBarRow) {
    if (row.own === true) {
      return (
        <>
          <b>Ferroni</b> is the baseline: 1× for {lanePhrase(lane)}.
        </>
      );
    }
    const pair = comparison(profile, row.id, lane);
    return (
      <>
        <b>{row.label}</b> needed {row.value.toFixed(2)}× as long as Ferroni for {lanePhrase(lane)}{" "}
        on {platformLabel(profile)}
        {row.value < 1 ? ", so it was faster. " : ". "}
        Over {pair.case_count} shared tasks, Ferroni was ahead in {pair.ferroni_cases}, {row.label}{" "}
        in {pair.comparator_cases}, and {pair.inconclusive_cases} were too close to call.
      </>
    );
  };
}

function FigureRow({ engine, lane }: { engine: (typeof ENGINES)[number]; lane: Lane }) {
  return (
    <tr>
      <td>{engine.label}</td>
      <td>{lanePhrase(lane)}</td>
      {PLATFORMS.map((platform) => (
        <td key={platform.value} className="mono-num">
          {comparison(platform.value, engine.id, lane).family_weighted_ratio.toFixed(2)}×
        </td>
      ))}
      <td className="mono-num">{comparison("macos-arm64", engine.id, lane).case_count}</td>
    </tr>
  );
}

/** Every figure of the chart, for both machines, as a table. */
function Figures() {
  const rows = ENGINES.flatMap((engine) => LANES.map((lane) => ({ engine, lane: lane.value })));
  return (
    <MonoTableView>
      <table className="mono-table">
        <thead>
          <tr>
            <th scope="col">Engine</th>
            <th scope="col">Phase</th>
            {PLATFORMS.map((platform) => (
              <th key={platform.value} scope="col" className="mono-num">
                {platform.label}
              </th>
            ))}
            <th scope="col" className="mono-num">
              Tasks
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ engine, lane }) => (
            <FigureRow key={`${engine.id}-${lane}`} engine={engine} lane={lane} />
          ))}
        </tbody>
      </table>
    </MonoTableView>
  );
}

export type SpeedComparisonProps = {
  /** Text and links under the scale. */
  after: ReactNode;
  /** The measuring conditions beside the chart. */
  conditions: ReactNode;
};

/**
 * Time for the same work, relative to Ferroni, from the verified benchmark
 * summary: one bar per engine for the chosen phase and machine.
 */
export function SpeedComparison({ after, conditions }: SpeedComparisonProps) {
  const [lane, setLane] = useState<Lane>("steady");
  const [profile, setProfile] = useState<Profile>("macos-arm64");
  const toggles = (
    <>
      <MonoSegmented
        label="Phase"
        value={lane}
        options={LANES}
        onChange={(value) => {
          setLane(LANES.find((entry) => entry.value === value)?.value ?? "steady");
        }}
      />
      <MonoSegmented
        label="Machine"
        value={profile}
        options={PLATFORMS}
        onChange={(value) => {
          setProfile(PLATFORMS.find((entry) => entry.value === value)?.value ?? "macos-arm64");
        }}
      />
    </>
  );
  return (
    <MonoBarChart
      label={`Time for the same work, ${lanePhrase(lane)} on ${platformLabel(profile)}`}
      title="Time for the same work"
      description="Ferroni = 1×, the line through every row. A bar past it took longer; a bar short of it was faster."
      toggles={toggles}
      rows={rowsFor(profile, lane)}
      scale={4}
      ticks={[1, 2, 3, 4]}
      describe={describeFor(profile, lane)}
      after={
        <>
          {after}
          <Figures />
        </>
      }
      conditions={conditions}
    />
  );
}
