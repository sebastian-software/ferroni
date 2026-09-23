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
          <h2 className="fh-section-title">Regex engine and scanner, one crate</h2>
          <p className="fh-section-subtitle">
            Match with the idiomatic Regex API, or tokenize with the multi-pattern Scanner that
            TextMate grammars need.
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
                r"(?&lt;year&gt;\d{"{4}"})-(?&lt;month&gt;\d{"{2}"})"
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

          <div className="fh-code-tabs fh-code-tabs-next">
            <span className="fh-code-tab is-active">highlight.rs</span>
          </div>
          <div className="fh-code-block">
            <pre>
              <span className="kw">use</span>{" "}
              <span className="ty">
                ferroni::scanner::{"{"}Scanner, ScannerFindOptions{"}"}
              </span>
              ;{"\n"}
              {"\n"}
              <span className="kw">let mut</span> scanner = <span className="ty">Scanner</span>::
              <span className="fn">new</span>(&amp;[{"\n"}
              {"    "}
              <span className="str">r"\b(function|const|let|var)\b"</span>,{"\n"}
              {"    "}
              <span className="str">r#""[^"]*""#</span>,{"\n"}
              {"    "}
              <span className="str">r"&#47;&#47;.*$"</span>,{"\n"}
              ]).unwrap();{"\n"}
              {"\n"}
              <span className="kw">let</span> m = scanner{"\n"}
              {"    "}.<span className="fn">find_next_match</span>(
              <span className="str">r#"const x = "hello""#</span>, 0,{" "}
              <span className="ty">ScannerFindOptions</span>::NONE){"\n"}
              {"    "}.unwrap();{"\n"}
              <span className="mc">assert_eq!</span>(m.index, 0);{" "}
              <span className="cm">&#47;&#47; "const" matched first</span>
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
  { name: "TextMate", role: "Grammar syntax" },
  { name: "VS Code", role: "Scanner via vscode-oniguruma" },
  { name: "Shiki", role: "Scanner via vscode-oniguruma" },
  { name: "jq", role: "Regex engine" },
  { name: "PHP", role: "mbregex (mb_ereg)" },
  { name: "Ruby", role: "Via its fork, Onigmo" },
];

export function EcosystemSection() {
  return (
    <section className="fh-section fh-eco">
      <div className="fh-container">
        <div className="fh-eco-header">
          <div className="fh-section-label">Ecosystem</div>
          <h2 className="fh-section-title">Built on Oniguruma&rsquo;s semantics</h2>
          <p className="fh-section-subtitle">
            These projects depend on Oniguruma&rsquo;s syntax and behavior. Ferroni brings them to
            Rust for ASCII and UTF-8 text, the encoding TextMate scanning and jq run on.
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
          Oniguruma&rsquo;s engine and the vscode-oniguruma scanner in one pure-Rust crate. Verified
          against the upstream tests, measured against the C original.
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
