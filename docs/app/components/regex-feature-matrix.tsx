import { FerraTable } from "ferramenta-family/ferra";

import capabilities from "../data/regex-capabilities.json";

/** Each cell is backed by an executable check in tests/regex_feature_matrix.rs. */
function FeatureRow({ feature }: { feature: (typeof capabilities.features)[number] }) {
  return (
    <tr>
      <th scope="row">{feature.label}</th>
      {capabilities.engines.map((engine) => (
        <td key={engine.id} data-own={engine.id === "rust" || undefined}>
          {Object.entries(feature.support).find(([id]) => id === engine.id)?.[1] ? "Yes" : "No"}
        </td>
      ))}
    </tr>
  );
}

export function RegexFeatureMatrix() {
  return (
    <FerraTable caption="Selected regex features · checked with untimed compatibility examples">
      <thead>
        <tr>
          <th scope="col">Feature</th>
          {capabilities.engines.map((engine) => (
            <th key={engine.id} scope="col" data-own={engine.id === "rust" || undefined}>
              {engine.label}
              <br />
              <small>{engine.language}</small>
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {capabilities.features.map((feature) => (
          <FeatureRow key={feature.label} feature={feature} />
        ))}
      </tbody>
    </FerraTable>
  );
}
