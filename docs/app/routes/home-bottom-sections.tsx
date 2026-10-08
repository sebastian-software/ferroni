import {
  ClosingAction,
  CodePanel,
  EvidenceFigures,
  Ledger,
  Mark,
  Section,
} from "ferramenta-family";
import { Link } from "react-router";

/* -------------------------------------------------- */
/*  Scanner                                           */
/* -------------------------------------------------- */

function ScannerExample() {
  return (
    <CodePanel caption="highlight.rs">
      <span className="kw">use</span> <span className="ty">ferroni::prelude::*</span>;{"\n"}
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
    </CodePanel>
  );
}

export function ScannerSection() {
  return (
    <Section
      id="fr-scanner"
      layout="split"
      title="Built for syntax highlighting"
      intro="A highlighter asks, at every position in a line, which of a grammar’s many patterns matches next. Ferroni’s Scanner answers with the API of vscode-oniguruma, the engine under vscode-textmate and Shiki, UTF-16 offsets included."
      note={
        <>
          Scanners built from one pattern cache compile each distinct pattern once, and a string id
          lets a scanner reuse what it learned about a line.{" "}
          <Link to="/guide/getting-started#the-scanner-api">The Scanner API in the guide</Link>
        </>
      }
    >
      <ScannerExample />
    </Section>
  );
}

/* -------------------------------------------------- */
/*  Untrusted input                                   */
/* -------------------------------------------------- */

function LimitsExample() {
  return (
    <CodePanel caption="limits.rs">
      <span className="kw">use</span> <span className="ty">std::time::Duration</span>;{"\n"}
      <span className="kw">use</span> <span className="ty">ferroni::prelude::*</span>;{"\n"}
      {"\n"}
      <span className="kw">let</span> re = <span className="ty">Regex</span>::
      <span className="fn">new</span>(<span className="str">r"(a+)+b"</span>).unwrap();{"\n"}
      <span className="kw">let</span> options = <span className="ty">SearchOptions</span>::
      <span className="fn">new</span>(){"\n"}
      {"    "}.<span className="fn">timeout</span>(<span className="ty">Duration</span>::
      <span className="fn">from_millis</span>(50));{"\n"}
      {"\n"}
      <span className="kw">let</span> hostile = <span className="str">"a"</span>.
      <span className="fn">repeat</span>(40);{"\n"}
      <span className="kw">let</span> result = re.<span className="fn">find_with</span>
      (&amp;hostile, options);{"\n"}
      <span className="mc">assert!</span>(<span className="mc">matches!</span>(result,{" "}
      <span className="ty">Err</span>(<span className="ty">RegexError</span>::TimeLimitOver)));
    </CodePanel>
  );
}

function LintExample() {
  return (
    <CodePanel caption="lint.rs">
      <span className="kw">use</span> <span className="ty">ferroni::prelude::*</span>;{"\n"}
      {"\n"}
      <span className="kw">let</span> re = <span className="ty">Regex</span>::
      <span className="fn">new</span>(<span className="str">r"(a+)+$"</span>).unwrap();{"\n"}
      <span className="kw">for</span> warning <span className="kw">in</span> re.
      <span className="fn">backtracking_warnings</span>() {"{"}
      {"\n"}
      {"    "}
      <span className="mc">eprintln!</span>(<span className="str">"{"{warning}"}"</span>);{" "}
      <span className="cm">&#47;&#47; nested unbounded repeat: …</span>
      {"\n"}
      {"}"}
      {"\n"}
      {"\n"}
      <span className="cm">&#47;&#47; Refuse risky patterns that users supply.</span>
      {"\n"}
      <span className="kw">let</span> strict = <span className="ty">Regex</span>::
      <span className="fn">builder</span>(<span className="str">r"(a+)+$"</span>){"\n"}
      {"    "}.<span className="fn">reject_backtracking_risks</span>(
      <span className="kw">true</span>){"\n"}
      {"    "}.<span className="fn">build</span>();{"\n"}
      <span className="mc">assert!</span>(strict.<span className="fn">is_err</span>());
    </CodePanel>
  );
}

