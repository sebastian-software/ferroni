import { Mark } from "ferramenta-family";
import highlightedInput from "virtual:ferroni-sample";

import sample from "../data/regex-sample.json";
import { HighlightedRust } from "./highlighted-rust";

export function HomeRunSample() {
  return (
    <div className="fam-run fr-run">
      <div className="fam-run-input">
        <div className="fam-run-wide">
          <HighlightedRust caption="website_sample.rs" html={highlightedInput} />
        </div>
        <details className="fam-run-narrow">
          <summary>
            <span>website_sample.rs</span>
            <small>Rust source</small>
            <Mark name="chev" className="icon" size={16} />
          </summary>
          <HighlightedRust caption="website_sample.rs" html={highlightedInput} />
        </details>
      </div>
      <Mark name="arrow" className="icon fam-run-arrow" />
      <figure className="fam-run-output">
        <figcaption>Ferroni {sample.version} · stdout</figcaption>
        <pre tabIndex={0}>
          <code>{sample.stdout.trimEnd()}</code>
        </pre>
      </figure>
    </div>
  );
}
