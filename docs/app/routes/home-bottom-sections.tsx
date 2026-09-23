import { Mark } from "ferramenta-family";
import { Link } from "react-router";

/* -------------------------------------------------- */
/*  Two APIs                                          */
/* -------------------------------------------------- */

function RegexExample() {
  return (
    <figure className="fr-code">
      <figcaption>main.rs</figcaption>
      <pre>
        <span className="kw">use</span> <span className="ty">ferroni::prelude::*</span>;{"\n"}
        {"\n"}
        <span className="kw">fn</span> <span className="fn">main</span>() -&gt;{" "}
        <span className="ty">Result</span>&lt;(), <span className="ty">RegexError</span>&gt; {"{"}
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
    </figure>
  );
}

function ScannerExample() {
  return (
    <figure className="fr-code">
      <figcaption>highlight.rs</figcaption>
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
    </figure>
  );
}

export function CodeSection() {
  return (
    <section className="fr-section" aria-labelledby="fr-code">
      <div className="wrap">
        <h2 id="fr-code">Regex engine and scanner, one crate</h2>
        <p className="fr-intro">
          Match with the idiomatic Regex API, or tokenize with the multi-pattern Scanner that
          TextMate grammars need. Both come with <code>cargo add ferroni</code>.
        </p>
        <div className="fr-code-grid">
          <RegexExample />
          <ScannerExample />
        </div>
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
/*  Coverage ledger                                   */
/* -------------------------------------------------- */

const coverage = [
  {
    who: "TextMate grammars, VS Code, Shiki",
    status: "Covered",
    covered: true,
    detail:
      "vscode-oniguruma compiles every pattern as UTF-8. Ferroni's scanner keeps its API shape and its UTF-16 offsets.",
  },
  {
    who: "jq",
    status: "Covered",
    covered: true,
    detail: "jq matches on UTF-8 text, which Ferroni handles in full.",
  },
  {
    who: "PHP mbregex",
    status: "UTF-8 only",
    covered: false,
    detail: "mb_ereg in Shift_JIS, EUC-JP or another non-UTF-8 encoding is not covered.",
  },
  {
    who: "Ruby",
    status: "Not a target",
    covered: false,
    detail: "Ruby runs Onigmo, a fork of Oniguruma with its own history and encodings.",
  },
];

export function CoverageSection() {
  return (
    <section className="fr-section" aria-labelledby="fr-coverage">
      <div className="wrap">
        <h2 id="fr-coverage">What it covers</h2>
        <p className="fr-intro">
          Ferroni ports ASCII and UTF-8, two of Oniguruma&rsquo;s 29 encodings, and leaves out the
          POSIX and GNU APIs (<Link to="/adr/003-encoding-scope-ascii-and-utf8-only">ADR-003</Link>,{" "}
          <Link to="/adr/012-posix-and-gnu-api-not-ported">ADR-012</Link>). For the projects built
          on Oniguruma&rsquo;s syntax, that means:
        </p>
        <div className="fr-ledger">
          {coverage.map((row) => (
            <div key={row.who} className="fr-ledger-row">
              <b>{row.who}</b>
              <span className={row.covered ? "fr-stamp is-covered" : "fr-stamp"}>{row.status}</span>
              <p>{row.detail}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

/* -------------------------------------------------- */
/*  Closing action                                    */
/* -------------------------------------------------- */

export function ClosingSection() {
  return (
    <section className="fr-section fr-closing" aria-labelledby="fr-start">
      <div className="wrap">
        <h2 id="fr-start">Start building with Ferroni</h2>
        <p className="fr-intro">
          Oniguruma&rsquo;s engine and the vscode-oniguruma scanner in one pure-Rust crate. Verified
          against the upstream tests, measured against the C original.
        </p>
        <div className="fr-cta-row">
          <Link to="/guide/getting-started" className="fr-btn fr-btn-primary fr-chamfer">
            Read the guide <Mark name="arrow" className="icon" size={18} />
          </Link>
          <Link to="/perf/benchmark-results" className="fr-btn fr-btn-ghost">
            Benchmarks
          </Link>
        </div>
        <p className="fr-links">
          <a href="https://crates.io/crates/ferroni">crates.io/crates/ferroni</a>
          <a href="https://docs.rs/ferroni">docs.rs/ferroni</a>
          <a href="https://github.com/sebastian-software/ferroni/blob/main/LICENSE">BSD-2-Clause</a>
        </p>
      </div>
    </section>
  );
}
