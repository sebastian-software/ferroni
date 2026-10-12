// cspell:words subexpression Onigmo syntect
import { Ledger, Principles, Section } from "ferramenta-family";
import { Link } from "react-router";
import { patternHtml } from "virtual:ferroni-sample";

import walkthrough from "../data/regex-walkthrough.json";
import { PatternExample } from "./pattern-example";

export function FitSection() {
  return (
    <Section
      id="fr-fit"
      title="Does Ferroni fit your project?"
      intro="Start with the behavior your application needs."
    >
      <Principles
        items={[
          {
            heading: "Run TextMate grammars",
            text: (
              <>
                Build editors and highlighters with the multi-pattern Scanner and UTF-16 offsets.{" "}
                <a href="#fr-scanner">See how scanning works</a>.
              </>
            ),
          },
          {
            heading: "Use richer regex syntax",
            text: (
              <>
                Use look-around, backreferences, conditionals and subexpression calls.{" "}
                <a href="#fr-syntax">Explore matching examples</a>.
              </>
            ),
          },
          {
            heading: "Replace C Oniguruma",
            text: (
              <>
                Keep UTF-8 pattern behavior while removing the C toolchain from your build.{" "}
                <Link to="/guide/compatibility">Check compatibility and scope</Link>.
              </>
            ),
          },
        ]}
      />
      <p className="fam-note">
        For patterns that fit Rust&rsquo;s <code>regex</code> crate, prefer its guaranteed
        linear-time matching. Ferroni uses backtracking;{" "}
        <Link to="/guide/untrusted-input">set limits for untrusted input</Link>.
      </p>
    </Section>
  );
}

export function SyntaxSection() {
  return (
    <Section
      id="fr-syntax"
      title="See what the pattern selects"
      intro="Each highlighted match below comes from running Ferroni. Look-behind and backreferences extend the syntax available in Rust’s regex crate; Unicode properties are useful in both engines."
    >
      <div className="fr-examples">
        {walkthrough.syntax.map((example, index) => (
          <PatternExample key={example.title} example={example} html={patternHtml[index]} />
        ))}
      </div>
      <p className="fam-note">
        <Link to="/guide/getting-started">Use these patterns with the Regex API</Link> ·{" "}
        <a href="https://github.com/sebastian-software/ferroni/blob/main/examples/website_walkthrough.rs">
          Run the walkthrough example
        </a>
      </p>
    </Section>
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
      <ul className="fr-patterns">
        {scanner.patterns.map((pattern, index) => (
          <li key={pattern}>
            <strong>
              {index}: {names[index]}
            </strong>{" "}
            <code>{pattern}</code>
          </li>
        ))}
      </ul>
      <ol className="fr-steps">
        {scanner.steps.map((step) => (
          <li key={step.from}>
            Search from <code>{step.from}</code>: pattern <code>{step.index}</code> selects{" "}
            <code>{step.matched}</code> at UTF-16 offsets{" "}
            <code>
              {step.start}–{step.end}
            </code>
            .
          </li>
        ))}
      </ol>
      <p className="fam-note">
        All patterns compete on each search. The earliest match wins; a tie goes to the earlier
        pattern in the list. Continue at the previous match&rsquo;s end. This ASCII line has the
        same byte and UTF-16 offsets.
      </p>
    </figure>
  );
}

const maintenance = [
  {
    name: "Bug fixes and compatibility",
    status: "Maintained",
    settled: true,
    detail: "Fix bugs and parity differences against C Oniguruma.",
  },
  {
    name: "Unicode tables",
    status: "Annual update",
    settled: true,
    detail: "Follow the annual Unicode Character Database release.",
  },
  {
    name: "Toolchain and dependencies",
    status: "Maintained",
    settled: true,
    detail:
      "Keep the Rust toolchain and dependencies working. Raise the MSRV only when a fix or dependency requires it, with release notes.",
  },
  {
    name: "Security",
    status: "Latest release",
    settled: true,
    detail:
      "Security fixes target the latest release. Report suspected vulnerabilities privately through the security policy.",
  },
];

export function StatusSection() {
  return (
    <Section
      id="project-status"
      title="Feature-complete and maintained"
      intro="Ferroni’s scope is Oniguruma syntax for ASCII and UTF-8, plus the vscode-oniguruma scanner. No feature work is planned."
    >
      <Ledger entries={maintenance} />
      <p className="fam-note">
        The other 27 encodings (
        <Link to="/adr/003-encoding-scope-ascii-and-utf8-only">ADR-003</Link>), the POSIX and GNU
        APIs (<Link to="/adr/012-posix-and-gnu-api-not-ported">ADR-012</Link>) and{" "}
        <a href="https://github.com/sebastian-software/ferroni/issues/159">opt-in Onigmo syntax</a>{" "}
        are outside the planned scope.
      </p>
      <p className="fam-note">
        Issues are welcome and triaged as time allows. Pull requests must keep{" "}
        <Link to="/adr/001-one-to-one-parity-with-c-original">
          structural parity with C Oniguruma
        </Link>
        . The{" "}
        <a href="https://github.com/sebastian-software/ferroni/labels/help%20wanted">
          help wanted issues
        </a>{" "}
        identify contribution opportunities, including the{" "}
        <a href="https://github.com/sebastian-software/ferroni/issues/252">
          syntect backend integration
        </a>
        .
      </p>
      <p className="fam-note">
        <a href="https://github.com/sebastian-software/ferroni/blob/main/CONTRIBUTING.md">
          Contributing
        </a>{" "}
        ·{" "}
        <a href="https://github.com/sebastian-software/ferroni/blob/main/SECURITY.md">
          Security policy and private reporting
        </a>
      </p>
    </Section>
  );
}
