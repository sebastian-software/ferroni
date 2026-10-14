import type { ReactNode } from "react";

import "./benchmark-factor.css";

export function BenchmarkFactor({
  value,
  direction,
  children,
}: {
  value: number;
  direction?: "equal" | "faster" | "inconclusive" | "slower";
  children?: ReactNode;
}) {
  return (
    <span
      className="benchmark-factor"
      data-speed={direction ?? (value > 1 ? "faster" : value < 1 ? "slower" : "equal")}
    >
      <strong>{value.toFixed(3)}×</strong>
      {children}
    </span>
  );
}
