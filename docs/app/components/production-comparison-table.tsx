import {
  type CaseEntry,
  type Comparison,
  data,
  engineNames,
  type Estimate,
  interpretation,
  laneNames,
  operationNames,
  type Pair,
  type Platform,
  ratio,
  required,
  time,
} from "./production-comparison-model";

function Timing({ estimate }: { estimate: Estimate | undefined }) {
  if (estimate === undefined) return <>—</>;
  return (
    <>
      <strong>{time(estimate.ns_median)}</strong>
      <small>
        Runs: {time(estimate.ns_range[0])}–{time(estimate.ns_range[1])}
      </small>
    </>
  );
}

function ScopeDetails({
  entry,
  pair,
  base,
  other,
  reason,
  engineName,
}: {
  entry: CaseEntry;
  pair: Pair | undefined;
  base: Estimate | undefined;
  other: Estimate | undefined;
  reason: string | undefined;
  engineName: string;
}) {
  return (
    <details>
      <summary>Scope and evidence</summary>
      <p>{entry.scope}</p>
      {pair === undefined ? (
        <p>{reason}</p>
      ) : (
        <>
          <p>
            Ratio interval envelope: {ratio(pair.interval_envelope[0])}–
            {ratio(pair.interval_envelope[1])}.
          </p>
          <p>
            Time interval envelopes (95% per process). Ferroni standard API:{" "}
            {time(required(base).interval_envelope_ns[0])}–
            {time(required(base).interval_envelope_ns[1])}. {engineName}:{" "}
            {time(required(other).interval_envelope_ns[0])}–
            {time(required(other).interval_envelope_ns[1])}.
          </p>
        </>
      )}
    </details>
  );
}

type RowProps = {
  entry: CaseEntry;
  platform: Platform;
  comparison: Comparison;
  lane: string;
  engine: string;
};

function WorkloadHeading({ entry }: { entry: CaseEntry }) {
  return (
    <th scope="row">
      <a href={entry.source_url}>{entry.id.replaceAll("_", " ")}</a>
      <small>
        {entry.tier} · {operationNames[entry.operation] ?? entry.operation}
      </small>
      <small>
        {entry.input_count} inputs
        {entry.pattern_origin === "controlled" ? " · controlled probe" : ""}
      </small>
    </th>
  );
}

function RatioCell({ pair }: { pair: Pair | undefined }) {
  return (
    <td className="production-number">
      {pair === undefined ? (
        "—"
      ) : (
        <>
          <strong>{ratio(pair.ratio_median)}</strong>
          <small>
            Runs: {ratio(pair.ratio_range[0])}–{ratio(pair.ratio_range[1])}
          </small>
        </>
      )}
    </td>
  );
}

function WorkloadRow({ entry, platform, comparison, lane, engine }: RowProps) {
  const eligibility = required(
    platform.eligibility.find((row) => row.case === entry.id && row.engine === engine),
  );
  const pair = comparison.cases.find((row) => row.case === entry.id);
  const estimate = (engineId: string) =>
    platform.estimates.find(
      (row) => row.case === entry.id && row.engine === engineId && row.lane === lane,
    );
  const base = estimate("rust"),
    other = estimate(engine);
  return (
    <tr>
      <WorkloadHeading entry={entry} />
      <td className="production-number">
        <Timing estimate={base} />
      </td>
      <td className="production-number">
        <Timing estimate={other} />
      </td>
      <RatioCell pair={pair} />
      <td>
        <strong>
          {pair === undefined
            ? eligibility.status.replaceAll("_", " ")
            : interpretation(pair.conclusion)}
        </strong>
        <ScopeDetails
          entry={entry}
          pair={pair}
          base={base}
          other={other}
          reason={eligibility.reason}
          engineName={engineNames[engine]}
        />
      </td>
    </tr>
  );
}

type TableProps = {
  platform: Platform;
  comparison: Comparison;
  lane: string;
  engine: string;
  id: string;
};

export function CaseTable({ platform, comparison, lane, engine, id }: TableProps) {
  return (
    <>
      <p className="production-scroll-note" id={`${id}-scroll`}>
        On narrow screens, scroll the table horizontally to compare timings and interpretation.
      </p>
      <div
        className="production-table-scroll"
        role="region"
        aria-label="Per-case benchmark results"
        aria-describedby={`${id}-scroll`}
        tabIndex={0}
      >
        <table>
          <caption>
            {platform.profile} · {laneNames[lane]} · Ferroni standard API and {engineNames[engine]}
          </caption>
          <thead>
            <tr>
              <th scope="col">Workload</th>
              <th scope="col" className="production-number">
                Ferroni standard API
              </th>
              <th scope="col" className="production-number">
                {engineNames[engine]}
              </th>
              <th scope="col" className="production-number">
                Ratio
              </th>
              <th scope="col">Interpretation and scope</th>
            </tr>
          </thead>
          <tbody>
            {data.cases.map((entry) => (
              <WorkloadRow
                key={entry.id}
                entry={entry}
                platform={platform}
                comparison={comparison}
                lane={lane}
                engine={engine}
              />
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
