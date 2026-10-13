import { type ComparisonRow, ComparisonTable } from "ferramenta-family";

import baseline from "../data/engine-comparison-baseline.json";
import current from "../data/engine-comparison.json";

type Cell = { factor?: number; cases: number; of: number };
const factorFormat = new Intl.NumberFormat("en-US", { maximumSignificantDigits: 3 });
const changeFormat = new Intl.NumberFormat("en-US", {
  style: "percent",
  maximumFractionDigits: 1,
  signDisplay: "exceptZero",
});

const rows: ComparisonRow[] = current.engines.map((engine) => {
  const previous = baseline.engines.find((row) => row.id === engine.id);
  const oldCells = previous?.cells as Record<string, Cell> | undefined;
  const newCells = engine.cells as Record<string, Cell>;
  return {
    label: engine.label,
    values: Object.fromEntries(
      current.workloads.map((workload) => {
        const before = oldCells?.[workload.id];
        const after = newCells[workload.id];
        // Do not compare workloads whose coverage counts changed.
        if (
          before?.factor === undefined ||
          after.factor === undefined ||
          before.cases !== after.cases ||
          before.of !== after.of
        ) {
          return [workload.id, undefined];
        }
        return [
          workload.id,
          <span className="fr-factor-change" key={workload.id}>
            {factorFormat.format(before.factor)}× → {factorFormat.format(after.factor)}×
            <small>
              {changeFormat.format(after.factor / before.factor - 1)} relative throughput
            </small>
          </span>,
        ];
      }),
    ),
  };
});

export function EngineComparisonHistory() {
  return (
    <div className="fr-benchmark-history">
      <ComparisonTable
        align="end"
        caption={`Previous published measurements (${baseline.measured}) → current measurements (${current.measured}). Each factor compares Ferroni with the engine in that row; 1× is equal and higher is faster.`}
        subject="Compared with"
        contenders={current.workloads.map((workload) => ({
          id: workload.id,
          label: workload.id === "highlighting" ? "Scanner replays" : workload.short,
        }))}
        rows={rows}
      />
    </div>
  );
}
