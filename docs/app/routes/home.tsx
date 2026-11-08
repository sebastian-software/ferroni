// cspell:words mbstring Kosako Onigmo
import type { ArdoMetaFunction } from "ardo/runtime";

import {
  MonoAction,
  MonoClosing,
  MonoColophon,
  MonoEngraving,
  MonoOpening,
  MonoPassage,
  MonoSection,
  MonoWide,
} from "ferramenta-family/monografie";
import { WORKSHOP } from "ferramenta-family/registry";

import { summary } from "../components/benchmark-summary";
import { Guardrails, Port, Prepared, Scope } from "../components/home-engine";
import { Explainer, Scanner, Syntax } from "../components/home-reading";
import { featureCount, spelled } from "../components/home-specimen-data";
import { SpeedComparison } from "../components/speed-comparison";
import { rustVersion, version } from "../data/site.json";

// oxlint-disable-next-line react/only-export-components -- Ardo reads route metadata.
export const meta: ArdoMetaFunction = () => [
  { title: "Ferroni — Oniguruma regex in pure Rust" },
  {
    name: "description",
    content:
      "Keep your Oniguruma patterns in native Rust: lookbehind, backreferences, Unicode and a TextMate scanner. No C toolchain required.",
  },
];
// oxlint-disable-next-line react/only-export-components -- Ardo reads the route layout.
export const handle = { layout: "bare" };

const release = version.split(".").slice(0, 2).join(".");
const runDay = summary.run.created_at.slice(0, 10);
function Actions() {
  return (
    <>
      <MonoAction href="/guide/getting-started" primary detail="cargo add ferroni">
        Run your first match
      </MonoAction>
      <MonoAction href="/guide/scanner" detail="Scanner guide">
        Build a scanner
      </MonoAction>
    </>
  );
}

function Opening() {
  return (
    <MonoOpening
      product="ferroni"
      instrumentLabel="Ferroni's instrument: a pattern comparator on a machined steel plate, three book-like pattern cassettes beside a linear guide, and a carriage that holds a rust-brown contact tool over a recessed text ribbon."
      kicker={`Ferroni ${release}, a regular-expression engine for Rust`}
      title="Your Oniguruma patterns."
      subtitle="Now in pure Rust, with the scanner editors rely on."
      standfirst="Oniguruma reads the patterns behind TextMate grammars, PHP’s mbstring and jq. Its C project closed in April 2025. Ferroni continues it in Rust, ported function by function: lookbehind, backreferences and Unicode matching included, and no C compiler in your build."
      actions={<Actions />}
      facts={[
        {
          value: (
            <>
              <em>2,974</em> of 2,974
            </>
          ),
          label: "upstream UTF-8 tests of Oniguruma",
        },
        {
          value: (
            <>
              <em>12</em> of 12
            </>
          ),
          label: "Oniguruma syntax modes",
        },
        {
          value: (
            <>
              <em>{featureCount.ferroni}</em> of {featureCount.total}
            </>
          ),
          label: "selected features, the only Rust engine",
        },
        { value: `Rust ${rustVersion}+`, label: "BSD-2-Clause, native and WebAssembly" },
      ]}
    />
  );
}
/** How the speed figures were measured, from the published run. */
function Conditions() {
  return (
    <>
      <h3>How this was measured</h3>
      <p>
        Run of {runDay} on Blacksmith VMs: macOS arm64 with 6 vCPU, Linux x86-64 with 4 vCPU.{" "}
        {summary.case_count} tasks from {summary.family_count} project families.
      </p>
      <p>Every task must reproduce the output of its source project before it is timed.</p>
      <p>
        Project families weigh equally. Each figure is a geometric mean of median task ratios over{" "}
        {spelled(summary.platforms[0].repetitions)} runs.
      </p>
      <p>Isolated regex work, not whole applications. Results vary by task.</p>
    </>
  );
}

function Speed() {
  return (
    <MonoSection
      id="fr-speed"
      title="Compile once. Put it to work."
      standfirst="Reusing a compiled pattern, Ferroni matches faster than C Oniguruma, fancy-regex and Rust’s regex crate. PCRE2 with its JIT compiler is faster still, and C compiles patterns faster."
    >
      <MonoPassage
        note={{
          figure: summary.case_count,
          children: "regex tasks taken from Django, Rails, WordPress and eight other projects",
        }}
      >
        <p>
          A highlighter compiles its grammar’s patterns once, then runs them over every line of
          every file it opens. A log filter compiles a pattern once and applies it to millions of
          lines. That reuse is the work Ferroni is built for. It interprets compiled patterns and
          generates no machine code while your program runs.
        </p>
      </MonoPassage>
      <p>
        Creating a pattern for one single search is a different job. There, compilation counts, and
        C Oniguruma is quicker. Switch the chart to see both sides.
      </p>
      <MonoWide>
        <SpeedComparison
          after={
            <p>
              Each row compares only the tasks both engines can run.{" "}
              <a href="/perf/regex-workloads">Every task, platform and timing</a>
            </p>
          }
          conditions={<Conditions />}
        />
      </MonoWide>
    </MonoSection>
  );
}

/** The work Ferroni continues, and who makes it. */
function Origin() {
  return (
    <section id="project-status" aria-label="Origin">
      <div className="mono-axis">
        <MonoColophon>
          Ferroni continues K. Kosako’s Oniguruma, ported from its C source, and follows the Scanner
          API and tests of Microsoft’s vscode-oniguruma. It is an independent continuation, not
          affiliated with or endorsed by Oniguruma’s original author, and keeps its BSD-2-Clause
          license. It is one of the Ferramenta engines, made by Sebastian Software in Mainz,
          Germany.
        </MonoColophon>
      </div>
    </section>
  );
}

/** The actions again, under the published anchors #fr-start and #work. */
function Closing() {
  return (
    <div id="fr-start">
      <MonoClosing
        title={<span id="work">Put your patterns to work.</span>}
        standfirst="Start with a working match, or go straight to the scanner for tokenizers and highlighters."
        actions={<Actions />}
        links={
          <>
            <a href="https://crates.io/crates/ferroni">crates.io</a>
            <a href="https://docs.rs/ferroni">API reference</a>
            <a href="https://github.com/sebastian-software/ferroni">GitHub repository</a>
            <a href={WORKSHOP.consulting}>Integration and support</a>
          </>
        }
      />
    </div>
  );
}

export default function HomePage() {
  return (
    <>
      <Opening />
      <MonoEngraving seed={1} />
      <Explainer />
      <MonoEngraving seed={2} />
      <Syntax />
      <MonoEngraving seed={3} />
      <Speed />
      <MonoEngraving seed={4} />
      <Scanner />
      <MonoEngraving seed={5} />
      <Prepared />
      <MonoEngraving seed={6} />
      <Port />
      <MonoEngraving seed={7} />
      <Guardrails />
      <MonoEngraving seed={8} />
      <Scope />
      <MonoEngraving seed={9} />
      <Origin />
      <Closing />
    </>
  );
}
