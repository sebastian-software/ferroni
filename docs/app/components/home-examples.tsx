import { ArdoLink as Link } from "ardo/runtime";
import { FerraArticle, FerraSpecimen, FerraTable } from "ferramenta-family/ferra";

import capabilities from "../data/regex-capabilities.json";
import walkthrough from "../data/regex-walkthrough.json";
import { PatternExample } from "./pattern-example";

function supportsAllFeatures(engine: string) {
  return capabilities.features.every((feature) =>
    Object.entries(feature.support).some(([id, supported]) => id === engine && supported),
  );
}

export function SyntaxSection() {
  const complete = capabilities.engines.filter(
    (engine) => engine.language === "Rust" && supportsAllFeatures(engine.id),
  );
  if (complete.length !== 1 || complete[0].id !== "rust") {
    throw new Error(
      "Revise the syntax section: Ferroni is no longer the one complete Rust engine.",
    );
  }
  return (
    <FerraArticle
      id="fr-syntax"
      title="Give your patterns room to work."
      introduction="Look behind a label. Match a repeated word. Select a Unicode script. Keep the expressions your application already relies on."
      meta={
        <Link to="/perf/regex-workloads#syntax-coverage">Compare the checked syntax support</Link>
      }
    >
      <p>
        Ferroni implements all twelve Oniguruma syntax modes, including named-group recursion. It is
        the only Rust engine in our selected-feature comparison that passes every check. These
        examples show actual matches from Ferroni.
      </p>
      <SyntaxExamples />
    </FerraArticle>
  );
}

export function SyntaxExamples() {
  return (
    <>
      {walkthrough.syntax.map((example) => (
        <PatternExample key={example.title} example={example} />
      ))}
    </>
  );
}

export function ScannerWalkthrough() {
  const { scanner } = walkthrough;
  const names = ["Keywords", "Strings", "Comments"];
  return (
    <>
      <FerraSpecimen caption="Scanner input · selected matches">
        <pre tabIndex={0} aria-label="Scanner input with selected matches">
          <code>
            {scanner.steps.map((step, index) => (
              <span key={step.index}>
                {scanner.input.slice(index === 0 ? 0 : scanner.steps[index - 1].end, step.start)}
                <mark>{step.matched}</mark>
              </span>
            ))}
            {scanner.input.slice(scanner.steps.at(-1)?.end ?? 0)}
          </code>
        </pre>
      </FerraSpecimen>
      <FerraTable caption="One line, three searches for the next match">
        <thead>
          <tr>
            <th scope="col">Pattern</th>
            <th scope="col">Selected match</th>
            <th scope="col">Byte range</th>
          </tr>
        </thead>
        <tbody>
          {scanner.steps.map((step) => (
            <tr key={step.from}>
              <th scope="row">{names[step.index]}</th>
              <td>
                <code>{step.matched}</code>
              </td>
              <td>
                {step.start}–{step.end}
              </td>
            </tr>
          ))}
        </tbody>
      </FerraTable>
      <p>
        All patterns compete on each search. The earliest match wins; the next search starts where
        it ended.
      </p>
    </>
  );
}
