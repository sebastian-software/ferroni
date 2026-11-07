import { useState } from "ardo/react";
import { MonoCaption, MonoFacing, MonoSheet, MonoWide } from "ferramenta-family/monografie";

import { dateText, dateTokens, type PatternToken, specimens } from "./home-specimen-data";
import "./capture-specimen.css";

const { date } = specimens;

type Highlight = { active?: string; setActive: (group?: string) => void };

const lit = (active: string | undefined, group?: string) =>
  group !== undefined && group === active ? "fr-on" : undefined;
const classes = (...names: Array<string | undefined>) =>
  names.filter(Boolean).join(" ") || undefined;

function Token({ token, active, setActive }: { token: PatternToken } & Highlight) {
  const { group } = token;
  const show = () => {
    setActive(group);
  };
  const hide = () => {
    setActive(undefined);
  };
  const interactive = group !== undefined;
  return (
    <span
      className={classes("fr-token", interactive ? undefined : "fr-literal", lit(active, group))}
      tabIndex={interactive ? 0 : undefined}
      onMouseEnter={interactive ? show : undefined}
      onFocus={interactive ? show : undefined}
      onMouseLeave={hide}
      onBlur={hide}
    >
      <code>{token.text}</code>
      <small>{token.label}</small>
    </span>
  );
}

function Found({ active }: { active?: string }) {
  return (
    <MonoSheet label="What Ferroni found" meta="captures(), replace_all()">
      <p className="fr-sub">The text</p>
      <p className="fr-hay">
        {dateText.map((part) =>
          part.group === undefined ? (
            part.text
          ) : (
            <span
              key={part.start}
              className={classes(
                part.look === true ? "fr-look" : "fr-cap",
                lit(active, part.group),
              )}
            >
              {part.text}
            </span>
          ),
        )}
      </p>
      <p className="fr-sub">Captures by name</p>
      <ul className="fr-out">
        {date.captures.map((capture) => (
          <li key={capture.name} className={lit(active, capture.name)}>
            <span>
              {capture.name} {capture.value}
            </span>
            <span>
              bytes {capture.start}–{capture.end}
            </span>
          </li>
        ))}
      </ul>
      <div className="fr-replaced">
        <p className="fr-sub">replace_all(text, {JSON.stringify(date.replacement)})</p>
        <p className="fr-hay">{date.replaced}</p>
      </div>
    </MonoSheet>
  );
}

/**
 * The date pattern beside what Ferroni found: pointing at a group in the
 * pattern lights the text it captured and its entry in the captures list.
 */
export function CaptureSpecimen() {
  const [active, setActive] = useState<string>();
  return (
    <MonoWide>
      <MonoFacing>
        <MonoSheet label="The pattern" meta="Regex::new()">
          <div className="fr-pattern">
            {dateTokens.map((token) => (
              <Token key={token.start} token={token} active={active} setActive={setActive} />
            ))}
          </div>
        </MonoSheet>
        <Found active={active} />
      </MonoFacing>
      <MonoCaption>
        Real output of ferroni {specimens.ferroni}. Point at a group in the pattern to see what it
        found. The lookbehind requires the label without including it in the match.
      </MonoCaption>
    </MonoWide>
  );
}
