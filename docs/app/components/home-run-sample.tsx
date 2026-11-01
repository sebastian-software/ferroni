import { FerraExample } from "ferramenta-family/ferra";

import sample from "../data/regex-sample.json";

export function HomeRunSample() {
  return (
    <FerraExample
      title="A date, captured by name"
      introduction="Look behind the label, then read the year, month and day."
      source={sample.input.replaceAll(/^\/\/!.*\n/gm, "").trimStart()}
      footer={
        <a href="https://github.com/sebastian-software/ferroni/blob/main/examples/website_sample.rs">
          Runnable Rust source and verified output
        </a>
      }
    >
      <pre tabIndex={0} aria-label="Example output">
        <code>{sample.stdout.trimEnd()}</code>
      </pre>
    </FerraExample>
  );
}
