import "./home.css";
import { ArrowRight, Github, Layers, Package, ShieldCheck, Zap } from "ardo/icons";
import { Link, type MetaFunction } from "react-router";

import { CodeSection, CTASection, EcosystemSection } from "./home-bottom-sections";

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

/* -------------------------------------------------- */
/*  Logo                                              */
/* -------------------------------------------------- */

function FerroniLogo({ size = 80 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size * (64 / 52)}
      viewBox="0 0 52 64"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-label="Ferroni"
      role="img"
    >
      <defs>
        <linearGradient
          id="ferro-grad"
          x1="0"
          y1="0"
          x2="52"
          y2="64"
          gradientUnits="userSpaceOnUse"
        >
          <stop offset="0%" stopColor="var(--f-logo-from, #fbbf24)" />
          <stop offset="40%" stopColor="var(--f-logo-via, #f59e0b)" />
          <stop offset="100%" stopColor="var(--f-logo-to, #92400e)" />
        </linearGradient>
      </defs>

      {/* F body with angled right edges */}
      <path d="M 4 0 H 42 L 38 14 H 16 V 24 H 36 L 32 38 H 16 V 64 H 4 Z" fill="url(#ferro-grad)" />

      {/* Sparks */}
      <path
        d="M 46 0 L 49 4 L 46 8 L 43 4 Z"
        fill="var(--f-spark-fill, #f59e0b)"
        className="fh-spark-anim"
      />
      <circle
        cx="48"
        cy="14"
        r="1.8"
        fill="var(--f-spark-fill-dim, #d97706)"
        className="fh-spark-anim-delayed"
      />
      <circle cx="44" cy="19" r="1.1" fill="var(--f-spark-fill-dim, #d97706)" opacity="0.5" />
    </svg>
  );
}

/* -------------------------------------------------- */
/*  Hero                                              */
/* -------------------------------------------------- */

