import {
  type ComparisonCell,
  type ComparisonRow,
  ComparisonTable,
  family,
  Mark,
  Measured,
  ProjectHero,
  Relations,
  Section,
  WorkWithUs,
} from "ferramenta-family";
import { Link, type MetaFunction } from "react-router";
import config from "virtual:ardo/config";

import { FitSection, StatusSection, SyntaxSection } from "../components/home-examples";
import { HomeNavigation } from "../components/home-navigation";
import { PerformanceChart } from "../components/home-performance-chart";
import { HomeRunSample } from "../components/home-run-sample";
import engineComparison from "../data/engine-comparison.json";
import {
  ClosingSection,
  CoverageSection,
  ProofSection,
  SafetySection,
  ScannerSection,
} from "./home-bottom-sections";
// React Router requires `meta` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const meta: MetaFunction = () => [
  { title: "Ferroni — Oniguruma's regex engine, modernized in Rust" },
  {
    name: "description",
    content:
      "Full Oniguruma syntax for Rust, with the vscode-oniguruma scanner that TextMate grammars run on. One cargo add, no C compiler, memory-safe, verified against the upstream tests, and measured against the C original.",
  },
];

function ferroni() {
  const tool = family.find((entry) => entry.name === "ferroni");
  if (tool === undefined) throw new Error("Ferroni is missing from the family registry.");
  return tool;
}

type FigureCell = {
  text: string;
  note?: string;
  grammars?: string[];
  cases: number;
  of: number;
  /** Per host; the summary shows only `factor`, their geometric mean. */
  factors?: Record<string, number>;
  factor?: number;
  /** "faster" or "slower": which way `text` reads. */
  direction?: string;
};

/* The cell covers only the cases the engine could run: footnoted, not spelled out. */
const partial = (cell: FigureCell) => cell.factor !== undefined && cell.cases < cell.of;

const workloads = engineComparison.workloads;

/* The C original's row: the figures the speed claims on this page rest on. */
const cRow = engineComparison.engines.find((engine) => engine.id === "c");
if (cRow === undefined) throw new Error("engine-comparison.json has no row for Oniguruma (C).");
const cCells = cRow.cells as Record<string, FigureCell>;

/*
 * "Faster than the original" is only said while it holds: on every host, in
 * every workload of the retained run. A run where C wins a workload turns the
 * headline into a plain "measured against".
 */
const cFactors = workloads.flatMap((workload) => Object.values(cCells[workload.id].factors ?? {}));
const aheadOfC = cFactors.length > 0 && cFactors.every((factor) => factor > 1);

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
      what="Oniguruma's regex engine, modernized in Rust."
      lede={
        <>
          Run TextMate grammars and patterns with look-behind or backreferences in pure Rust. No C
          compiler. Verified against Oniguruma&rsquo;s upstream tests.
          {aheadOfC ? " Faster than C in our measured workloads." : " Benchmarked against C."}
        </>
      }
      actions={
        <>
          <Link to="/guide/getting-started" className="fam-btn fam-btn-primary">
            Read the quickstart <Mark name="arrow" className="icon" size={18} />
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

function SampleSection() {
  return (
    <Section
      id="fr-run"
      title="Familiar Rust, full Oniguruma syntax"
      intro={
        <>
          Select a date with look-behind, then read its named captures with a familiar Rust API. The
          output below comes from running this example.
        </>
      }
      note={
        <a href="https://github.com/sebastian-software/ferroni/blob/main/examples/website_sample.rs">
          Run the example: cargo run --example website_sample
        </a>
      }
    >
      <HomeRunSample />
    </Section>
  );
}

function figureValue(cell: FigureCell): ComparisonCell {
  if (cell.factor === undefined) {
    return cell.grammars === undefined ? undefined : { mark: "no", note: cell.grammars.join(", ") };
  }
  return (
    <>
      {cell.text} <small>{cell.direction}</small>
      {partial(cell) && (
        <>
          <sup aria-hidden="true">*</sup>
          <span className="fam-sr-only">, some tasks left out</span>
        </>
      )}
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

/* The footnotes the table needs, worded once in compare-engines.py. */
const allCells = engineComparison.engines.flatMap((engine) =>
  workloads.map((workload) => (engine.cells as Record<string, FigureCell>)[workload.id]),
);
const tableNotes = [
  ...(allCells.some((cell) => partial(cell)) ? [`* ${engineComparison.notes.partial}`] : []),
  ...(allCells.some((cell) => cell.text === "–") ? [`– ${engineComparison.notes.absent}`] : []),
  // Which of fancy-regex's configurations each column quotes.
  engineComparison.notes.configurations,
];

function SpeedSection() {
  const hosts = engineComparison.hosts
    .map((host) => `${host.label}: ${host.machine}, ${host.cpus} vCPUs`)
    .join("; ");
  return (
    <Section
      id="fr-evidence"
      layout="split"
      title={aheadOfC ? "Faster in the measured workloads" : "Measured against C Oniguruma"}
      intro={
        <>
          Three workloads, two hosts. The chart shows relative throughput against C Oniguruma,
          averaged geometrically across tasks and hosts. Each case must reproduce the reference
          matches before it is timed.
        </>
      }
      note={
        <>
          Rust&rsquo;s <code>regex</code> crate and PCRE2 JIT win most text tasks they support. C
          Oniguruma wins some individual tasks, and compilation results are mixed. Compare engines
          for the syntax and workloads your application needs.
        </>
      }
    >
      <PerformanceChart />
      <details className="fr-comparison-details">
        <summary>Compare all seven engines</summary>
        <ComparisonTable
          align="end"
          caption="How much faster or slower Ferroni is than each engine."
          subject="Compared with"
          contenders={workloads.map((workload) => ({ id: workload.id, label: workload.short }))}
          rows={comparisonRows}
        />
        {tableNotes.map((note) => (
          <p key={note} className="fam-note">
            {note}
          </p>
        ))}
      </details>
      <Measured
        on={engineComparison.measured}
        machine={`Blacksmith runners. ${hosts}`}
        revision={<code>{engineComparison.commit.slice(0, 8)}</code>}
      >
        <Link to="/perf/engine-comparison">Per-host figures, every task and the raw data</Link>
      </Measured>
    </Section>
  );
}

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

export default function HomePage() {
  return (
    <div className="fam-page ferroni-home">
      <HeroSection />
      <HomeNavigation />
      <FitSection />
      <SampleSection />
      <SyntaxSection />
      <SpeedSection />
      <ScannerSection />
      <SafetySection />
      <ProofSection />
      <CoverageSection />
      <StatusSection />
      <RelationsSection />
      <ClosingSection />
      <WorkWithUs />
    </div>
  );
}
