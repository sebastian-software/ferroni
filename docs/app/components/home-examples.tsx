import { Section } from "ferramenta-family";
import { Link } from "react-router";
import { patternHtml } from "virtual:ferroni-sample";

import capabilities from "../data/regex-capabilities.json";
import walkthrough from "../data/regex-walkthrough.json";
import { PatternExample } from "./pattern-example";

type Feature = (typeof capabilities.features)[number];

function supports(feature: Feature, engine: string) {
  return Object.entries(feature.support).some(([id, supported]) => id === engine && supported);
}

/* The Rust engines of the matrix that hold every feature it lists, Ferroni among them. */
function completeRustEngines() {
  return capabilities.engines.filter(
    (engine) =>
      engine.language === "Rust" &&
      capabilities.features.every((feature) => supports(feature, engine.id)),
  );
}

export function SyntaxSection() {
  const complete = completeRustEngines();
  if (complete.length !== 1 || complete[0].id !== "rust") {
    throw new Error(
      "Revise the syntax section: Ferroni is no longer the one complete Rust engine.",
    );
  }
  return (
    <Section
      id="fr-syntax"
      title="Give your patterns room to work"
      intro={
        <>
          Match a repeated word. Look behind a label. Call a named group recursively. Ferroni brings
          Oniguruma&rsquo;s full syntax to Rust, in all twelve syntax modes. It covers every feature
          in our checked syntax matrix, the only Rust engine in that comparison to do so.
          Rust&rsquo;s <code>regex</code> crate is a great choice when you need its linear-time
          guarantee.
        </>
      }
      note={
        <>
          These matches come from running Ferroni.{" "}
          <Link to="/perf/regex-workloads#syntax-coverage">Compare syntax support</Link>
          {" · "}
          <Link to="/guide/coming-from-regex#syntax-beyond-the-regex-crate">Worked examples</Link>
        </>
      }
    >
      <SyntaxExamples />
    </Section>
  );
}

/** Three matches with syntax the regex crate rejects, each from running Ferroni. */
export function SyntaxExamples() {
  return (
    <div className="fr-examples">
      {walkthrough.syntax.map((example, index) => (
        <PatternExample key={example.title} example={example} html={patternHtml[index]} />
      ))}
    </div>
  );
}

export function ScannerWalkthrough() {
  const { scanner } = walkthrough;
  const names = ["Keywords", "Strings", "Comments"];
  return (
    <figure className="fr-scanner-walkthrough">
      <figcaption>One line, three searches for the next match</figcaption>
      <pre tabIndex={0}>
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
      <ol className="fr-steps">
        {scanner.steps.map((step) => (
          <li key={step.from}>
            <strong>{names[step.index]}</strong> <code>{scanner.patterns[step.index]}</code>
            <span>
              From offset <code>{step.from}</code>, it selects <code>{step.matched}</code> at{" "}
              <code>
                {step.start}–{step.end}
              </code>
              .
            </span>
          </li>
        ))}
      </ol>
      <p className="fam-note">
        All patterns compete on each search; the earliest match wins, and the next search starts
        where it ended.
      </p>
    </figure>
  );
}