function HeroSection() {
  return (
    <section className="fh-hero">
      <div className="fh-hero-inner">
        <div className="fh-logo-wrap">
          <FerroniLogo size={80} />
        </div>

        <h1 className="fh-headline">
          <span className="fh-headline-gradient">Oniguruma, forged in Rust.</span>
        </h1>

        <p className="fh-tagline">
          The regex engine behind TextMate grammars, jq, and PHP&rsquo;s mbregex, continued in
          memory-safe Rust after the C project ended &mdash; with the vscode-oniguruma scanner built
          in. Verified against the upstream tests, and 2.5x to 33x faster at tokenizing real code.
        </p>

        <div className="fh-cta-group">
          <Link to="/guide/getting-started" className="fh-cta fh-cta-primary">
            Get Started <ArrowRight size={16} />
          </Link>
          <a
            href="https://github.com/sebastian-software/ferroni"
            className="fh-cta fh-cta-secondary"
            target="_blank"
            rel="noopener noreferrer"
          >
            <Github size={16} /> GitHub
          </a>
        </div>
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
/*  Stats                                             */
/* -------------------------------------------------- */

const stats = [
  { value: "2,195", label: "Test functions" },
  { value: "100%", label: "Upstream UTF-8 tests" },
  { value: "0.4%", label: "Unsafe code" },
  { value: "BSD-2", label: "License" },
];

function StatsSection() {
  return (
    <section className="fh-stats">
      <div className="fh-stats-grid">
        {stats.map((s) => (
          <div key={s.label} className="fh-stat">
            <div className="fh-stat-value">{s.value}</div>
            <div className="fh-stat-label">{s.label}</div>
          </div>
        ))}
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
/*  Why Ferroni                                       */
/* -------------------------------------------------- */

const features = [
  {
    icon: <ShieldCheck size={22} strokeWidth={1.5} />,
    title: "Same engine, verified",
    text: "A line-by-line port, not a lookalike: named captures, variable-length look-behind, conditionals, absent expressions, 886 Unicode properties, 12 syntaxes. Every upstream UTF-8 test passes.",
  },
  {
    icon: <Package size={22} strokeWidth={1.5} />,
    title: "Memory-safe, no C toolchain",
    text: "cargo add ferroni and build. No bindgen, no C compiler, no node-gyp. 0.4% unsafe code, every block documented.",
  },
  {
    icon: <Zap size={22} strokeWidth={1.5} />,
    title: "Faster where highlighters work",
    text: "Tokenizing real code with complete TextMate grammars runs 2.5x to 33x faster than the C original. The measured factors are below.",
  },
  {
    icon: <Layers size={22} strokeWidth={1.5} />,
    title: "The vscode-oniguruma scanner, built in",
    text: "vscode-textmate and Shiki tokenize through vscode-oniguruma\u2019s scanner. Ferroni ships a scanner of the same shape, UTF-16 offsets included, next to the regex engine.",
  },
];

function WhySection() {
  return (
    <section className="fh-section fh-why">
      <div className="fh-container">
        <div className="fh-why-header">
          <div className="fh-section-label">Why Ferroni</div>
          <h2 className="fh-section-title">Same engine. Carried forward.</h2>
          <p className="fh-section-subtitle">
            Oniguruma&rsquo;s C project ended in April 2025. Ferroni does not wrap it: it ports the
            engine into Rust, keeps the same structure and optimization pipeline, then tunes the
            path highlighters take.
          </p>
        </div>

        <div className="fh-cards">
          {features.map((f) => (
            <div key={f.title} className="fh-card">
              <div className="fh-card-icon">{f.icon}</div>
              <h3 className="fh-card-title">{f.title}</h3>
              <p className="fh-card-text">{f.text}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
/*  Performance                                       */
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
    ferroni: "~93 \u00B5s",
    oniguruma: "~3.04 ms",
  },
  {
    category: "Syntax Highlighting",
    label: "Rust Document",
    desc: "81 patterns, 31 lines, line by line",
    speedup: "9.8x",
    ferroni: "~108 \u00B5s",
    oniguruma: "~1.06 ms",
  },
  {
    category: "Text Search",
    label: "Rejection Speed",
    desc: "No match in 50 KB buffer",
    speedup: "6.2x",
    ferroni: "~1.5 \u00B5s",
    oniguruma: "~9.3 \u00B5s",
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

function PerfSection() {
  return (
    <section className="fh-section fh-perf">
      <div className="fh-container">
        <div className="fh-perf-header">
          <div className="fh-section-label">Performance</div>
          <h2 className="fh-section-title">Measured, not claimed.</h2>
          <p className="fh-section-subtitle">
            Every number comes from battle_bench, a head-to-head benchmark suite running Ferroni
            against Oniguruma on the same inputs. No cherry-picked subsets.
          </p>
        </div>

        <div className="fh-perf-grid">
          {benchmarks.map((b) => (
            <div key={b.label} className="fh-perf-card">
              <div className="fh-perf-category">{b.category}</div>
              <div className="fh-perf-label">{b.label}</div>
              <div className="fh-perf-desc">{b.desc}</div>
              <div className="fh-perf-speedup">{b.speedup}</div>
              <div className="fh-perf-speedup-label">faster</div>
              <div className="fh-perf-times">
                <div className="fh-perf-time">
                  <span className="fh-perf-time-engine">Ferroni</span>
                  <span className="fh-perf-time-value is-ferroni">{b.ferroni}</span>
                </div>
                <div className="fh-perf-time">
                  <span className="fh-perf-time-engine">Oniguruma</span>
                  <span className="fh-perf-time-value">{b.oniguruma}</span>
                </div>
              </div>
            </div>
          ))}
        </div>

        <p className="fh-perf-note">
          Measured on 2026-09-23 with the <code>battle_bench</code> reference suite at commit{" "}
          <code>374e3fed</code>, on a MacBookPro18,1 (Apple M1 Pro, 32&nbsp;GB) running macOS 27.0.
          Each factor is the ratio of the two timings shown on its card. The syntax highlighting
          cards tokenize a whole document line by line, each line handed to the scanner once, the
          way vscode-textmate and Shiki drive it. Full tables, raw values, and the measurement
          context are in <Link to="/perf/benchmark-results">Benchmark Results</Link>.
        </p>
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
      <StatsSection />
      <WhySection />
      <PerfSection />
      <CodeSection />
      <EcosystemSection />
      <CTASection />
    </div>
  );
}
