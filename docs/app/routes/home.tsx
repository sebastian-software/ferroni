import {
  EvidenceFigures,
  family,
  IronBand,
  Mark,
  PipelineAssembly,
  ProjectHero,
  RegistryFacts,
  RunSample,
  Section,
  useToolFacts,
} from "ferramenta-family";
import { Link, type MetaFunction } from "react-router";

import sample from "../data/regex-sample.json";
import { ClosingSection, CodeSection, CoverageSection } from "./home-bottom-sections";

// React Router requires `meta` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const meta: MetaFunction = () => [
  { title: "Ferroni — Oniguruma, continued in Rust" },
  {
    name: "description",
    content:
      "Ferroni continues the Oniguruma regex engine in memory-safe Rust after the C project ended, with the vscode-oniguruma scanner built in. Verified against the upstream tests, and measured against C on real code.",
  },
];

function CurrentRelease() {
  const tool = family.find((entry) => entry.name === "ferroni");
  if (tool === undefined) throw new Error("Ferroni is missing from the family registry.");
  const facts = useToolFacts(tool);
  return (
    <a className="fr-release" href="https://crates.io/crates/ferroni">
      {facts.onCrates ? `v${facts.version} on crates.io` : "Releases on crates.io"}
    </a>
  );
}

function HeroSection() {
  return (
    <ProjectHero
      mark="ferroni"
      title={
        <span id="fr-title" translate="no">
          Ferroni
        </span>
      }
      lede={
        <>
          <strong>Oniguruma, continued in Rust.</strong> A regex engine in memory-safe Rust, with
          the vscode-oniguruma scanner built in for TextMate grammars. Verified against the upstream
          tests and measured against the C original.
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
      install={
        <>
          <code translate="no">cargo add ferroni</code>
          <CurrentRelease />
        </>
      }
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

function PipelineSection() {
  return (
    <Section
      id="fr-pipeline"
      title="Where Ferroni sits"
      intro={
        <>
          Ferroni supplies the regex engine for Ferriki, whose Shiki-compatible highlighter uses
          TextMate grammars to tokenize code. Ferromark combines that highlighting with Markdown
          rendering. Each tool also works on its own.
        </>
      }
    >
      <PipelineAssembly current="ferroni" />
    </Section>
  );
}

const benchmarks = [
  {
    category: "Syntax Highlighting",
    label: "TypeScript Document",
    desc: "279 patterns, 28 lines, line by line",
    speedup: "2.5x",
    ferroni: "~1.26 ms",
    oniguruma: "~3.13 ms",
  },
  {
    category: "Syntax Highlighting",
    label: "CSS Document",
    desc: "117 patterns, 19 lines, line by line",
    speedup: "32.6x",
    ferroni: "~93 µs",
    oniguruma: "~3.04 ms",
  },
  {
    category: "Syntax Highlighting",
    label: "Rust Document",
    desc: "81 patterns, 31 lines, line by line",
    speedup: "9.8x",
    ferroni: "~108 µs",
    oniguruma: "~1.06 ms",
  },
  {
    category: "Text Search",
    label: "Rejection Speed",
    desc: "No match in 50 KB buffer",
    speedup: "6.2x",
    ferroni: "~1.5 µs",
    oniguruma: "~9.3 µs",
  },
  {
    category: "Text Search",
    label: "RegSet Multi-Pattern",
    desc: "5 patterns, simultaneous search",
    speedup: "3.6x",
    ferroni: "~104 ns",
    oniguruma: "~370 ns",
  },
  {
    category: "Pattern Matching",
    label: "Lookaround Combined",
    desc: "Feature most Rust engines skip",
    speedup: "3.1x",
    ferroni: "~79 ns",
    oniguruma: "~247 ns",
  },
];

function EvidenceSection() {
  return (
    <Section
      id="fr-evidence"
      layout="split"
      title="Measured against C Oniguruma"
      intro={
        <>
          Each factor is Oniguruma&rsquo;s time divided by Ferroni&rsquo;s on the same input, higher
          is faster. The highlighting rows tokenize whole documents line by line, each line handed
          to the scanner once, the way vscode-textmate and Shiki drive it.
        </>
      }
      note={
        <>
          Reference measurements from 2026-09-23 with <code>battle_bench</code> at commit{" "}
          <code>2f109a75</code>, on a MacBookPro18,1 (Apple M1 Pro, 32&nbsp;GB), macOS 27.0. These
          predate the latest optimizations. Full tables and reproduction:{" "}
          <Link to="/perf/benchmark-results">Benchmark Results</Link>. More recent measurements:{" "}
          <Link to="/perf/simple-pattern-profiling">Simple-pattern profiling</Link>.
        </>
      }
    >
      <EvidenceFigures
        figures={benchmarks.map((benchmark) => ({
          label: benchmark.label,
          value: benchmark.speedup.replace("x", "×"),
          detail: benchmark.desc,
          measure: `${benchmark.ferroni} vs ${benchmark.oniguruma}`,
        }))}
      />
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
    <RegistryFacts>
      <div className="fam-page ferroni-home">
        <HeroSection />
        <IronBand
          id="fr-forward"
          title="Oniguruma ended. The engine goes on."
          intro="Oniguruma’s C project closed on April 24, 2025, after more than twenty years as the regex engine that TextMate grammars are written for. Ferroni carries it forward."
          rows={pillars}
        />
        <PipelineSection />
        <SampleSection />
        <EvidenceSection />
        <CodeSection />
        <CoverageSection />
        <ClosingSection />
      </div>
    </RegistryFacts>
  );
}
