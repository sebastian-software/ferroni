import {
  type ComparisonCell,
  type ComparisonRow,
  ComparisonTable,
  family,
  Mark,
  Measured,
  Principles,
  ProjectHero,
  Relations,
  RunSample,
  Section,
  WorkWithUs,
} from "ferramenta-family";
import { Link, type MetaFunction } from "react-router";
import config from "virtual:ardo/config";

import engineComparison from "../data/engine-comparison.json";
import sample from "../data/regex-sample.json";
import { ClosingSection, CodeSection, CoverageSection } from "./home-bottom-sections";

// React Router requires `meta` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const meta: MetaFunction = () => [
  { title: "Ferroni — Oniguruma-compatible regex engine" },
  {
    name: "description",
    content:
      "Ferroni continues the Oniguruma regex engine in memory-safe Rust after the C project ended, with the vscode-oniguruma scanner built in. Verified against the upstream tests, and measured against C on real code.",
  },
];

function ferroni() {
  const tool = family.find((entry) => entry.name === "ferroni");
  if (tool === undefined) throw new Error("Ferroni is missing from the family registry.");
  return tool;
}

/*
 * What it succeeds and what it is checked against come from the registry; the
 * release is the version this site was built from, read from Cargo.toml.
 */
function HeroSection() {
  const tool = ferroni();
  const version = config.project?.version;
  return (
    <ProjectHero
      icon="ferroni"
      title={<span translate="no">Ferroni</span>}
      what="A regex engine in memory-safe Rust."
      lede={
        <>
          It continues Oniguruma, the engine TextMate grammars are written for, after its C project
          ended, with the vscode-oniguruma scanner built in. Verified against the upstream tests and
          measured against the C original.
        </>
      }
      actions={
        <>
          <Link to="/guide/getting-started" className="fam-btn fam-btn-primary">
            Get started <Mark name="arrow" className="icon" size={18} />
          </Link>
          <a href="https://github.com/sebastian-software/ferroni" className="fam-btn fam-btn-ghost">
            <Mark name="github" className="icon" size={18} /> GitHub
          </a>
        </>
      }
      install={<code translate="no">cargo add ferroni</code>}
      facts={[
        { label: "Succeeds", value: tool.succeeds },
        { label: "Checked against", value: tool.evidence },
        ...(version == null ? [] : [{ label: "Release", value: `v${version}` }]),
      ]}
    />
  );
}

const pillars = [
  {
    heading: "Same engine, verified",
    text: "A line-by-line port that keeps Oniguruma's module structure and optimization pipeline. Verified against the upstream UTF-8 tests, with differential checks against C.",
  },
  {
    heading: "Memory-safe, no C toolchain",
    text: "cargo add ferroni and build: no bindgen, no C compiler, no node-gyp. The limited unsafe code follows two documented patterns in ADR-002.",
  },
  {
    heading: "Measured on real code",
    text: "From individual regex searches to complete TextMate grammars, the benchmark reports record input, timings, and reproduction steps alongside the tradeoffs.",
  },
  {
    heading: "The scanner, built in",
    text: "vscode-textmate and Shiki tokenize through vscode-oniguruma's scanner. Ferroni ships a scanner of the same shape, UTF-16 offsets included, next to the regex engine.",
  },
];

function RelationsSection() {
  return (
    <Section
      id="fr-relations"
      title="Where Ferroni sits"
      intro="Ferroni supplies the regex engine for Ferriki, whose Shiki-compatible highlighter tokenizes code with TextMate grammars. Each tool also works on its own."
    >
      <Relations current="ferroni" />
    </Section>
  );
}

type FigureCell = {
  text: string;
  note?: string;
  grammars?: string[];
  cases: number;
  of: number;
  factors?: Record<string, number>;
};

const workloads = engineComparison.workloads;

function figureValue(cell: FigureCell): ComparisonCell {
  if (cell.factors === undefined) {
    return cell.grammars === undefined ? undefined : { mark: "no", note: cell.grammars.join(", ") };
  }
  if (cell.note === undefined) return cell.text;
  return (
    <>
      {cell.text} <small>{cell.note}</small>
    </>
  );
}

