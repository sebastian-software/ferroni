import { FEATURE_ENGINES, specimens } from "./home-specimen-data";
import "./feature-specimens.css";

type Feature = (typeof specimens.features)[number];

function FeatureRow({ feature }: { feature: Feature }) {
  return (
    <tr>
      <th scope="row">{feature.label.replace(" · ", ", ")}</th>
      <td>
        <code className="fr-feature-pattern">{feature.pattern}</code>
        <span className="fr-feature-input">
          {feature.match.before}
          <mark>{feature.match.matched}</mark>
          {feature.match.after}
        </span>
      </td>
      {FEATURE_ENGINES.map((engine) => {
        const yes = feature.support[engine.id];
        return (
          <td
            key={engine.id}
            className="fr-support"
            data-engine={engine.label}
            data-own={engine.id === "rust" ? "" : undefined}
            data-yes={yes ? "" : undefined}
          >
            <span className="fr-dot" aria-hidden="true" />
            {yes ? "Yes" : "No"}
          </td>
        );
      })}
    </tr>
  );
}

/**
 * One row per selected feature: Ferroni's real match for the feature's pattern
 * and text, then which engines support it. Below 761px each row becomes one
 * entry with the engines in a line.
 */
export function FeatureSpecimens() {
  return (
    <table className="fr-features">
      <caption className="mono-visually-hidden">
        Selected regex features, Ferroni’s match for each, and which engines support it
      </caption>
      <thead>
        <tr>
          <th scope="col">Feature</th>
          <th scope="col">Pattern, text and Ferroni’s match</th>
          {FEATURE_ENGINES.map((engine) => (
            <th key={engine.id} scope="col" className={engine.id === "rust" ? "fr-own" : undefined}>
              {engine.label}
              <small>{engine.language}</small>
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {specimens.features.map((feature) => (
          <FeatureRow key={feature.id} feature={feature} />
        ))}
      </tbody>
    </table>
  );
}
