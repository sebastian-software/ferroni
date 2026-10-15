import { type ComparisonRow, ComparisonTable } from "ferramenta-family";

import capabilities from "../data/regex-capabilities.json";
import "./regex-feature-matrix.css";

const rows: ComparisonRow[] = capabilities.features.map((feature) => ({
  label: feature.label,
  values: feature.support,
}));

/**
 * Selected syntax across the compared engines. Each cell is an executable
 * check in tests/regex_feature_matrix.rs; the page around it says how to read
 * the marks.
 */
export function RegexFeatureMatrix() {
  return (
    <div className="regex-feature-matrix">
      <ComparisonTable
        caption="Selected regex features · checked with untimed compatibility examples"
        subject="Feature"
        contenders={capabilities.engines.map((engine) => ({
          id: engine.id,
          label: (
            <>
              {engine.label}
              <small>{engine.language}</small>
            </>
          ),
          own: engine.id === "rust",
        }))}
        rows={rows}
      />
    </div>
  );
}
