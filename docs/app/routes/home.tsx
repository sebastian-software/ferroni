import "./home.css";
import { ArrowRight, Github, Layers, Package, ShieldCheck, Zap } from "ardo/icons";
import { Link, type MetaFunction } from "react-router";

import { CodeSection, CTASection, EcosystemSection } from "./home-bottom-sections";

// React Router requires `meta` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const meta: MetaFunction = () => [
  { title: "Ferroni — Pure-Rust Oniguruma Engine" },
  {
    name: "description",
    content:
      "Ferroni is a pure-Rust port of the Oniguruma regex engine. Full feature parity with the C original, ahead of Oniguruma across the measured runtime cases. No C toolchain required.",
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
          <span className="fh-headline-gradient">Regex, forged in Rust.</span>
        </h1>

        <p className="fh-tagline">
          Ferroni is a pure-Rust port of the Oniguruma regex engine &mdash; the engine behind Ruby,
          PHP, and TextMate grammars. Full feature parity with the C original, and ahead of
          Oniguruma across every measured runtime case in the reference suite.
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
  { value: "100%", label: "C parity" },
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
    icon: <Zap size={22} strokeWidth={1.5} />,
    title: "Built for scanner speed",
    text: "Tuned for the hot path in syntax highlighters and text scanners: first-match latency and full-line tokenization on real TextMate grammars run far ahead of Oniguruma. The measured factors are below.",
  },
  {
    icon: <ShieldCheck size={22} strokeWidth={1.5} />,
    title: "Full Oniguruma compatibility",
    text: "Named captures, variable-length lookbehind, conditionals, absent expressions, 886 Unicode properties, subexpression calls. If it works in Oniguruma, it works in Ferroni.",
  },
  {
    icon: <Package size={22} strokeWidth={1.5} />,
    title: "Pure Rust, no C toolchain",
    text: "cargo add ferroni and build. Cross-compiles to wasm32-unknown-unknown. No node-gyp, no local C compiler. Only 0.4% unsafe code, all documented.",
  },
  {
    icon: <Layers size={22} strokeWidth={1.5} />,
    title: "Built-in multi-pattern scanner",
    text: "Drop-in compatible with vscode-oniguruma. Regex engine and TextMate grammar scanner in a single dependency. Used by Shiki and VS Code.",
  },
];

function WhySection() {
  return (
    <section className="fh-section fh-why">
      <div className="fh-container">
        <div className="fh-why-header">
          <div className="fh-section-label">Why Ferroni</div>
          <h2 className="fh-section-title">Full compatibility. No compromises.</h2>
          <p className="fh-section-subtitle">
            Ferroni does not wrap Oniguruma. It ports the engine into Rust, keeps the same structure
            and optimization pipeline, then tunes the runtime path hard.
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
    label: "Scanner First Match",
    desc: "TypeScript grammar, 279 patterns",
    speedup: "58.9x",
    ferroni: "~425 ns",
    oniguruma: "~25 \u00B5s",
  },
  {
    category: "Syntax Highlighting",
    label: "Full Line Tokenization",
    desc: "TypeScript, end-to-end",
    speedup: "31.6x",
    ferroni: "~6.9 \u00B5s",
    oniguruma: "~217 \u00B5s",
  },
  {
    category: "Syntax Highlighting",
    label: "CSS Tokenization",
    desc: "Multi-pattern scanner workload",
    speedup: "11.3x",
    ferroni: "~1.3 ms",
    oniguruma: "~14.7 ms",
  },
  {
    category: "Text Search",
    label: "Rejection Speed",
    desc: "No match in 50 KB buffer",
    speedup: "6.1x",
    ferroni: "~1.5 \u00B5s",
    oniguruma: "~9.2 \u00B5s",
  },
  {
    category: "Text Search",
    label: "RegSet Multi-Pattern",
    desc: "5 patterns, simultaneous search",
    speedup: "3.9x",
    ferroni: "<100 ns",
    oniguruma: "~385 ns",
  },
  {
    category: "Pattern Matching",
    label: "Lookaround Combined",
    desc: "Feature most Rust engines skip",
    speedup: "3.6x",
    ferroni: "<80 ns",
    oniguruma: "~280 ns",
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
          Measured on 2026-03-06 with the <code>battle_bench</code> reference suite at commit{" "}
          <code>e8f120aa</code>, on a Mac13,2 (Apple M1 Ultra, 64&nbsp;GB) running macOS 26.3. Each
          factor is the ratio of the two timings shown on its card. Full tables, raw values, and the
          measurement context are in <Link to="/perf/benchmark-results">Benchmark Results</Link>.
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
