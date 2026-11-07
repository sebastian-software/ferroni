import { type RefObject, useEffect, useId, useRef, useState } from "ardo/react";
import { MonoSegmented, MonoWide } from "ferramenta-family/monografie";

import { specimens } from "./home-specimen-data";
import "./scanner-walkthrough.css";

const { scanner } = specimens;
type Line = "ascii" | "wide";
type Step = (typeof scanner.ascii.steps)[number];

const encoder = new TextEncoder();
const reduced = () =>
  typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Every character of a line with the byte it starts at. */
function characters(input: string) {
  let byte = 0;
  return Array.from(input, (char) => {
    const start = byte;
    byte += encoder.encode(char).length;
    return { char, start };
  });
}

/** Steps through the searches: once on request, with pauses unless motion is reduced. */
function usePlayback(line: Line) {
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const [step, setStep] = useState(-1);
  const [runs, setRuns] = useState(0);
  const stop = () => {
    clearTimeout(timerRef.current);
  };
  useEffect(() => {
    if (runs === 0) return;
    const count = scanner[line].steps.length;
    let next = reduced() ? count - 1 : -1;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const tick = () => {
      setStep(next);
      next += 1;
      timer = next < count ? setTimeout(tick, next === 0 ? 700 : 1500) : undefined;
      timerRef.current = timer;
    };
    timer = setTimeout(tick, 0);
    timerRef.current = timer;
    return () => {
      clearTimeout(timer);
    };
  }, [runs, line]);
  const replay = () => {
    setRuns((count) => count + 1);
  };
  const show = (index: number) => {
    stop();
    setStep(index);
  };
  return { step, replay, show, stop };
}

/** Plays once when the element is half in view. */
function useFirstView(elementRef: RefObject<HTMLElement | null>, onView: () => void) {
  const callbackRef = useRef(onView);
  callbackRef.current = onView;
  useEffect(() => {
    const target = elementRef.current;
    if (target === null || typeof IntersectionObserver !== "function") return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries[0].isIntersecting) return;
        callbackRef.current();
        observer.disconnect();
      },
      { threshold: 0.5 },
    );
    observer.observe(target);
    return () => {
      observer.disconnect();
    };
  }, [elementRef]);
}

/** Moves the caret to the character where a search starts, again when the line resizes. */
function useCaret(
  boxRef: RefObject<HTMLElement | null>,
  caretRef: RefObject<HTMLElement | null>,
  byte: number,
) {
  useEffect(() => {
    const box = boxRef.current;
    if (box === null) return;
    const place = () => {
      const target = box.querySelector<HTMLElement>(`[data-byte="${String(byte)}"]`);
      caretRef.current?.style.setProperty(
        "left",
        `${String((target?.offsetLeft ?? box.scrollWidth) - 1)}px`,
      );
    };
    place();
    if (typeof ResizeObserver !== "function") return;
    const observer = new ResizeObserver(place);
    observer.observe(box);
    return () => {
      observer.disconnect();
    };
  }, [boxRef, caretRef, byte]);
}

function ScanLine({ line, step }: { line: Line; step: number }) {
  const { input, steps } = scanner[line];
  const boxRef = useRef<HTMLDivElement>(null);
  const caretRef = useRef<HTMLElement>(null);
  useCaret(boxRef, caretRef, step >= 0 ? steps[step].from : 0);
  const state = (byte: number) => {
    const index = steps.findIndex(
      (entry, k) => k <= step && byte >= entry.start && byte < entry.end,
    );
    if (index === -1) return "";
    return index === step ? "fr-now" : "fr-past";
  };
  return (
    <div className="fr-scan-line" ref={boxRef} aria-hidden="true">
      <i className="fr-caret" ref={caretRef} data-label="search starts here" />
      {characters(input).map(({ char, start }) => (
        <span key={start} data-byte={start} className={state(start) || undefined}>
          {char}
        </span>
      ))}
    </div>
  );
}

function Patterns({ winner }: { winner?: number }) {
  return (
    <div>
      <p className="fr-scan-sub">Patterns in the scanner</p>
      <ol className="fr-scan-patterns">
        {scanner.patterns.map((pattern, index) => (
          <li key={pattern} className={winner === index ? "fr-win" : undefined}>
            <b>pattern {index}</b>
            <code>{pattern}</code>
          </li>
        ))}
      </ol>
    </div>
  );
}

function StepRow({ entry, now, onShow }: { entry: Step; now: boolean; onShow: () => void }) {
  return (
    <tr tabIndex={0} className={now ? "fr-now" : undefined} onClick={onShow} onFocus={onShow}>
      <td className="fr-num">{entry.from}</td>
      <td>pattern {entry.index}</td>
      <td>
        <code>{entry.matched}</code>
      </td>
      <td className="fr-num">
        {entry.start}–{entry.end}
      </td>
      <td className="fr-num">
        {entry.utf16Start}–{entry.utf16End}
      </td>
    </tr>
  );
}

function Steps({ line, step, show }: { line: Line; step: number; show: (i: number) => void }) {
  return (
    <table className="fr-scan-steps">
      <thead>
        <tr>
          <th scope="col">From byte</th>
          <th scope="col">Winning pattern</th>
          <th scope="col">Match</th>
          <th scope="col" className="fr-num">
            Bytes
          </th>
          <th scope="col" className="fr-num">
            UTF-16
          </th>
        </tr>
      </thead>
      <tbody>
        {scanner[line].steps.map((entry, index) => (
          <StepRow
            key={`${line}-${String(entry.from)}`}
            entry={entry}
            now={index === step}
            onShow={() => {
              show(index);
            }}
          />
        ))}
      </tbody>
    </table>
  );
}

const LINES = [
  { value: "ascii", label: "ASCII line" },
  { value: "wide", label: "Line with ß" },
];

/**
 * One line, three searches for the next match: a caret walks to where each
 * search starts, the winning pattern lights up and the steps table shows byte
 * and UTF-16 positions. It plays once when it comes into view, and again on
 * request or when the line changes.
 */
export function ScannerWalkthrough() {
  const titleId = useId();
  const figureRef = useRef<HTMLElement>(null);
  const [line, setLine] = useState<Line>("ascii");
  const { step, replay, show, stop } = usePlayback(line);
  useFirstView(figureRef, replay);
  const { steps } = scanner[line];
  const differs = steps.some((entry) => entry.start !== entry.utf16Start);
  return (
    <MonoWide>
      <figure className="fr-scan" ref={figureRef} aria-labelledby={titleId}>
        <div className="fr-scan-head">
          <p id={titleId}>One line, three searches for the next match</p>
          <MonoSegmented
            label="Line"
            value={line}
            options={LINES}
            onChange={(value) => {
              stop();
              setLine(value === "wide" ? "wide" : "ascii");
              replay();
            }}
          />
        </div>
        <ScanLine line={line} step={step} />
        <div className="fr-scan-cols">
          <Patterns winner={step >= 0 ? steps[step].index : undefined} />
          <Steps line={line} step={step} show={show} />
        </div>
        <div className="fr-scan-foot">
          <span>
            {differs
              ? "ß takes two bytes but one UTF-16 unit, so the positions part after it."
              : "On ASCII text, byte and UTF-16 positions agree."}
          </span>
          <button type="button" className="fr-replay" onClick={replay}>
            Run the searches again
          </button>
        </div>
      </figure>
    </MonoWide>
  );
}
