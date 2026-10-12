import { ClosingAction, EvidenceFigures, Ledger, Mark, Section } from "ferramenta-family";
import { Link } from "react-router";
import { limitsHtml, lintHtml, scannerHtml } from "virtual:ferroni-sample";

import { HighlightedRust } from "../components/highlighted-rust";
import { ScannerWalkthrough } from "../components/home-examples";

/* -------------------------------------------------- */
/*  Scanner                                           */
/* -------------------------------------------------- */

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
      <HighlightedRust caption="highlight.rs" html={scannerHtml} />
      <ScannerWalkthrough />
    </Section>
  );
}

/* -------------------------------------------------- */
/*  Untrusted input                                   */
/* -------------------------------------------------- */

export function SafetySection() {
  return (
    <Section
      id="fr-safety"
      tone="dim"
      title="Put limits on untrusted input"
      intro={
        <>
          Ferroni is memory-safe, but matching uses backtracking. Set a timeout or retry limit for
          untrusted input, and use the compile-time warnings to reject risky patterns. Memory safety
          does not guarantee bounded matching time.
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
        <HighlightedRust caption="limits.rs" html={limitsHtml} />
        <HighlightedRust caption="lint.rs" html={lintHtml} />
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
      title="Compatibility you can verify"
      intro="Ferroni carries Oniguruma forward after its C project closed in April 2025. The port preserves module structure, function names and control flow, and passes every upstream UTF-8 test. Unicode tables and the Rust API extend that foundation."
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
        Install with <code>cargo add ferroni</code>. The guide walks through your first match,
        captures, scanner loops and per-search limits. For existing applications, start with the{" "}
        <Link to="/guide/compatibility">compatibility guide</Link>.
      </p>
    </ClosingAction>
  );
}
