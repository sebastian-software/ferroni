import { useId, useState } from "react";

import { comparison, type Profile, summary } from "./benchmark-summary";
import "./speed-chart.css";

const engines = [
  { id: "c", label: "C Oniguruma", detail: "C" },
  { id: "onigmo", label: "Ruby Onigmo", detail: "C" },
  { id: "pcre2", label: "PCRE2 interpreter", detail: "C" },
  { id: "pcre2_jit", label: "PCRE2 JIT", detail: "C, JIT" },
  { id: "fancy_regex", label: "fancy-regex", detail: "Rust" },
  { id: "regex", label: "Rust regex", detail: "Rust" },
];

const platforms: Array<{ id: Profile; label: string }> = [
  { id: "macos-arm64", label: "ARM64 / macOS" },
  { id: "linux-x86-64", label: "x86-64 / Linux" },
];

/** Powers of two around every factor, so 1× sits on a tick and halving mirrors doubling. */
function scale(values: number[]) {
  const low = Math.min(-1, Math.floor(Math.log2(Math.min(...values))));
  const high = Math.max(2, Math.ceil(Math.log2(Math.max(...values))));
  return {
    at: (value: number) => ((Math.log2(value) - low) / (high - low)) * 100,
    ticks: Array.from({ length: high - low + 1 }, (_, index) => 2 ** (low + index)),
  };
}

/* One scale for both platforms, so switching moves the bars and not the axis. */
const { at, ticks } = scale(
  platforms.flatMap((platform) =>
    engines.map((engine) => comparison(platform.id, engine.id).family_weighted_ratio),
  ),
);
const parity = at(1);

function PlatformToggle({
  profile,
  onChange,
}: {
  profile: Profile;
  onChange: (profile: Profile) => void;
}) {
  return (
    <div className="fr-speed-toggle" role="group" aria-label="Platform">
      {platforms.map((platform) => (
        <button
          key={platform.id}
          type="button"
          aria-pressed={platform.id === profile}
          onClick={() => {
            onChange(platform.id);
          }}
        >
          {platform.label}
        </button>
      ))}
    </div>
  );
}

function SpeedRow({ engine, profile }: { engine: (typeof engines)[number]; profile: Profile }) {
  const pair = comparison(profile, engine.id);
  const value = pair.family_weighted_ratio;
  const faster = value >= 1;
  const end = at(value);
  return (
    <li className="fam-bar" data-direction={faster ? "faster" : "slower"}>
      <span className="fam-bar-label">
        <b>{engine.label}</b>
        <small>
          {engine.detail} · {pair.case_count} of {summary.case_count} tasks
        </small>
      </span>
      <span className="fr-speed-track" aria-hidden="true">
        <span className="fr-speed-parity" style={{ left: `${parity}%` }} />
        <span
          className="fr-speed-fill"
          style={{ left: `${Math.min(parity, end)}%`, width: `${Math.abs(end - parity)}%` }}
        />
      </span>
      <span className="fam-bar-value">
        {value.toFixed(2)}×
        <span className="fam-sr-only">
          {faster ? ", Ferroni is faster" : `, ${engine.label} is faster`}
        </span>
      </span>
    </li>
  );
}

/* The ticks and the two directions, in the track's column so they line up with the bars. */
function SpeedAxis() {
  return (
    <>
      <div className="fr-speed-axis" aria-hidden="true">
        <span />
        <span className="fr-speed-ticks">
          {ticks.map((tick) => (
            <span key={tick} style={{ left: `${at(tick)}%` }}>
              {tick}×
            </span>
          ))}
        </span>
      </div>
      <div className="fr-speed-legend" aria-hidden="true">
        <span />
        <span className="fr-speed-sides">
          <span>← Other engine faster</span>
          <span>Ferroni faster →</span>
        </span>
      </div>
    </>
  );
}

/**
 * Ferroni's reuse throughput against each engine, as bars that grow from 1×:
 * right where Ferroni is faster, left where the other engine is. Each row is
 * its own pair on the tasks both engines can run; the task count says how many.
 */
export function SpeedChart() {
  const [profile, setProfile] = useState<Profile>("macos-arm64");
  const captionId = useId();
  const sorted = engines
    .map((engine) => ({ engine, value: comparison(profile, engine.id).family_weighted_ratio }))
    .sort((a, b) => b.value - a.value);
  return (
    <figure className="fam-bars fr-speed" aria-labelledby={captionId}>
      <div className="fr-speed-head">
        <figcaption className="fam-compare-caption" id={captionId}>
          Ferroni&rsquo;s matching speed relative to each engine, with a compiled pattern reused.
          Each row covers the tasks both engines can run.
        </figcaption>
        <PlatformToggle profile={profile} onChange={setProfile} />
      </div>
      <ul className="fam-bars-list">
        {sorted.map(({ engine }) => (
          <SpeedRow key={engine.id} engine={engine} profile={profile} />
        ))}
      </ul>
      <SpeedAxis />
    </figure>
  );
}
