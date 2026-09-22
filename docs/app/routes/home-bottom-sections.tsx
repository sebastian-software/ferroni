import { ArrowRight, ExternalLink, Github, Package } from "ardo/icons";
import { Link } from "react-router";

/*  Quick Start                                       */
/* -------------------------------------------------- */

// Keep the multi-line, syntax-highlighted Rust example readable in the JSX.
// eslint-disable-next-line max-lines-per-function
export function CodeSection() {
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
              <span className="kw">use</span> <span className="ty">ferroni::prelude::*</span>;{"\n"}
              {"\n"}
              <span className="kw">fn</span> <span className="fn">main</span>() -&gt;{" "}
              <span className="ty">Result</span>&lt;(), <span className="ty">RegexError</span>&gt;{" "}
              {"{"}
              {"\n"}
              {"    "}
              <span className="kw">let</span> re = <span className="ty">Regex</span>::
              <span className="fn">new</span>(
              <span className="str">
                r"(?&lt;year&gt;\d{"{4}"})-(?\u003cmonth\u003e\d{"{2}"})"
              </span>
              )?;{"\n"}
              {"\n"}
              {"    "}
              <span className="kw">let</span> caps = re.
              <span className="fn">captures</span>(<span className="str">"Date: 2026-02-12"</span>
              ).unwrap();{"\n"}
              {"    "}
              <span className="mc">assert_eq!</span>(caps.
              <span className="fn">name</span>(<span className="str">"year"</span>
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
  );
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
];

export function EcosystemSection() {
  return (
    <section className="fh-section fh-eco">
      <div className="fh-container">
        <div className="fh-eco-header">
          <div className="fh-section-label">Ecosystem</div>
          <h2 className="fh-section-title">The Oniguruma ecosystem, unlocked</h2>
          <p className="fh-section-subtitle">
            Ferroni works wherever Oniguruma does. These projects all depend on Oniguruma&rsquo;s
            feature set &mdash; and Ferroni covers it completely.
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
  );
}

/* -------------------------------------------------- */
/*  Final CTA                                         */
/* -------------------------------------------------- */

// Keep the paired calls to action grouped as one accessible section.
// eslint-disable-next-line max-lines-per-function
export function CTASection() {
  return (
    <section className="fh-section fh-final">
      <div className="fh-container fh-final-inner">
        <div className="fh-section-label">Get Started</div>
        <h2 className="fh-section-title">Start building with Ferroni</h2>
        <p className="fh-section-subtitle">
          Full Oniguruma compatibility with dramatically better performance. One dependency. Pure
          Rust.
        </p>

        <div className="fh-cta-group">
          <Link to="/guide/getting-started" className="fh-cta fh-cta-primary">
            Read the Docs <ArrowRight size={16} />
          </Link>
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
          <Link to="/perf/benchmark-results" className="fh-final-link">
            <ExternalLink size={14} /> Benchmarks
          </Link>
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
          <a href="https://oss.sebastian-software.com" target="_blank" rel="noopener noreferrer">
            Sebastian Software GmbH
          </a>
        </div>
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
