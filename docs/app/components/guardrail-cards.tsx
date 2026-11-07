import { MonoCard, MonoCards, MonoWide } from "ferramenta-family/monografie";

import specimens from "../data/home-specimens.json";
import "./guardrail-cards.css";

const { guardrails } = specimens;

/** The real backtracking warning, strict refusal and time limit of the published crate. */
export function GuardrailCards() {
  return (
    <MonoWide>
      <MonoCards className="fr-guard">
        <MonoCard
          title="A risky pattern, flagged"
          api={`Regex::new(r"${guardrails.pattern}").backtracking_warnings()`}
        >
          <pre>{guardrails.warnings.join("\n")}</pre>
        </MonoCard>
        <MonoCard title="Refused when users supply it" api="reject_backtracking_risks(true)">
          <pre>
            {`Regex::builder(r"${guardrails.pattern}")\n    .reject_backtracking_risks(true)\n    .build()`}
          </pre>
          <pre className="fr-guard-error">Err: {guardrails.strictError}</pre>
        </MonoCard>
        <MonoCard
          title="A hostile text, stopped"
          api={`SearchOptions::new().timeout(${String(guardrails.timeoutMs)} ms)`}
        >
          <pre>
            {`Regex::new(r"${guardrails.timeoutPattern}")?\n    .find_with(&"a".repeat(${String(guardrails.timeoutRepeat)}), options)`}
          </pre>
          <pre className="fr-guard-error">
            Err({guardrails.timeoutError}) after {guardrails.timeoutMs} ms
          </pre>
        </MonoCard>
      </MonoCards>
    </MonoWide>
  );
}
