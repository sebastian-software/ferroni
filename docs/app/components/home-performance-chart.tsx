import comparison from "../data/engine-comparison.json";

const { workloads } = comparison;
const original = comparison.engines.find((engine) => engine.id === "c");
if (original === undefined) throw new Error("Missing C Oniguruma measurements.");
const cells: Record<string, { factor?: number }> = original.cells;
const factors = workloads.map((workload) => cells[workload.id].factor ?? 1);
const maximum = Math.max(1, ...factors);
const ratio = new Intl.NumberFormat("en-US", { maximumSignificantDigits: 2 });

export function PerformanceChart() {
  return (
    <figure className="fr-speed-chart">
      <figcaption>Relative throughput · C Oniguruma = 1×</figcaption>
      <ul>
        {workloads.map((workload, index) => {
          const factor = factors[index];
          return (
            <li key={workload.id}>
              <span>{workload.label}</span>
              <div className="fr-speed-track" aria-hidden="true">
                <span style={{ width: `${(factor / maximum) * 100}%` }} />
                <i style={{ left: `${(1 / maximum) * 100}%` }} />
              </div>
              <strong>{ratio.format(factor)}×</strong>
            </li>
          );
        })}
      </ul>
      <p className="fam-note">
        The marker is C Oniguruma at 1×. Higher means more throughput on these workloads; ratios are
        geometric means across tasks and both hosts.
      </p>
    </figure>
  );
}
