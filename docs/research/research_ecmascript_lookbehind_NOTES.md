# Issue #44 ECMAScript Look-Behind Research Spike

## Question

Can Ferroni's existing compiler and VM execute captures and nested look-ahead
inside negative look-behind with Node-compatible match positions and capture
state when the behavior is enabled only through an experimental syntax?

## Prototype shape

The spike adds private Ferroni-only syntax behavior bits and a hidden prototype
syntax. The default Oniguruma syntax is unchanged. A throwaway example runs the
same patterns and subjects through Ferroni and Node, then compares compilation,
first-match byte offsets, numbered captures, and selected named captures.

The spike is archived here and is not built by cargo. To run the differential,
copy `research_ecmascript_lookbehind.rs` into `examples/` and use:

```shell
cargo run --example research_ecmascript_lookbehind
```

Pass `--verbose` after `--` to print every matching case.

## Results

The first focused corpus matched Node in 18 of 21 cases after only enabling the
two previously rejected AST constructs. The three differences were all
backreferences to a capture that did not participate. Adding the ECMAScript
rule that an unmatched backreference matches the empty string brought the
focused corpus to 21 of 21.

The expanded corpus contains 481 comparisons across captures, capture rollback,
group numbering, named groups, later backreferences, fixed and variable-length
look-behind, positive and negative nested look-ahead, alternation, zero-width
assertions, Unicode subjects, and the original cspell-derived forms.

- 469 of 481 comparisons match Node exactly.
- All 435 comparisons outside the two unsupported pattern families match.
- All original issue examples and their direct edge cases match.
- The twelve differences are confined to two deeper ECMAScript behaviors.

### Unsupported behavior 1: nested look-ahead beyond a variable boundary

A nested look-ahead after an unbounded prefix, such as a quantified prefix,
cannot currently inspect text beyond the saved look-behind right boundary.
Ferroni uses that boundary to require the variable-length look-behind body to
end at the assertion position. Fixed-length prefixes, including the reported
apostrophe patterns, do not take this path and match Node.

Supporting this generally requires separate bounds for the consuming
look-behind body and for zero-width assertions nested inside it. Merely
preserving the original quantifier did not change the result.

### Unsupported behavior 2: backreferences inside look-behind

ECMAScript evaluates look-behind atoms in reverse matching order. A
backreference that appears after its capture in pattern text can therefore be
evaluated before that capture participates and matches the empty string.
Ferroni follows the Oniguruma execution model: it steps back to a candidate
start and executes the look-behind body forward. Internal backreferences can
therefore observe different capture state.

Matching JavaScript generally would require an AST transformation or a
backward-executing VM path. The simple validator extension cannot provide it.

## Recommendation

Do not call the current prototype an ECMAScript syntax mode. A broad claim would
be misleading because JavaScript's backward capture and internal-backreference
semantics are not represented by Ferroni's current execution model.

A production implementation is viable as a narrowly named opt-in extension if
it does all of the following:

- preserves the default Oniguruma validator and runtime behavior;
- permits captures in negative look-behind;
- permits nested look-ahead only where its required visibility is representable;
- rejects internal look-behind backreferences and variable-boundary nested
  assertions rather than silently producing different results;
- enables unmatched-backreference-as-empty behavior only for the opt-in mode;
- keeps a Node differential corpus as the semantic contract.

If the goal becomes full ECMAScript look-behind compatibility, treat that as a
larger compiler/VM project rather than an extension of this spike.

## Prototype lifecycle

This code is intentionally marked as a prototype. After the maintainer chooses
between the narrow extension and a broader ECMAScript project, either replace
the hidden syntax with a reviewed API and convert the corpus to tests, or remove
the implementation and retain only this research conclusion in the issue.
