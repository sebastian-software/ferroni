import "./home.css"
import {
  ArrowRight,
  Github,
  Zap,
  ShieldCheck,
  Package,
  Layers,
  ExternalLink,
} from "ardo/icons"
import type { MetaFunction } from "react-router"

export const meta: MetaFunction = () => [
  { title: "Ferroni — Pure-Rust Oniguruma Engine" },
  {
    name: "description",
    content:
      "Ferroni is a pure-Rust port of the Oniguruma regex engine. Full feature parity with the C original. Up to 59x faster on scanner workloads. No C toolchain required.",
  },
]

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
      <path
        d="M 4 0 H 42 L 38 14 H 16 V 24 H 36 L 32 38 H 16 V 64 H 4 Z"
        fill="url(#ferro-grad)"
      />

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
      <circle
        cx="44"
        cy="19"
        r="1.1"
        fill="var(--f-spark-fill-dim, #d97706)"
        opacity="0.5"
      />
    </svg>
  )
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
          <span className="fh-headline-gradient">
            Regex, forged in Rust.
          </span>
        </h1>

        <p className="fh-tagline">
          Ferroni is a pure-Rust port of the Oniguruma regex engine &mdash; the
          engine behind Ruby, PHP, and TextMate grammars. Full feature parity
          with the C original. Up to 59x faster on scanner workloads.
        </p>

        <div className="fh-cta-group">
          <a href="/guide/getting-started" className="fh-cta fh-cta-primary">
            Get Started <ArrowRight size={16} />
          </a>
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
  )
}

/* -------------------------------------------------- */
/*  Stats                                             */
/* -------------------------------------------------- */

const stats = [
  { value: "2,090", label: "Tests passing" },
  { value: "100%", label: "C parity" },
  { value: "0.4%", label: "Unsafe code" },
  { value: "BSD-2", label: "License" },
]

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
  )
}

/* -------------------------------------------------- */
/*  Why Ferroni                                       */
/* -------------------------------------------------- */

