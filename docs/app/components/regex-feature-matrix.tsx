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
      <a href="https://docs.rs/regex/latest/regex/">Rust regex</a>. The{" "}
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
        caption="Selected regex syntax · checked with untimed compatibility examples"
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
        Yes means the illustrated native syntax is accepted and returns the expected match. No
        refers to that syntax; a rewritten expression may solve the same task. PCRE2&rsquo;s
        interpreter and JIT both pass the supported examples here. fancy-regex uses Oniguruma mode.
        This is a feature comparison, not a claim that all accepted patterns have identical
        semantics. On small screens, scroll the matrix horizontally.
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
