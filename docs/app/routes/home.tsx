import "./home.css";
import { familyGroups, isEngine, Mark } from "ferramenta-family";
import { Fragment } from "react";
import { Link, type MetaFunction } from "react-router";
import config from "virtual:ardo/config";

import { ClosingSection, CodeSection, CoverageSection } from "./home-bottom-sections";

// React Router requires `meta` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const meta: MetaFunction = () => [
  { title: "Ferroni — Oniguruma, continued in Rust" },
  {
    name: "description",
    content:
      "Ferroni continues the Oniguruma regex engine in memory-safe Rust after the C project ended, with the vscode-oniguruma scanner built in. Verified against the upstream tests, and 2.5x to 33x faster at tokenizing real code.",
  },
];

/*
 * The page follows the Ferramenta design system (ferramenta/DESIGN.md): the
 * shared tokens, display face and marks come from `ferramenta-family`; the
 * section patterns below (hero, iron band, pipeline assembly, evidence,
 * ledger) mirror ferramenta.dev and are candidates for the shared package.
 */

/* -------------------------------------------------- */
/*  Hero                                              */
/* -------------------------------------------------- */

function HeroSection() {
  const version = config.project?.version;
  return (
    <section className="fr-hero" aria-labelledby="fr-title">
      <div className="wrap">
        <div>
          <h1 id="fr-title">
            Oniguruma, <em>forged in Rust.</em>
          </h1>
          <p className="fr-lede">
            The regex engine behind TextMate grammars, jq and PHP&rsquo;s mbregex, continued in
            memory-safe Rust after the C project ended, with the vscode-oniguruma scanner built in.
            Verified against the upstream tests, and 2.5x to 33x faster at tokenizing real code.
          </p>
          <div className="fr-cta-row">
            <Link to="/guide/getting-started" className="fr-btn fr-btn-primary fr-chamfer">
              Get started <Mark name="arrow" className="icon" size={18} />
            </Link>
            <a href="https://github.com/sebastian-software/ferroni" className="fr-btn fr-btn-ghost">
              <Mark name="github" className="icon" size={18} /> GitHub
            </a>
          </div>
          <p className="fr-install">
            <code>cargo add ferroni</code>
            {version == null ? null : <span>v{version} on crates.io</span>}
          </p>
        </div>
        <span className="markplate fr-hero-plate" aria-hidden="true">
          <Mark name="ferroni" />
        </span>
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
/*  Carried forward (iron band)                       */
/* -------------------------------------------------- */

const pillars = [
  {
    heading: "Same engine, verified",
    text: "A line-by-line port that keeps Oniguruma's module structure and optimization pipeline, not a lookalike. Every upstream UTF-8 test passes, alongside 2,245 test functions in total.",
  },
  {
    heading: "Memory-safe, no C toolchain",
    text: "cargo add ferroni and build: no bindgen, no C compiler, no node-gyp. Unsafe code stays at 0.4%, and every block is documented in ADR-002.",
  },
  {
    heading: "Faster where highlighters work",
    text: "Tokenizing real code with complete TextMate grammars runs 2.5x to 33x faster than the C original, and text search up to 6x.",
  },
  {
    heading: "The scanner, built in",
    text: "vscode-textmate and Shiki tokenize through vscode-oniguruma's scanner. Ferroni ships a scanner of the same shape, UTF-16 offsets included, next to the regex engine.",
  },
];

function CarriedForwardSection() {
  return (
    <section className="fr-ironband" aria-labelledby="fr-forward">
      <div className="wrap">
        <h2 id="fr-forward">Oniguruma ended. The engine goes on.</h2>
        <p className="fr-intro">
          Oniguruma&rsquo;s C project closed on April 24, 2025, after more than twenty years as the
          regex engine that TextMate grammars are written for. Ferroni carries it forward.
        </p>
        <div className="fr-pillars">
          {pillars.map((pillar) => (
            <div key={pillar.heading}>
              <h3>{pillar.heading}</h3>
              <p>{pillar.text}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
/*  Pipeline                                          */
/* -------------------------------------------------- */

const fastenerPositions = ["tl", "tr", "br", "bl"] as const;

/** The content pipeline as a chamfered steel chassis, Ferroni marked as current. */
function PipelineAssembly() {
  const tools = familyGroups().pipeline.filter(isEngine);
  return (
    <figure
      className="fr-assembly"
      aria-label="Ferroni provides the regex engine for Ferriki, and Ferriki feeds highlighting into Ferromark."
    >
      {fastenerPositions.map((position) => (
        <span key={position} className="fastener" data-position={position} aria-hidden="true" />
      ))}
      <div className="fr-assembly-flow">
        <span className="fr-assembly-terminal">
          <small>Input</small>
          <b>TextMate grammars</b>
        </span>
        <Mark name="arrow" className="fr-assembly-connector icon" />
        {tools.map((tool, index) => (
          <Fragment key={tool.name}>
            <a
              className={
                tool.name === "ferroni" ? "fr-assembly-stage is-current" : "fr-assembly-stage"
              }
              href={tool.docs ?? tool.repo}
              aria-current={tool.name === "ferroni" ? "page" : undefined}
            >
              <span className="fr-assembly-step">{String(index + 1).padStart(2, "0")}</span>
              <span className="markplate">
                <Mark name={tool.name} />
              </span>
              <span className="fr-assembly-copy">
                <b>{tool.name}</b>
                <small>{tool.shortJob}</small>
              </span>
            </a>
            <Mark name="arrow" className="fr-assembly-connector icon" />
          </Fragment>
        ))}
        <span className="fr-assembly-terminal fr-assembly-output">
          <small>Output</small>
          <b>Markdown → highlighted HTML</b>
        </span>
      </div>
    </figure>
  );
}

function PipelineSection() {
  return (
    <section className="fr-section" aria-labelledby="fr-pipeline">
      <div className="wrap">
        <h2 id="fr-pipeline">Where Ferroni sits</h2>
        <p className="fr-intro">
          Ferroni is the foundation of the Ferramenta content pipeline. Ferriki, our
          Shiki-compatible highlighter, tokenizes TextMate grammars with Ferroni&rsquo;s scanner and
          hands highlighted code to Ferromark, our Markdown engine. We wanted that chain in Rust end
          to end, and it starts with the regex engine.
        </p>
        <PipelineAssembly />
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
/*  Evidence                                          */
/* -------------------------------------------------- */

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
    <section className="fr-section" aria-labelledby="fr-evidence">
      <div className="wrap fr-evidence">
        <div className="fr-evidence-head">
          <h2 id="fr-evidence">Faster on real code</h2>
          <p className="fr-intro">
            Each factor is Oniguruma&rsquo;s time divided by Ferroni&rsquo;s on the same input,
            higher is faster. The highlighting rows tokenize whole documents line by line, each line
            handed to the scanner once, the way vscode-textmate and Shiki drive it.
          </p>
          <p className="fr-aside">
            Measured on 2026-09-23 with the <code>battle_bench</code> reference suite at commit{" "}
            <code>2f109a75</code>, on a MacBookPro18,1 (Apple M1 Pro, 32&nbsp;GB) running macOS
            27.0. Full tables, raw values, and reproduction:{" "}
            <Link to="/perf/benchmark-results">Benchmark Results →</Link>
          </p>
        </div>
        <dl className="fr-figures">
          {benchmarks.map((b) => (
            <div key={b.label}>
              <dt>{b.label}</dt>
              <dd className="fr-figure">{b.speedup.replace("x", "×")}</dd>
              <dd className="fr-figure-detail">
                {b.desc}
                <span>
                  {b.ferroni} vs {b.oniguruma}
                </span>
              </dd>
            </div>
          ))}
        </dl>
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
/*  Page                                              */
/* -------------------------------------------------- */

export default function HomePage() {
  return (
    <div className="ferroni-home">
      <HeroSection />
      <CarriedForwardSection />
      <PipelineSection />
      <EvidenceSection />
      <CodeSection />
      <CoverageSection />
      <ClosingSection />
    </div>
  );
}
