# API Parity Plan

Remaining gaps between the C Oniguruma API and the Rust port.
Encodings (27 of 29 missing) are **intentionally scoped to ASCII + UTF-8 only**.

## Status Legend

- [ ] Not started
- [x] Done

---

## 1. Trivial (1-liner or simple accessor, < 5 min each)

- [ ] `onig_get_syntax(reg)` — return `reg.syntax` pointer
- [ ] `onig_get_default_case_fold_flag()` — return global static
- [ ] `onig_set_default_case_fold_flag(flag)` — set global static
- [ ] `onig_get_contents_end_by_callout_args(args)` — return `args.contents_end`

## 2. Easy (small functions, < 30 min each)

- [ ] `onig_set_warn_func(func)` — store global fn pointer
- [ ] `onig_set_verb_warn_func(func)` — store global fn pointer
- [ ] `onig_get_callback_each_match()` — return global callback
- [ ] `onig_set_callback_each_match(func)` — store global callback
- [ ] `onig_copy_encoding(to, from)` — struct copy

## 3. Medium (callout data accessor variants, < 1h total)

These are thin wrappers around the existing `onig_get_callout_data` / `onig_set_callout_data`:

- [ ] `onig_get_callout_data_by_callout_args(args, num, slot)` — extract regex+stk from args, delegate
- [ ] `onig_set_callout_data_by_callout_args(args, num, slot, val)` — same pattern
- [ ] `onig_get_callout_data_by_callout_args_self(args, slot)` — uses args.num as num
- [ ] `onig_set_callout_data_by_callout_args_self(args, slot, val)` — same
- [ ] `onig_get_callout_data_dont_clear_old(reg, mp, num, slot)` — like get but skip clear
- [ ] `onig_get_callout_data_by_callout_args_self_dont_clear_old(args, slot)` — combo

### Callout data by tag (require tag→num lookup):

- [ ] `onig_get_callout_data_by_tag(reg, mp, tag, tag_end, slot)` — tag→num→get
- [ ] `onig_set_callout_data_by_tag(reg, mp, tag, tag_end, slot, val)` — tag→num→set
- [ ] `onig_get_callout_data_by_tag_dont_clear_old(reg, mp, tag, tag_end, slot)` — same + dont_clear

## 4. Medium (callout introspection, < 1h total)

- [ ] `onig_get_callout_name_by_name_id(id)` — lookup in global callout name table
- [ ] `onig_get_capture_range_in_callout(args, mem, beg, end)` — read region from match args
- [ ] `onig_get_used_stack_size_in_callout(args)` — return current stack pointer offset

## 5. Larger (named callout registration, ~2-3h)

- [ ] `onig_set_callout_of_name(enc, type, name, name_end, callout_in, func, end_func, arg_num, arg_types, opt_arg_num, opt_defaults)` — register user-defined named callout. Requires extending the global callout name table to accept user registrations (currently only built-in callouts are registered during `onig_init`).

## 6. Larger (builtin callout public API, ~2h)

The builtins (`FAIL`, `MISMATCH`, `ERROR`, `COUNT`, `MAX`, `CMP`) are already
implemented internally in `regexec.rs` as hardcoded match arms. Making them
available as public functions requires extracting the logic:

- [ ] `onig_builtin_fail(args, user_data)` — always return ONIG_CALLOUT_FAIL
- [ ] `onig_builtin_mismatch(args, user_data)` — return ONIG_MISMATCH
- [ ] `onig_builtin_error(args, user_data)` — return error code from arg
- [ ] `onig_builtin_skip(args, user_data)` — return ONIG_CALLOUT_FAIL + set SKIP
- [ ] `onig_builtin_count(args, user_data)` — increment/read counter
- [ ] `onig_builtin_total_count(args, user_data)` — sum all counters
- [ ] `onig_builtin_max(args, user_data)` — max-tracking callout
- [ ] `onig_builtin_cmp(args, user_data)` — compare two callout data slots
- [ ] `onig_setup_builtin_monitors_by_ascii_encoded_name(fp)` — register all builtins with a monitor output stream

## 7. Not planned

These are intentionally omitted as they don't apply to idiomatic Rust:

- `onig_new_deluxe` — superseded by `onig_new` + options
- `onig_reg_init` — internal, inlined into `onig_new`
- `onig_new_without_alloc` — Rust handles allocation via Box
- `onig_unicode_define_user_property` — requires mutable global Unicode table
- `onigenc_init`, `onig_initialize_encoding` — no-op with only 2 encodings
- `onigenc_set/get_default_encoding` — always UTF-8
- `onigenc_set_default_caseconv_table` — not applicable
- `onigenc_strlen_null`, `onigenc_str_bytelen_null` — Rust has `.len()`
- `onigenc_get_right_adjust_char_head_with_prev` — niche, unused in practice

---

## Summary

| Priority | Count | Effort |
|---|---|---|
| 1. Trivial | 4 | ~20 min |
| 2. Easy | 5 | ~2h |
| 3. Medium (callout wrappers) | 9 | ~1h |
| 4. Medium (callout introspection) | 3 | ~1h |
| 5. Named callout registration | 1 | ~2-3h |
| 6. Builtin callout public API | 9 | ~2h |
| 7. Not planned | 9 | — |
| **Total actionable** | **31** | **~8-9h** |

After completing items 1–6, API parity would be **96/103 (93%)**, with the
remaining 7 being intentional Rust-idiom omissions (Drop, allocation, encoding
utilities).