/* Every figure comes from docs/app/data/engine-comparison.json, which
 * `scripts/compare-engines.py figures` derives from the retained run. */
const comparisonRows: ComparisonRow[] = engineComparison.engines.map((engine) => {
  const cells = engine.cells as Record<string, FigureCell>;
  const measured = workloads.flatMap((workload) => Object.values(cells[workload.id].factors ?? {}));
  return {
    label: engine.label,
    values: Object.fromEntries(
      workloads.map((workload) => [workload.id, figureValue(cells[workload.id])]),
    ),
    // Ferroni is slower in every workload this engine was measured in.
    behind: measured.length > 0 && measured.every((factor) => factor < 1),
  };
});

function EvidenceSection() {
  const hosts = engineComparison.hosts
    .map((host) => `${host.label}: ${host.machine}, ${host.cpus} vCPUs`)
    .join("; ");
  return (
    <Section
      id="fr-evidence"
      layout="split"
      title="Measured against seven engines"
      intro={
        <>
          Each factor is the other engine&rsquo;s time divided by Ferroni&rsquo;s, as the geometric
          mean over the workload; a range spans the two hosts. Above 1&times;, Ferroni is faster.
          Every engine first has to reproduce Oniguruma&rsquo;s results, or the results Shiki
          produced for highlighting.
        </>
      }
      note={
        <>
          Text processing runs 49 tasks over HTML, logs, chat with emoji, Markdown, JSON, CSV and
          source code, with syntax the <code>regex</code> crate also runs or Oniguruma syntax.
          Highlighting replays the scanner calls Shiki makes for C, Java and PHP documents, grammars
          every engine can run. PCRE2&rsquo;s JIT and the <code>regex</code> crate win most text
          tasks, but neither finds the earliest match among many patterns in one search, as a
          highlighter needs.
        </>
      }
    >
      <ComparisonTable
        align="end"
        caption="Ferroni's speedup over each engine; below 1×, the other engine is faster."
        subject="Compared with"
        contenders={workloads.map((workload) => ({ id: workload.id, label: workload.short }))}
        rows={comparisonRows}
      />
      <Measured
        on={engineComparison.measured}
        machine={`Blacksmith runners. ${hosts}`}
        revision={<code>{engineComparison.commit.slice(0, 8)}</code>}
      >
        <Link to="/perf/engine-comparison">Every case, the hosts, versions and raw data</Link>
      </Measured>
    </Section>
  );
}

function SampleSection() {
  return (
    <Section
      id="fr-run"
      title="A regular expression, run"
      intro="A lookbehind selects the date; named groups return its parts. This is the committed output of the Rust example shown here."
      note={
        <a href="https://github.com/sebastian-software/ferroni/blob/main/examples/website_sample.rs">
          Run the example: cargo run --example website_sample
        </a>
      }
    >
      <RunSample
        input={sample.input}
        inputCaption="website_sample.rs"
        inputKind="Rust source"
        output={sample.output}
        outputCaption={`Ferroni ${sample.version} · stdout`}
      />
      <details className="fr-sample-output">
        <summary>Read the output as text</summary>
        <pre tabIndex={0}>
          <code>{sample.stdout}</code>
        </pre>
      </details>
    </Section>
  );
}

export default function HomePage() {
  return (
    <div className="fam-page ferroni-home">
      <HeroSection />
      <Section
        id="fr-forward"
        title="Oniguruma ended. The engine goes on."
        intro="Oniguruma’s C project closed on April 24, 2025, after more than twenty years as the regex engine that TextMate grammars are written for. Ferroni carries it forward."
      >
        <Principles items={pillars} />
      </Section>
      <RelationsSection />
      <SampleSection />
      <EvidenceSection />
      <CodeSection />
      <CoverageSection />
      <ClosingSection />
      <WorkWithUs />
    </div>
  );
}
