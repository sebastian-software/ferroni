// cspell:ignore regexr strasse straße
import { type ComparisonRow, ComparisonTable } from "ferramenta-family";
import { Link } from "react-router";

import capabilities from "../data/regex-capabilities.json";
import "./regex-feature-matrix.css";

const rows: ComparisonRow[] = capabilities.features.map((feature) => ({
  label: feature.label,
  values: feature.support,
}));

function MatrixSources() {
  return (
    <p className="regex-feature-note">
      Sources: <a href="https://github.com/kkos/oniguruma/blob/master/doc/RE">Oniguruma syntax</a>,{" "}
      <a href="https://www.pcre.org/current/doc/html/pcre2syntax.html">PCRE2 syntax</a>,{" "}
      <a href="https://docs.rs/regex/latest/regex/">Rust regex</a>,{" "}
      <a href="https://docs.rs/regexr/0.6.1/regexr/">regexr 0.6.1</a>. The{" "}
      <a href="https://github.com/sebastian-software/ferroni/blob/main/tests/regex_feature_matrix.rs">
        executable matrix checks
      </a>{" "}
      use the pinned dependencies and native sources. See{" "}
      <Link to="/guide/compatibility">compatibility and scope</Link>.
    </p>
  );
}

export function RegexFeatureMatrix() {
  return (
    <div className="regex-feature-matrix">
      <h3>Choose the syntax your application needs</h3>
      <p>
        Ferroni brings Oniguruma&rsquo;s pattern behavior to a Rust-only build. PCRE2 also supports
        lookaround, backreferences and recursion; the dialect and implementation language still
        differ. Rust <code>regex</code> offers guaranteed linear-time matching with a smaller
        syntax. These are selected features of the configurations used in the comparison.
      </p>
      <ComparisonTable
        caption="Selected regex features · checked with untimed compatibility examples"
        subject="Feature"
        contenders={capabilities.engines.map((engine) => ({
          id: engine.id,
          label: (
            <>
              {engine.label}
              <small>{engine.language}</small>
            </>
          ),
          own: engine.id === "rust",
        }))}
        rows={rows}
      />
      <p className="regex-feature-note">
        Yes means the illustrated syntax returns the expected match. No means that syntax is
        rejected or its behavior differs; a rewritten expression may solve the same task. PCRE2 uses
        UTF and UCP, and its interpreter and JIT both pass its supported examples. fancy-regex uses
        Oniguruma mode; regexr uses Unicode mode with JIT off. This is a feature comparison, not a
        claim that all accepted patterns have identical semantics. On small screens, scroll the
        matrix horizontally.
      </p>
      <p className="regex-feature-note">
        Variable-length lookbehind here includes an unbounded <code>\s*</code>. PCRE2 supports
        bounded variable-length lookbehind. Multi-character case folding means, for example,
        <code>strasse</code> matching <code>straße</code> with case ignored. Unicode word
        segmentation uses UAX #29 rather than the ordinary <code>\b</code> word-character boundary.
      </p>
      <p className="regex-feature-note">
        The performance corpus measures real regex steps and covers only part of this syntax. Absent
        repeaters and recursion-level backreferences have no timings in that corpus. Ferroni&rsquo;s
        value is preserving Oniguruma behavior in Rust, with current Unicode data and a{" "}
        <Link to="/guide/getting-started#the-scanner-api">TextMate Scanner API</Link>.
      </p>
      <MatrixSources />
    </div>
  );
}
