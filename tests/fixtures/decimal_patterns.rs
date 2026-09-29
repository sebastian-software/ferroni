// Unmodified number expressions from the V and PureScript TextMate grammars.
// Snapshot provenance is recorded in docs/experiments/197-decimal-loop-atomic.md.
pub const V_FLOAT: &str = r"([0-9]+(_?))+(\.)([0-9]+)";
pub const V_EXPONENT: &str = r"([0-9]+(_?))+(\.)([0-9]+[Ee][-+]?[0-9]+)";
// Requires internal digit give-back; experiment 4 makes the complete tail atomic.
pub const PURESCRIPT_INTEGER: &str = r"\b(([0-9]+_?)*[0-9]+|0([Xx]\h+|[Oo][0-7]+))\b";