export function SafetySection() {
  return (
    <Section
      id="fr-safety"
      tone="dim"
      title="Safe with untrusted input"
      intro={
        <>
          Memory-safe Rust replaces the C code behind Oniguruma&rsquo;s CVEs, and{" "}
          <code>unsafe</code> stays confined to the links between parse-tree nodes. For text and
          patterns you did not write, a search can carry a timeout or a retry limit and report it as
          an error, and a compile-time check flags patterns that backtrack catastrophically.
        </>
      }
      note={
        <>
          Pattern compilation, matching and the scanner are fuzzed on every pull request.{" "}
          <Link to="/guide/untrusted-input">Untrusted input in the guide</Link>
          {" · "}
          <Link to="/adr/002-unsafe-code-policy">The unsafe code policy</Link>
        </>
      }
    >
      <div className="fam-code-grid">
        <LimitsExample />
        <LintExample />
      </div>
    </Section>
  );
}

/* -------------------------------------------------- */
/*  Compatibility evidence                            */
/* -------------------------------------------------- */

/* The parity counts are the totals of the table in guide/compatibility.mdx. */
const proof = [
  {
    label: "Upstream C test cases",
    value: "2,974 / 2,974",
    detail: "Every UTF-8 test file of Oniguruma, ported case by case.",
  },
  {
    label: "vscode-oniguruma tests",
    value: "15 / 15",
    detail: "The scanner's upstream suite, ported as 25 Rust tests.",
  },
  {
    label: "Unicode data",
    value: "18.0",
    detail: "Generated from the Unicode Character Database; C Oniguruma ships 16.0.",
  },
  {
    label: "Syntax modes",
    value: "12",
    detail: "Oniguruma, Ruby, Perl, Python, Java, POSIX and six more.",
  },
];

export function ProofSection() {
  return (
    <Section
      id="fr-proof"
      layout="split"
      title="Compatibility, with the evidence attached"
      intro="The port keeps Oniguruma’s module structure, function names and control flow, so it can be checked against C test by test. Every upstream test that targets UTF-8 passes."
      note={
        <>
          Line coverage is gated in CI, and every benchmark case first has to reproduce C
          Oniguruma&rsquo;s results.{" "}
          <Link to="/guide/compatibility">How compatibility is checked</Link>
        </>
      }
    >
      <EvidenceFigures figures={proof} />
    </Section>
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
    <Section
      id="fr-coverage"
      title="What it covers"
      intro={
        <>
          Ferroni ports ASCII and UTF-8, two of Oniguruma&rsquo;s 29 encodings, and leaves out the
          POSIX and GNU APIs (<Link to="/adr/003-encoding-scope-ascii-and-utf8-only">ADR-003</Link>,{" "}
          <Link to="/adr/012-posix-and-gnu-api-not-ported">ADR-012</Link>). For the projects built
          on Oniguruma&rsquo;s syntax, that means:
        </>
      }
    >
      <Ledger
        entries={coverage.map((row) => ({
          name: row.who,
          status: row.status,
          settled: row.covered,
          detail: row.detail,
        }))}
      />
    </Section>
  );
}

export function ClosingSection() {
  return (
    <ClosingAction
      id="fr-start"
      title="Start building with Ferroni"
      actions={
        <>
          <Link to="/guide/getting-started" className="fam-btn fam-btn-primary">
            Read the guide <Mark name="arrow" className="icon" size={18} />
          </Link>
          <Link to="/perf/engine-comparison" className="fam-btn fam-btn-ghost">
            Benchmarks
          </Link>
        </>
      }
      links={
        <>
          <a href="https://crates.io/crates/ferroni">crates.io/crates/ferroni</a>
          <a href="https://docs.rs/ferroni">docs.rs/ferroni</a>
          <a href="https://github.com/sebastian-software/ferroni">GitHub</a>
          <a href="https://github.com/sebastian-software/ferroni/blob/main/LICENSE">BSD-2-Clause</a>
        </>
      }
    >
      <p className="fam-intro">
        Oniguruma&rsquo;s engine and the vscode-oniguruma scanner in one pure-Rust crate. The guide
        takes you from <code>cargo add ferroni</code> to your first match, a tokenizer loop, and
        searches that are safe to run on untrusted input.
      </p>
    </ClosingAction>
  );
}