const features = [
  {
    icon: <Zap size={22} strokeWidth={1.5} />,
    title: "Built for scanner speed",
    text: "Up to 59x faster first-match latency. 31x faster full-line tokenization. Tuned for the hot path in syntax highlighters and text scanners.",
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
]

function WhySection() {
  return (
    <section className="fh-section fh-why">
      <div className="fh-container">
        <div className="fh-why-header">
          <div className="fh-section-label">Why Ferroni</div>
          <h2 className="fh-section-title">
            Full compatibility. No compromises.
          </h2>
          <p className="fh-section-subtitle">
            Ferroni does not wrap Oniguruma. It ports the engine into Rust,
            keeps the same structure and optimization pipeline, then tunes the
            runtime path hard.
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
  )
}

/* -------------------------------------------------- */
/*  Performance                                       */
/* -------------------------------------------------- */

const benchmarks = [
  {
    category: "Syntax Highlighting",
    label: "Scanner First Match",
    desc: "TypeScript grammar, 279 patterns",
    speedup: "59x",
    ferroni: "~425 ns",
    oniguruma: "~25 \u00B5s",
  },
  {
    category: "Syntax Highlighting",
    label: "Full Line Tokenization",
    desc: "TypeScript, end-to-end",
    speedup: "31x",
    ferroni: "~6.9 \u00B5s",
    oniguruma: "~217 \u00B5s",
  },
  {
    category: "Syntax Highlighting",
    label: "CSS Tokenization",
    desc: "Multi-pattern scanner workload",
    speedup: "11x",
    ferroni: "~1.3 ms",
    oniguruma: "~14.7 ms",
  },
  {
    category: "Text Search",
    label: "Rejection Speed",
    desc: "No match in 50 KB buffer",
    speedup: "6x",
    ferroni: "~1.5 \u00B5s",
    oniguruma: "~9.2 \u00B5s",
  },
  {
    category: "Text Search",
    label: "RegSet Multi-Pattern",
    desc: "5 patterns, simultaneous search",
    speedup: "3.8x",
    ferroni: "<100 ns",
    oniguruma: "~385 ns",
  },
  {
    category: "Pattern Matching",
    label: "Lookaround Combined",
    desc: "Feature most Rust engines skip",
    speedup: "3.5x",
    ferroni: "<80 ns",
    oniguruma: "~280 ns",
  },
]

function PerfSection() {
  return (
    <section className="fh-section fh-perf">
      <div className="fh-container">
        <div className="fh-perf-header">
          <div className="fh-section-label">Performance</div>
          <h2 className="fh-section-title">Measured, not claimed.</h2>
          <p className="fh-section-subtitle">
            Every number comes from battle_bench, a head-to-head benchmark suite
            running Ferroni against Oniguruma on the same inputs. No
            cherry-picked subsets.
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
                  <span className="fh-perf-time-value is-ferroni">
                    {b.ferroni}
                  </span>
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
          Full benchmark tables and methodology in{" "}
          <a
            href="https://github.com/sebastian-software/ferroni/blob/main/docs/perf/benchmark-results.md"
            target="_blank"
            rel="noopener noreferrer"
          >
            docs/perf/benchmark-results.md
          </a>
        </p>
      </div>
    </section>
  )
}

/* -------------------------------------------------- */
/*  Quick Start                                       */
/* -------------------------------------------------- */

function CodeSection() {
  return (
    <section className="fh-section fh-code">
      <div className="fh-container">
        <div className="fh-code-header">
          <div className="fh-section-label">Quick Start</div>
          <h2 className="fh-section-title">Three lines to your first match</h2>
          <p className="fh-section-subtitle">
            Add Ferroni as a dependency. Write a pattern. Match.
          </p>
        </div>

        <div className="fh-code-wrapper">
          <div className="fh-code-tabs">
            <span className="fh-code-tab is-active">main.rs</span>
          </div>
          <div className="fh-code-block">
            <pre>
              <span className="kw">use</span>{" "}
              <span className="ty">ferroni::prelude::*</span>;{"\n"}
              {"\n"}
              <span className="kw">fn</span>{" "}
              <span className="fn">main</span>() -&gt;{" "}
              <span className="ty">Result</span>&lt;(),{" "}
              <span className="ty">RegexError</span>&gt; {"{"}
              {"\n"}
              {"    "}
              <span className="kw">let</span> re ={" "}
              <span className="ty">Regex</span>::
              <span className="fn">new</span>(
              <span className="str">
                r"(?&lt;year&gt;\d{"{4}"})-(?\u003cmonth\u003e\d{"{2}"})"
              </span>
              )?;{"\n"}
              {"\n"}
              {"    "}
              <span className="kw">let</span> caps = re.
              <span className="fn">captures</span>(
              <span className="str">"Date: 2026-02-12"</span>
              ).unwrap();{"\n"}
              {"    "}
              <span className="mc">assert_eq!</span>(caps.
              <span className="fn">name</span>(
              <span className="str">"year"</span>
              ).unwrap().as_str(), <span className="str">"2026"</span>);{"\n"}
              {"    "}
              <span className="ty">Ok</span>(()){"\n"}
              {"}"}
            </pre>
          </div>

          <div className="fh-install-line">
            <span className="prompt">$</span>
            cargo add ferroni
          </div>
        </div>
      </div>
    </section>
  )
}

/* -------------------------------------------------- */
/*  Ecosystem                                         */
/* -------------------------------------------------- */

const ecosystem = [
  { name: "Ruby", role: "Core regex engine" },
  { name: "PHP", role: "mbstring module" },
  { name: "TextMate", role: "Grammar syntax" },
  { name: "jq", role: "Pattern matching" },
  { name: "Shiki", role: "Syntax highlighting" },
  { name: "VS Code", role: "Token engine" },
]

function EcosystemSection() {
  return (
    <section className="fh-section fh-eco">
      <div className="fh-container">
        <div className="fh-eco-header">
          <div className="fh-section-label">Ecosystem</div>
          <h2 className="fh-section-title">
            The Oniguruma ecosystem, unlocked
          </h2>
          <p className="fh-section-subtitle">
            Ferroni works wherever Oniguruma does. These projects all depend on
            Oniguruma&rsquo;s feature set &mdash; and Ferroni covers it
            completely.
          </p>
        </div>

        <div className="fh-eco-grid">
          {ecosystem.map((e) => (
            <div key={e.name} className="fh-eco-item">
              <div className="fh-eco-name">{e.name}</div>
              <div className="fh-eco-role">{e.role}</div>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

/* -------------------------------------------------- */
/*  Final CTA                                         */
/* -------------------------------------------------- */

function CTASection() {
  return (
    <section className="fh-section fh-final">
      <div className="fh-container fh-final-inner">
        <div className="fh-section-label">Get Started</div>
        <h2 className="fh-section-title">Start building with Ferroni</h2>
        <p className="fh-section-subtitle">
          Full Oniguruma compatibility with dramatically better performance.
          One dependency. Pure Rust.
        </p>

        <div className="fh-cta-group">
          <a href="/guide/getting-started" className="fh-cta fh-cta-primary">
            Read the Docs <ArrowRight size={16} />
          </a>
          <a
            href="https://crates.io/crates/ferroni"
            className="fh-cta fh-cta-secondary"
            target="_blank"
            rel="noopener noreferrer"
          >
            <Package size={16} /> crates.io
          </a>
        </div>

        <div className="fh-final-links">
          <a
            href="https://github.com/sebastian-software/ferroni"
            className="fh-final-link"
            target="_blank"
            rel="noopener noreferrer"
          >
            <Github size={14} /> GitHub
          </a>
          <a
            href="https://github.com/sebastian-software/ferroni/blob/main/docs/perf/benchmark-results.md"
            className="fh-final-link"
            target="_blank"
            rel="noopener noreferrer"
          >
            <ExternalLink size={14} /> Benchmarks
          </a>
          <a
            href="https://github.com/sebastian-software/ferroni/blob/main/LICENSE"
            className="fh-final-link"
            target="_blank"
            rel="noopener noreferrer"
          >
            <ExternalLink size={14} /> BSD-2-Clause
          </a>
        </div>

        <div className="fh-footer-copy">
          Copyright 2026{" "}
          <a
            href="https://www.sebastian-software.de/"
            target="_blank"
            rel="noopener noreferrer"
          >
            Sebastian Software GmbH
          </a>
        </div>
      </div>
    </section>
  )
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
  )
}
