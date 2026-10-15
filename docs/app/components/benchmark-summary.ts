import data from "../data/production-summary.json";

export { data as summary };

export type Profile = "linux-x86-64" | "macos-arm64";
export type Lane = "compile" | "first_use" | "steady";
type Comparison = (typeof data.platforms)[number]["comparisons"][number];

/** One engine pair of the verified summary: Ferroni's throughput relative to `engine`. */
export function comparison(profile: Profile, engine: string, lane: Lane = "steady"): Comparison {
  const platform = data.platforms.find((entry) => entry.profile === profile);
  const pair = platform?.comparisons.find(
    (entry) => entry.engine === engine && entry.lane === lane,
  );
  if (pair === undefined) throw new Error("Verified benchmark summary is incomplete.");
  return pair;
}

/** A factor as the page states it: "3.3×". */
export function factor(value: number, digits = 1) {
  return `${value.toFixed(digits)}×`;
}

/** The date of the measurement run, as the page states it. */
export function runDate() {
  return new Date(data.run.created_at).toLocaleDateString("en-US", {
    month: "long",
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  });
}
