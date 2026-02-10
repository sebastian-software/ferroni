// regcomp.rs - Port of regcomp.c
// Compiler: converts AST (Node trees) into bytecode (Operation arrays).
//
// This is a 1:1 port of oniguruma's regcomp.c (~8,500 LOC).
// Structure mirrors the C original: operation management → string compilation →
// cclass compilation → quantifier compilation → bag compilation → anchor compilation →
// tree compilation → entry point.

#![allow(non_upper_case_globals)]
#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(unused_mut)]

use crate::oniguruma::*;
use crate::regenc::*;
use crate::regint::*;
use crate::regparse_types::*;

// ============================================================================
// Constants (matching C OPSIZE_* and SIZE_INC)
// ============================================================================

// All operations are 1 slot in the ops array (matching C where every OPSIZE_* = 1)
const SIZE_INC: i32 = 1;

const OPSIZE_ANYCHAR_STAR: i32 = 1;
const OPSIZE_ANYCHAR_STAR_PEEK_NEXT: i32 = 1;
const OPSIZE_JUMP: i32 = 1;
const OPSIZE_PUSH: i32 = 1;
const OPSIZE_PUSH_SUPER: i32 = 1;
const OPSIZE_POP: i32 = 1;
const OPSIZE_POP_TO_MARK: i32 = 1;
const OPSIZE_PUSH_OR_JUMP_EXACT1: i32 = 1;
const OPSIZE_PUSH_IF_PEEK_NEXT: i32 = 1;
const OPSIZE_REPEAT: i32 = 1;
const OPSIZE_REPEAT_INC: i32 = 1;
const OPSIZE_REPEAT_INC_NG: i32 = 1;
const OPSIZE_WORD_BOUNDARY: i32 = 1;
const OPSIZE_BACKREF: i32 = 1;
const OPSIZE_FAIL: i32 = 1;
const OPSIZE_MEM_START: i32 = 1;
const OPSIZE_MEM_START_PUSH: i32 = 1;
const OPSIZE_MEM_END_PUSH: i32 = 1;
const OPSIZE_MEM_END_PUSH_REC: i32 = 1;
const OPSIZE_MEM_END: i32 = 1;
const OPSIZE_MEM_END_REC: i32 = 1;
const OPSIZE_EMPTY_CHECK_START: i32 = 1;
const OPSIZE_EMPTY_CHECK_END: i32 = 1;
const OPSIZE_CHECK_POSITION: i32 = 1;
const OPSIZE_CALL: i32 = 1;
const OPSIZE_RETURN: i32 = 1;
const OPSIZE_MOVE: i32 = 1;
const OPSIZE_STEP_BACK_START: i32 = 1;
const OPSIZE_STEP_BACK_NEXT: i32 = 1;
const OPSIZE_CUT_TO_MARK: i32 = 1;
const OPSIZE_MARK: i32 = 1;
const OPSIZE_SAVE_VAL: i32 = 1;
const OPSIZE_UPDATE_VAR: i32 = 1;

// ============================================================================
// Operation management
// ============================================================================

/// Add an operation with the given opcode and payload to the regex's ops array.
/// Returns the index of the newly added operation.
fn add_op(reg: &mut RegexType, opcode: OpCode, payload: OperationPayload) -> i32 {
    let idx = reg.ops.len();
    reg.ops.push(Operation { opcode, payload });
    idx as i32
}

/// Get the index of the current (last) operation.
fn ops_curr_offset(reg: &RegexType) -> i32 {
    (reg.ops.len() as i32) - 1
}

// ============================================================================
// Utility functions
// ============================================================================

/// Safe multiplication comparison: a * b > limit
fn len_multiply_cmp(a: OnigLen, b: i32, limit: OnigLen) -> bool {
    if a == 0 || b == 0 {
        return false;
    }
    if a > limit / (b as OnigLen) {
        return true;
    }
    a * (b as OnigLen) > limit
}

/// Add two lengths safely, capping at INFINITE_LEN.
fn distance_add(d1: OnigLen, d2: OnigLen) -> OnigLen {
    if d1 == INFINITE_LEN || d2 == INFINITE_LEN {
        INFINITE_LEN
    } else if d1 <= INFINITE_LEN - d2 {
        d1 + d2
    } else {
        INFINITE_LEN
    }
}

/// Multiply a length by a count safely, capping at INFINITE_LEN.
fn distance_multiply(d: OnigLen, m: i32) -> OnigLen {
    if m == 0 {
        return 0;
    }
    if d >= INFINITE_LEN / (m as OnigLen) {
        return INFINITE_LEN;
    }
    d * (m as OnigLen)
}

/// Check if a bitset is empty (all zeros).
fn bitset_is_empty(bs: &BitSet) -> bool {
    for i in 0..BITSET_REAL_SIZE {
        if bs[i] != 0 {
            return false;
        }
    }
    true
}

/// Check if a node is a "strict real" node (actually matches characters).
fn is_strict_real_node(node: &Node) -> bool {
    match &node.inner {
        NodeInner::String(_) | NodeInner::CClass(_) | NodeInner::CType(_) => true,
        _ => false,
    }
}

// ============================================================================
// String compilation
// ============================================================================

/// Select the opcode for a string of given byte length and encoding char width.
fn select_str_opcode(mb_len: i32, str_len: i32) -> OpCode {
    if mb_len == 1 {
        match str_len {
            1 => OpCode::Str1,
            2 => OpCode::Str2,
            3 => OpCode::Str3,
            4 => OpCode::Str4,
            5 => OpCode::Str5,
            _ => OpCode::StrN,
        }
    } else if mb_len == 2 {
        match str_len {
            1 => OpCode::StrMb2n1,
            2 => OpCode::StrMb2n2,
            3 => OpCode::StrMb2n3,
            _ => OpCode::StrMb2n,
        }
    } else if mb_len == 3 {
        OpCode::StrMb3n
    } else {
        OpCode::StrMbn
    }
}

/// Calculate bytecode length for adding a compiled string segment.
fn add_compile_string_length(_s: &[u8], mb_len: i32, str_len: i32) -> i32 {
    SIZE_INC
}

/// Add a compiled string segment to the bytecode.
fn add_compile_string(
    reg: &mut RegexType,
    s: &[u8],
    mb_len: i32,
    str_len: i32,
) -> i32 {
    let op = select_str_opcode(mb_len, str_len);
    let byte_len = mb_len * str_len;

    let payload = if mb_len == 1 && byte_len <= 16 {
        // Single-byte encoding: use compact Exact payload
        let mut buf = [0u8; 16];
        buf[..byte_len as usize].copy_from_slice(&s[..byte_len as usize]);
        OperationPayload::Exact { s: buf }
    } else if mb_len == 1 {
        OperationPayload::ExactN {
            s: s[..byte_len as usize].to_vec(),
            n: str_len,
        }
    } else {
        // Multi-byte encoding: always use ExactLenN with byte count
        OperationPayload::ExactLenN {
            s: s[..byte_len as usize].to_vec(),
            n: byte_len,  // total byte count
            len: mb_len,  // bytes per character
        }
    };

    add_op(reg, op, payload);
    0
}

/// Calculate bytecode length for a string node.
fn compile_length_string_node(node: &Node, reg: &RegexType) -> i32 {
    let sn = node.as_str().unwrap();
    let enc = reg.enc;

    if sn.s.is_empty() {
        return 0;
    }

    let mut len = 0i32;
    let mut pos = 0usize;
    let slen = sn.s.len();

    let ambig = node.has_status(ND_ST_IGNORECASE);

    while pos < slen {
        let first_len = enc.mbc_enc_len(&sn.s[pos..]);
        let mut run = 1;
        let next = pos + first_len;
        let mut p = next;

        // Group consecutive characters with the same mb_len
        while p < slen {
            let enc_len = enc.mbc_enc_len(&sn.s[p..]);
            if enc_len != first_len {
                break;
            }
            run += 1;
            p += enc_len;
        }

        len += add_compile_string_length(&sn.s[pos..], first_len as i32, run);
        pos = p;
    }

    len
}

/// Calculate bytecode length for a "crude" string node.
fn compile_length_string_crude_node(node: &Node, reg: &RegexType) -> i32 {
    let sn = node.as_str().unwrap();
    if sn.s.is_empty() {
        return 0;
    }
    SIZE_INC
}

/// Compile a string node to bytecode.
fn compile_string_node(node: &Node, reg: &mut RegexType) -> i32 {
    let sn = node.as_str().unwrap();
    let enc = reg.enc;

    if sn.s.is_empty() {
        return 0;
    }

    let ambig = node.has_status(ND_ST_IGNORECASE);
    let mut pos = 0usize;
    let slen = sn.s.len();

    while pos < slen {
        let first_len = enc.mbc_enc_len(&sn.s[pos..]);
        let mut run = 1;
        let next = pos + first_len;
        let mut p = next;

        while p < slen {
            let enc_len = enc.mbc_enc_len(&sn.s[p..]);
            if enc_len != first_len {
                break;
            }
            run += 1;
            p += enc_len;
        }

        let r = add_compile_string(reg, &sn.s[pos..], first_len as i32, run);
        if r != 0 {
            return r;
        }
        pos = p;
    }

    0
}

/// Compile a crude string node to bytecode.
fn compile_string_crude_node(node: &Node, reg: &mut RegexType) -> i32 {
    let sn = node.as_str().unwrap();
    if sn.s.is_empty() {
        return 0;
    }

    let byte_len = sn.s.len();
    let payload = if byte_len <= 16 {
        let mut buf = [0u8; 16];
        buf[..byte_len].copy_from_slice(&sn.s[..byte_len]);
        OperationPayload::Exact { s: buf }
    } else {
        OperationPayload::ExactN {
            s: sn.s.clone(),
            n: byte_len as i32,
        }
    };

    add_op(reg, select_str_opcode(1, byte_len as i32), payload);
    0
}

// ============================================================================
// Character class compilation
// ============================================================================

/// Calculate bytecode length for a character class node.
fn compile_length_cclass_node(cc: &CClassNode, reg: &RegexType) -> i32 {
    SIZE_INC
}

/// Compile a character class node to bytecode.
fn compile_cclass_node(cc: &CClassNode, reg: &mut RegexType) -> i32 {
    let has_mb = cc.mbuf.is_some();
    let has_sb = !bitset_is_empty(&cc.bs);

    if has_mb && has_sb {
        // Mixed single-byte and multi-byte
        let opcode = if cc.is_not() {
            OpCode::CClassMixNot
        } else {
            OpCode::CClassMix
        };
        let mb_data = cc.mbuf.as_ref().map(|b| b.data.clone()).unwrap_or_default();
        add_op(reg, opcode, OperationPayload::CClassMix {
            mb: mb_data,
            bsp: Box::new(cc.bs),
        });
    } else if has_mb {
        // Multi-byte only
        let opcode = if cc.is_not() {
            OpCode::CClassMbNot
        } else {
            OpCode::CClassMb
        };
        let mb_data = cc.mbuf.as_ref().map(|b| b.data.clone()).unwrap_or_default();
        add_op(reg, opcode, OperationPayload::CClassMb { mb: mb_data });
    } else {
        // Single-byte only
        let opcode = if cc.is_not() {
            OpCode::CClassNot
        } else {
            OpCode::CClass
        };
        add_op(reg, opcode, OperationPayload::CClass {
            bsp: Box::new(cc.bs),
        });
    }

    0
}

// ============================================================================
// Repeat range management
// ============================================================================

/// Register a repeat range entry. Returns the repeat ID.
fn entry_repeat_range(
    reg: &mut RegexType,
    lower: i32,
    upper: i32,
) -> Result<i32, i32> {
    let id = reg.num_repeat;
    reg.num_repeat += 1;

    reg.repeat_range.push(RepeatRange {
        lower,
        upper,
        u_offset: 0,
    });

    Ok(id)
}

// ============================================================================
// Quantifier compilation
// ============================================================================

/// Compile a quantifier body wrapped with empty-match check if needed.
fn compile_quant_body_with_empty_check(
    node: &Node,
    reg: &mut RegexType,
    env: &ParseEnv,
    emptiness: BodyEmptyType,
) -> i32 {
    let is_empty = emptiness != BodyEmptyType::NotEmpty;

    if is_empty {
        let mem = reg.num_empty_check;
        reg.num_empty_check += 1;
        add_op(reg, OpCode::EmptyCheckStart, OperationPayload::EmptyCheckStart { mem });
    }

    let r = compile_tree(node, reg, env);
    if r != 0 {
        return r;
    }

    if is_empty {
        let mem = reg.num_empty_check - 1;
        let empty_status_mem = 0; // TODO: actual status tracking
        let opcode = match emptiness {
            BodyEmptyType::MayBeEmptyMem => OpCode::EmptyCheckEndMemst,
            BodyEmptyType::MayBeEmptyRec => OpCode::EmptyCheckEndMemstPush,
            _ => OpCode::EmptyCheckEnd,
        };
        add_op(reg, opcode, OperationPayload::EmptyCheckEnd { mem, empty_status_mem });
    }

    0
}

/// Compile a node N times (for expanding small-count quantifiers).
fn compile_tree_n_times(
    node: &Node,
    n: i32,
    reg: &mut RegexType,
    env: &ParseEnv,
) -> i32 {
    for _ in 0..n {
        let r = compile_tree(node, reg, env);
        if r != 0 {
            return r;
        }
    }
    0
}

/// Check if a quantifier node represents .* (anychar infinite greedy).
fn is_anychar_infinite_greedy(qn: &QuantNode) -> bool {
    if qn.greedy && is_infinite_repeat(qn.upper) && qn.lower <= 1 {
        if let Some(body) = &qn.body {
            return matches!(body.inner, NodeInner::CType(ref ct) if ct.ctype == ONIGENC_CTYPE_WORD as i32 || true);
        }
    }
    false
}

/// Calculate bytecode length for a quantifier node.
fn compile_length_quantifier_node(qn: &QuantNode, reg: &RegexType, env: &ParseEnv) -> i32 {
    let body = qn.body.as_ref().unwrap();

    if qn.upper == 0 {
        // {0} matches nothing
        if is_anychar_infinite_greedy(qn) {
            return SIZE_INC;
        }
        return 0;
    }

    let is_empty = qn.emptiness != BodyEmptyType::NotEmpty;
    let body_len = compile_length_tree(body, reg, env);
    if body_len < 0 {
        return body_len;
    }

    let empty_len = if is_empty { OPSIZE_EMPTY_CHECK_START + OPSIZE_EMPTY_CHECK_END } else { 0 };
    let mod_tlen = body_len + empty_len;

    if is_infinite_repeat(qn.upper) {
        if qn.lower <= 1 {
            // *, +, *?, +?
            OPSIZE_PUSH + mod_tlen + OPSIZE_JUMP
        } else {
            // {n,} or {n,}?
            let n_body_len = compile_length_tree_n_times(body, qn.lower - 1, reg, env);
            n_body_len + OPSIZE_PUSH + mod_tlen + OPSIZE_JUMP
        }
    } else if qn.upper == 0 {
        0
    } else if !is_infinite_repeat(qn.upper) && qn.lower == qn.upper {
        // {n,n} exact repeat
        if qn.lower == 1 {
            body_len
        } else {
            // Use REPEAT opcodes for larger exact counts
            let id_len = OPSIZE_REPEAT + mod_tlen + OPSIZE_REPEAT_INC;
            id_len
        }
    } else {
        // {n,m} range repeat
        OPSIZE_REPEAT + mod_tlen + OPSIZE_REPEAT_INC
    }
}

/// Calculate compile length for N repetitions of a node.
fn compile_length_tree_n_times(node: &Node, n: i32, reg: &RegexType, env: &ParseEnv) -> i32 {
    let len = compile_length_tree(node, reg, env);
    if len < 0 {
        return len;
    }
    len * n
}

/// Compile a quantifier node to bytecode.
fn compile_quantifier_node(qn: &QuantNode, reg: &mut RegexType, env: &ParseEnv) -> i32 {
    let body = qn.body.as_ref().unwrap();

    if qn.upper == 0 {
        return 0;
    }

    let is_empty = qn.emptiness != BodyEmptyType::NotEmpty;
    let body_len = compile_length_tree(body, reg, env);
    if body_len < 0 {
        return body_len;
    }

    let empty_len = if is_empty { OPSIZE_EMPTY_CHECK_START + OPSIZE_EMPTY_CHECK_END } else { 0 };
    let mod_tlen = body_len + empty_len;

    if is_infinite_repeat(qn.upper) {
        if qn.lower <= 1 {
            if qn.greedy {
                // a* or a+
                if qn.lower == 1 {
                    // a+ : body first, then loop
                    compile_tree_n_times(body, 1, reg, env);
                }
                // PUSH → body → JUMP back to PUSH
                // C: COP(reg)->push.addr = SIZE_INC + mod_tlen + OPSIZE_JUMP;
                add_op(reg, OpCode::Push, OperationPayload::Push {
                    addr: SIZE_INC + mod_tlen + OPSIZE_JUMP,
                });
                let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness);
                if r != 0 {
                    return r;
                }
                // C: addr = -(mod_tlen + (int)OPSIZE_PUSH);
                add_op(reg, OpCode::Jump, OperationPayload::Jump {
                    addr: -(mod_tlen + OPSIZE_PUSH),
                });
            } else {
                // a*? or a+?
                if qn.lower == 1 {
                    compile_tree_n_times(body, 1, reg, env);
                }
                // JUMP forward → body → PUSH back
                // C: COP(reg)->jump.addr = mod_tlen + SIZE_INC;
                add_op(reg, OpCode::Jump, OperationPayload::Jump {
                    addr: mod_tlen + SIZE_INC,
                });
                let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness);
                if r != 0 {
                    return r;
                }
                // C: COP(reg)->push.addr = -mod_tlen;
                add_op(reg, OpCode::Push, OperationPayload::Push {
                    addr: -mod_tlen,
                });
            }
        } else {
            // {n,} with n >= 2
            // Compile body n-1 times, then loop
            let r = compile_tree_n_times(body, qn.lower - 1, reg, env);
            if r != 0 {
                return r;
            }

            if qn.greedy {
                add_op(reg, OpCode::Push, OperationPayload::Push {
                    addr: SIZE_INC + mod_tlen + OPSIZE_JUMP,
                });
                let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness);
                if r != 0 {
                    return r;
                }
                // C: addr = -(mod_tlen + (int)OPSIZE_PUSH);
                add_op(reg, OpCode::Jump, OperationPayload::Jump {
                    addr: -(mod_tlen + OPSIZE_PUSH),
                });
            } else {
                // C: COP(reg)->jump.addr = mod_tlen + SIZE_INC;
                add_op(reg, OpCode::Jump, OperationPayload::Jump {
                    addr: mod_tlen + SIZE_INC,
                });
                let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness);
                if r != 0 {
                    return r;
                }
                // C: COP(reg)->push.addr = -mod_tlen;
                add_op(reg, OpCode::Push, OperationPayload::Push {
                    addr: -mod_tlen,
                });
            }
        }
    } else if qn.lower == qn.upper {
        // {n} exact repeat
        if qn.lower == 1 {
            let r = compile_tree(body, reg, env);
            return r;
        }
        // Use REPEAT opcode
        let id = entry_repeat_range(reg, qn.lower, qn.upper);
        if let Err(e) = id {
            return e;
        }
        let id = id.unwrap();

        add_op(reg, OpCode::Repeat, OperationPayload::Repeat {
            id,
            addr: SIZE_INC + mod_tlen + OPSIZE_REPEAT_INC,
        });
        // Patch u_offset to point to the body start (op after REPEAT)
        reg.repeat_range[id as usize].u_offset = reg.ops.len() as i32;
        let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness);
        if r != 0 {
            return r;
        }
        add_op(reg, if qn.greedy { OpCode::RepeatInc } else { OpCode::RepeatIncNg },
               OperationPayload::RepeatInc { id });
    } else {
        // {n,m} range repeat
        let id = entry_repeat_range(reg, qn.lower, qn.upper);
        if let Err(e) = id {
            return e;
        }
        let id = id.unwrap();

        let opcode = if qn.greedy { OpCode::Repeat } else { OpCode::RepeatNg };
        add_op(reg, opcode, OperationPayload::Repeat {
            id,
            addr: SIZE_INC + mod_tlen + OPSIZE_REPEAT_INC,
        });
        // Patch u_offset to point to the body start (op after REPEAT)
        reg.repeat_range[id as usize].u_offset = reg.ops.len() as i32;
        let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness);
        if r != 0 {
            return r;
        }
        add_op(reg, if qn.greedy { OpCode::RepeatInc } else { OpCode::RepeatIncNg },
               OperationPayload::RepeatInc { id });
    }

    0
}

// ============================================================================
// Bag (group) compilation
// ============================================================================

/// Calculate bytecode length for a bag node.
fn compile_length_bag_node(bag: &BagNode, reg: &RegexType, env: &ParseEnv) -> i32 {
    let body = bag.body.as_ref();

    match bag.bag_type {
        BagType::Memory => {
            let body_len = if let Some(b) = body {
                compile_length_tree(b, reg, env)
            } else {
                0
            };
            if body_len < 0 {
                return body_len;
            }
            // MEM_START + body + MEM_END
            OPSIZE_MEM_START + body_len + OPSIZE_MEM_END
        }
        BagType::StopBacktrack => {
            let body_len = if let Some(b) = body {
                compile_length_tree(b, reg, env)
            } else {
                0
            };
            if body_len < 0 {
                return body_len;
            }
            // MARK + body + CUT_TO_MARK
            OPSIZE_MARK + body_len + OPSIZE_CUT_TO_MARK
        }
        BagType::Option => {
            let body_len = if let Some(b) = body {
                compile_length_tree(b, reg, env)
            } else {
                0
            };
            if body_len < 0 {
                return body_len;
            }
            body_len
        }
        BagType::IfElse => {
            // TODO: conditional compilation
            let body_len = if let Some(b) = body {
                compile_length_tree(b, reg, env)
            } else {
                0
            };
            if body_len < 0 {
                return body_len;
            }
            body_len
        }
    }
}

/// Compile a bag memory (capture group) node.
fn compile_bag_memory_node(bag: &BagNode, reg: &mut RegexType, env: &ParseEnv) -> i32 {
    let (regnum, _called_addr, _entry_count, _called_state) = match &bag.bag_data {
        BagData::Memory { regnum, called_addr, entry_count, called_state } => {
            (*regnum, *called_addr, *entry_count, *called_state)
        }
        _ => return ONIGERR_TYPE_BUG as i32,
    };

    // Determine if we need push variants
    let need_push = mem_status_at(reg.push_mem_start, regnum as usize);

    if need_push {
        add_op(reg, OpCode::MemStartPush, OperationPayload::MemoryStart { num: regnum });
    } else {
        add_op(reg, OpCode::MemStart, OperationPayload::MemoryStart { num: regnum });
    }

    if let Some(body) = &bag.body {
        let r = compile_tree(body, reg, env);
        if r != 0 {
            return r;
        }
    }

    let need_push_end = mem_status_at(reg.push_mem_end, regnum as usize);
    if need_push_end {
        add_op(reg, OpCode::MemEndPush, OperationPayload::MemoryEnd { num: regnum });
    } else {
        add_op(reg, OpCode::MemEnd, OperationPayload::MemoryEnd { num: regnum });
    }

    0
}

/// Compile a bag node to bytecode.
fn compile_bag_node(bag: &BagNode, reg: &mut RegexType, env: &ParseEnv) -> i32 {
    match bag.bag_type {
        BagType::Memory => {
            compile_bag_memory_node(bag, reg, env)
        }
        BagType::StopBacktrack => {
            let id = reg.num_call; // use call count as mark ID
            reg.num_call += 1;

            add_op(reg, OpCode::Mark, OperationPayload::Mark {
                id,
                save_pos: true,
            });

            if let Some(body) = &bag.body {
                let r = compile_tree(body, reg, env);
                if r != 0 {
                    return r;
                }
            }

            add_op(reg, OpCode::CutToMark, OperationPayload::CutToMark {
                id,
                restore_pos: false,
            });

            0
        }
        BagType::Option => {
            // Option change: just compile the body with the option set.
            // The option was already applied to the parse env during parsing.
            if let Some(body) = &bag.body {
                return compile_tree(body, reg, env);
            }
            0
        }
        BagType::IfElse => {
            // TODO: conditional compilation
            if let Some(body) = &bag.body {
                return compile_tree(body, reg, env);
            }
            0
        }
    }
}

// ============================================================================
// Anchor compilation
// ============================================================================

/// Calculate bytecode length for an anchor node.
fn compile_length_anchor_node(an: &AnchorNode, reg: &RegexType, env: &ParseEnv) -> i32 {
    let at = an.anchor_type;

    if at == ANCR_PREC_READ {
        // (?=...) positive lookahead: MARK + body + CUT_TO_MARK
        let body_len = if let Some(body) = &an.body {
            compile_length_tree(body, reg, env)
        } else {
            0
        };
        if body_len < 0 {
            return body_len;
        }
        OPSIZE_MARK + body_len + OPSIZE_CUT_TO_MARK
    } else if at == ANCR_PREC_READ_NOT {
        // (?!...) negative lookahead: PUSH + MARK + body + POP_TO_MARK + POP + FAIL
        let body_len = if let Some(body) = &an.body {
            compile_length_tree(body, reg, env)
        } else {
            0
        };
        if body_len < 0 {
            return body_len;
        }
        OPSIZE_PUSH + OPSIZE_MARK + body_len + OPSIZE_POP_TO_MARK + OPSIZE_POP + OPSIZE_FAIL
    } else if at == ANCR_LOOK_BEHIND {
        // (?<=...) positive lookbehind
        let body_len = if let Some(body) = &an.body {
            compile_length_tree(body, reg, env)
        } else {
            0
        };
        if body_len < 0 {
            return body_len;
        }
        OPSIZE_MARK + OPSIZE_STEP_BACK_START + body_len + OPSIZE_CUT_TO_MARK
    } else if at == ANCR_LOOK_BEHIND_NOT {
        // (?<!...) negative lookbehind
        let body_len = if let Some(body) = &an.body {
            compile_length_tree(body, reg, env)
        } else {
            0
        };
        if body_len < 0 {
            return body_len;
        }
        OPSIZE_PUSH + OPSIZE_MARK + OPSIZE_STEP_BACK_START + body_len +
            OPSIZE_POP_TO_MARK + OPSIZE_POP + OPSIZE_FAIL
    } else {
        // Simple anchors: ^, $, \b, \B, \A, \z, etc.
        SIZE_INC
    }
}

/// Compile an anchor node to bytecode.
fn compile_anchor_node(an: &AnchorNode, reg: &mut RegexType, env: &ParseEnv) -> i32 {
    let at = an.anchor_type;

    if at == ANCR_PREC_READ {
        // (?=...) positive lookahead
        let id = reg.num_call;
        reg.num_call += 1;

        add_op(reg, OpCode::Mark, OperationPayload::Mark {
            id,
            save_pos: true,
        });

        if let Some(body) = &an.body {
            let r = compile_tree(body, reg, env);
            if r != 0 {
                return r;
            }
        }

        add_op(reg, OpCode::CutToMark, OperationPayload::CutToMark {
            id,
            restore_pos: true,
        });
        return 0;
    }

    if at == ANCR_PREC_READ_NOT {
        // (?!...) negative lookahead
        let body_len = if let Some(body) = &an.body {
            compile_length_tree(body, reg, env)
        } else {
            0
        };

        let id = reg.num_call;
        reg.num_call += 1;

        // PUSH past the fail section (C: SIZE_INC + MARK + body + POP_TO_MARK + POP + FAIL)
        let push_addr = SIZE_INC + OPSIZE_MARK + body_len + OPSIZE_POP_TO_MARK + OPSIZE_POP + OPSIZE_FAIL;
        add_op(reg, OpCode::Push, OperationPayload::Push { addr: push_addr });

        add_op(reg, OpCode::Mark, OperationPayload::Mark {
            id,
            save_pos: false,
        });

        if let Some(body) = &an.body {
            let r = compile_tree(body, reg, env);
            if r != 0 {
                return r;
            }
        }

        add_op(reg, OpCode::PopToMark, OperationPayload::PopToMark { id });
        add_op(reg, OpCode::Pop, OperationPayload::None);
        add_op(reg, OpCode::Fail, OperationPayload::None);
        return 0;
    }

    if at == ANCR_LOOK_BEHIND {
        // (?<=...) positive lookbehind
        let id = reg.num_call;
        reg.num_call += 1;

        add_op(reg, OpCode::Mark, OperationPayload::Mark {
            id,
            save_pos: true,
        });

        let char_len = an.char_min_len as i32;
        add_op(reg, OpCode::StepBackStart, OperationPayload::StepBackStart {
            initial: char_len,
            remaining: 0,
            addr: 0, // no variable-length looping
        });

        if let Some(body) = &an.body {
            let r = compile_tree(body, reg, env);
            if r != 0 {
                return r;
            }
        }

        add_op(reg, OpCode::CutToMark, OperationPayload::CutToMark {
            id,
            restore_pos: true,
        });
        return 0;
    }

    if at == ANCR_LOOK_BEHIND_NOT {
        // (?<!...) negative lookbehind
        let body_len = if let Some(body) = &an.body {
            compile_length_tree(body, reg, env)
        } else {
            0
        };

        let id = reg.num_call;
        reg.num_call += 1;

        let push_addr = SIZE_INC + OPSIZE_MARK + OPSIZE_STEP_BACK_START +
            body_len + OPSIZE_POP_TO_MARK + OPSIZE_POP;
        add_op(reg, OpCode::Push, OperationPayload::Push { addr: push_addr });

        add_op(reg, OpCode::Mark, OperationPayload::Mark {
            id,
            save_pos: false,
        });

        let char_len = an.char_min_len as i32;
        add_op(reg, OpCode::StepBackStart, OperationPayload::StepBackStart {
            initial: char_len,
            remaining: 0,
            addr: 0,
        });

        if let Some(body) = &an.body {
            let r = compile_tree(body, reg, env);
            if r != 0 {
                return r;
            }
        }

        add_op(reg, OpCode::PopToMark, OperationPayload::PopToMark { id });
        add_op(reg, OpCode::Pop, OperationPayload::None);
        add_op(reg, OpCode::Fail, OperationPayload::None);
        return 0;
    }

    // Simple anchors
    match at {
        ANCR_BEGIN_BUF => {
            add_op(reg, OpCode::BeginBuf, OperationPayload::None);
        }
        ANCR_END_BUF => {
            add_op(reg, OpCode::EndBuf, OperationPayload::None);
        }
        ANCR_BEGIN_LINE => {
            add_op(reg, OpCode::BeginLine, OperationPayload::None);
        }
        ANCR_END_LINE => {
            add_op(reg, OpCode::EndLine, OperationPayload::None);
        }
        ANCR_SEMI_END_BUF => {
            add_op(reg, OpCode::SemiEndBuf, OperationPayload::None);
        }
        ANCR_BEGIN_POSITION => {
            add_op(reg, OpCode::CheckPosition, OperationPayload::CheckPosition {
                check_type: CheckPositionType::SearchStart,
            });
        }
        ANCR_WORD_BOUNDARY => {
            let mode = if an.ascii_mode { 1 } else { 0 };
            add_op(reg, OpCode::WordBoundary, OperationPayload::WordBoundary { mode });
        }
        ANCR_NO_WORD_BOUNDARY => {
            let mode = if an.ascii_mode { 1 } else { 0 };
            add_op(reg, OpCode::NoWordBoundary, OperationPayload::WordBoundary { mode });
        }
        ANCR_WORD_BEGIN => {
            let mode = if an.ascii_mode { 1 } else { 0 };
            add_op(reg, OpCode::WordBegin, OperationPayload::WordBoundary { mode });
        }
        ANCR_WORD_END => {
            let mode = if an.ascii_mode { 1 } else { 0 };
            add_op(reg, OpCode::WordEnd, OperationPayload::WordBoundary { mode });
        }
        ANCR_TEXT_SEGMENT_BOUNDARY => {
            add_op(reg, OpCode::TextSegmentBoundary, OperationPayload::TextSegmentBoundary {
                boundary_type: TextSegmentBoundaryType::ExtendedGraphemeCluster,
                not: false,
            });
        }
        ANCR_NO_TEXT_SEGMENT_BOUNDARY => {
            add_op(reg, OpCode::TextSegmentBoundary, OperationPayload::TextSegmentBoundary {
                boundary_type: TextSegmentBoundaryType::ExtendedGraphemeCluster,
                not: true,
            });
        }
        _ => {
            return ONIGERR_TYPE_BUG as i32;
        }
    }

    0
}

// ============================================================================
// Gimmick compilation
// ============================================================================

/// Calculate bytecode length for a gimmick node.
fn compile_length_gimmick_node(gn: &GimmickNode) -> i32 {
    match gn.gimmick_type {
        GimmickType::Fail => SIZE_INC,
        GimmickType::Save => OPSIZE_SAVE_VAL,
        GimmickType::UpdateVar => OPSIZE_UPDATE_VAR,
        GimmickType::Callout => {
            // TODO: callout compilation
            SIZE_INC
        }
    }
}

/// Compile a gimmick node to bytecode.
fn compile_gimmick_node(gn: &GimmickNode, reg: &mut RegexType, env: &ParseEnv) -> i32 {
    match gn.gimmick_type {
        GimmickType::Fail => {
            add_op(reg, OpCode::Fail, OperationPayload::None);
        }
        GimmickType::Save => {
            add_op(reg, OpCode::SaveVal, OperationPayload::SaveVal {
                save_type: SaveType::Keep, // TODO: determine from detail_type
                id: gn.id,
            });
        }
        GimmickType::UpdateVar => {
            add_op(reg, OpCode::UpdateVar, OperationPayload::UpdateVar {
                var_type: UpdateVarType::KeepFromStackLast, // TODO: determine from detail_type
                id: gn.id,
                clear: false,
            });
        }
        GimmickType::Callout => {
            // TODO: callout compilation
        }
    }
    0
}

// ============================================================================
// Main compilation passes
// ============================================================================

/// Pass 1: Calculate the bytecode length needed for a node tree.
/// Returns the number of operations that will be generated.
pub fn compile_length_tree(node: &Node, reg: &RegexType, env: &ParseEnv) -> i32 {
    match &node.inner {
        NodeInner::List(cons) => {
            let mut len = 0i32;
            // Compile car
            len += compile_length_tree(&cons.car, reg, env);
            // Walk cdr chain
            let mut cur = cons.cdr.as_ref();
            while let Some(next) = cur {
                if let NodeInner::List(c) = &next.inner {
                    len += compile_length_tree(&c.car, reg, env);
                    cur = c.cdr.as_ref();
                } else {
                    len += compile_length_tree(next, reg, env);
                    break;
                }
            }
            len
        }

        NodeInner::Alt(cons) => {
            // For alternation, each branch needs PUSH + body + JUMP (except last)
            let mut total = 0i32;
            let mut n_alts = 0i32;

            // First alternative
            let first_len = compile_length_tree(&cons.car, reg, env);
            total += first_len;
            n_alts += 1;

            let mut cur = cons.cdr.as_ref();
            while let Some(next) = cur {
                if let NodeInner::Alt(c) = &next.inner {
                    let branch_len = compile_length_tree(&c.car, reg, env);
                    total += branch_len;
                    n_alts += 1;
                    cur = c.cdr.as_ref();
                } else {
                    let branch_len = compile_length_tree(next, reg, env);
                    total += branch_len;
                    n_alts += 1;
                    cur = None;
                }
            }

            // Each branch except the last needs PUSH + JUMP
            total += (n_alts - 1) * (OPSIZE_PUSH + OPSIZE_JUMP);
            total
        }

        NodeInner::String(_) => {
            let sn = node.as_str().unwrap();
            if sn.is_crude() {
                compile_length_string_crude_node(node, reg)
            } else {
                compile_length_string_node(node, reg)
            }
        }

        NodeInner::CClass(cc) => {
            compile_length_cclass_node(cc, reg)
        }

        NodeInner::CType(ct) => {
            SIZE_INC
        }

        NodeInner::BackRef(_br) => {
            OPSIZE_BACKREF
        }

        NodeInner::Quant(qn) => {
            compile_length_quantifier_node(qn, reg, env)
        }

        NodeInner::Bag(bag) => {
            compile_length_bag_node(bag, reg, env)
        }

        NodeInner::Anchor(an) => {
            compile_length_anchor_node(an, reg, env)
        }

        NodeInner::Gimmick(gn) => {
            compile_length_gimmick_node(gn)
        }

        NodeInner::Call(_) => {
            OPSIZE_CALL
        }
    }
}

/// Pass 2: Generate bytecode operations from the node tree.
/// Returns 0 on success or a negative error code.
pub fn compile_tree(node: &Node, reg: &mut RegexType, env: &ParseEnv) -> i32 {
    match &node.inner {
        NodeInner::List(cons) => {
            let r = compile_tree(&cons.car, reg, env);
            if r != 0 {
                return r;
            }
            let mut cur = cons.cdr.as_ref();
            while let Some(next) = cur {
                if let NodeInner::List(c) = &next.inner {
                    let r = compile_tree(&c.car, reg, env);
                    if r != 0 {
                        return r;
                    }
                    cur = c.cdr.as_ref();
                } else {
                    return compile_tree(next, reg, env);
                }
            }
            0
        }

        NodeInner::Alt(cons) => {
            // Collect all alternatives to calculate lengths
            let mut branches: Vec<&Node> = Vec::new();
            branches.push(&cons.car);

            let mut cur = cons.cdr.as_ref();
            while let Some(next) = cur {
                if let NodeInner::Alt(c) = &next.inner {
                    branches.push(&c.car);
                    cur = c.cdr.as_ref();
                } else {
                    branches.push(next);
                    cur = None;
                }
            }

            let n = branches.len();
            if n == 1 {
                return compile_tree(branches[0], reg, env);
            }

            // Pre-calculate remaining lengths for PUSH/JUMP addresses
            let mut branch_lens: Vec<i32> = Vec::with_capacity(n);
            for b in &branches {
                branch_lens.push(compile_length_tree(b, reg, env));
            }

            // For each branch except the last: PUSH + body + JUMP
            // Last branch: just body
            let mut remaining_len = 0i32;
            for i in (1..n).rev() {
                remaining_len += branch_lens[i];
                if i < n - 1 {
                    remaining_len += OPSIZE_PUSH + OPSIZE_JUMP;
                }
            }

            for i in 0..n {
                if i < n - 1 {
                    // PUSH to next alternative
                    let push_addr = SIZE_INC + branch_lens[i] + OPSIZE_JUMP;
                    add_op(reg, OpCode::Push, OperationPayload::Push { addr: push_addr });
                }

                let r = compile_tree(branches[i], reg, env);
                if r != 0 {
                    return r;
                }

                if i < n - 1 {
                    // JUMP past remaining alternatives
                    remaining_len -= branch_lens[i + 1];
                    if i + 1 < n - 1 {
                        remaining_len -= OPSIZE_PUSH + OPSIZE_JUMP;
                    }
                    let jump_addr = SIZE_INC + remaining_len;
                    if i + 1 < n - 1 {
                        add_op(reg, OpCode::Jump, OperationPayload::Jump {
                            addr: jump_addr + (n as i32 - i as i32 - 2) * (OPSIZE_PUSH + OPSIZE_JUMP),
                        });
                    } else {
                        add_op(reg, OpCode::Jump, OperationPayload::Jump {
                            addr: SIZE_INC + branch_lens[i + 1],
                        });
                    }
                }
            }
            0
        }

        NodeInner::String(_) => {
            let sn = node.as_str().unwrap();
            if sn.is_crude() {
                compile_string_crude_node(node, reg)
            } else {
                compile_string_node(node, reg)
            }
        }

        NodeInner::CClass(cc) => {
            compile_cclass_node(cc, reg)
        }

        NodeInner::CType(ct) => {
            let opcode = match ct.ctype as u32 {
                ONIGENC_CTYPE_WORD => {
                    if ct.not {
                        if ct.ascii_mode { OpCode::NoWordAscii } else { OpCode::NoWord }
                    } else {
                        if ct.ascii_mode { OpCode::WordAscii } else { OpCode::Word }
                    }
                }
                _ => {
                    // Anychar type
                    if node.has_status(ND_ST_MULTILINE) {
                        OpCode::AnyCharMl
                    } else {
                        OpCode::AnyChar
                    }
                }
            };
            add_op(reg, opcode, OperationPayload::None);
            0
        }

        NodeInner::BackRef(br) => {
            let refs = br.back_refs();

            if refs.len() == 1 {
                let n = refs[0];
                match n {
                    1 => {
                        add_op(reg, OpCode::BackRef1, OperationPayload::None);
                    }
                    2 => {
                        add_op(reg, OpCode::BackRef2, OperationPayload::None);
                    }
                    _ => {
                        if node.has_status(ND_ST_IGNORECASE) {
                            add_op(reg, OpCode::BackRefNIc, OperationPayload::BackRefN { n1: n });
                        } else {
                            add_op(reg, OpCode::BackRefN, OperationPayload::BackRefN { n1: n });
                        }
                    }
                }
            } else {
                // Multi backref
                let ns = refs.to_vec();
                if node.has_status(ND_ST_IGNORECASE) {
                    add_op(reg, OpCode::BackRefMultiIc, OperationPayload::BackRefGeneral {
                        num: refs.len() as i32,
                        ns,
                        nest_level: 0,
                    });
                } else {
                    add_op(reg, OpCode::BackRefMulti, OperationPayload::BackRefGeneral {
                        num: refs.len() as i32,
                        ns,
                        nest_level: 0,
                    });
                }
            }
            0
        }

        NodeInner::Quant(qn) => {
            compile_quantifier_node(qn, reg, env)
        }

        NodeInner::Bag(bag) => {
            compile_bag_node(bag, reg, env)
        }

        NodeInner::Anchor(an) => {
            compile_anchor_node(an, reg, env)
        }

        NodeInner::Gimmick(gn) => {
            compile_gimmick_node(gn, reg, env)
        }

        NodeInner::Call(call) => {
            add_op(reg, OpCode::Call, OperationPayload::Call {
                addr: call.called_gnum, // Will be resolved to actual address later
            });
            0
        }
    }
}

// ============================================================================
// Entry points
// ============================================================================

/// Helper: check if mem_status bit 0 is on (meaning "all on")
#[inline]
fn mem_status_is_all_on(stats: MemStatusType) -> bool {
    (stats & 1) != 0
}

/// Flatten a List node into a Vec of car elements.
fn flatten_list(mut node: Box<Node>) -> Vec<Box<Node>> {
    let mut items = Vec::new();
    loop {
        match node.inner {
            NodeInner::List(cons) => {
                items.push(cons.car);
                match cons.cdr {
                    Some(next) => node = next,
                    None => break,
                }
            }
            _ => {
                // Shouldn't happen - the last cdr should be None
                items.push(node);
                break;
            }
        }
    }
    items
}

/// Rebuild a List node from a Vec of car elements.
fn rebuild_list(items: Vec<Box<Node>>) -> Box<Node> {
    let mut items = items;
    assert!(!items.is_empty());
    let mut result = Node {
        status: 0,
        parent: std::ptr::null_mut(),
        inner: NodeInner::List(ConsAltNode {
            car: items.pop().unwrap(),
            cdr: None,
        }),
    };
    while let Some(item) = items.pop() {
        result = Node {
            status: 0,
            parent: std::ptr::null_mut(),
            inner: NodeInner::List(ConsAltNode {
                car: item,
                cdr: Some(Box::new(result)),
            }),
        };
    }
    Box::new(result)
}

/// Consolidate adjacent string nodes in the parse tree.
/// Mirrors C's reduce_string_list() from regcomp.c.
pub fn reduce_string_list(node: &mut Node, enc: OnigEncoding) -> i32 {
    match &mut node.inner {
        NodeInner::List(_) => {
            // Take ownership of the list, flatten, merge, rebuild
            let placeholder = NodeInner::String(StrNode { s: Vec::new(), flag: 0 });
            let old_inner = std::mem::replace(&mut node.inner, placeholder);
            let list_node = Box::new(Node {
                status: 0,
                parent: std::ptr::null_mut(),
                inner: old_inner,
            });
            let mut items = flatten_list(list_node);

            // First recurse into non-string children
            for item in items.iter_mut() {
                if item.node_type() != NodeType::String {
                    let r = reduce_string_list(item, enc);
                    if r != 0 {
                        // Rebuild and put back before returning error
                        node.inner = rebuild_list(items).inner;
                        return r;
                    }
                }
            }

            // Merge adjacent string nodes with same flags and status
            let mut merged: Vec<Box<Node>> = Vec::new();
            for item in items {
                if item.node_type() == NodeType::String {
                    let can_merge = if let Some(last) = merged.last() {
                        if last.node_type() == NodeType::String {
                            let last_str = last.as_str().unwrap();
                            let curr_str = item.as_str().unwrap();
                            last_str.flag == curr_str.flag && last.status == item.status
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if can_merge {
                        let curr_bytes = item.as_str().unwrap().s.clone();
                        let last = merged.last_mut().unwrap();
                        last.as_str_mut().unwrap().s.extend_from_slice(&curr_bytes);
                    } else {
                        merged.push(item);
                    }
                } else {
                    merged.push(item);
                }
            }

            // Rebuild the list
            if merged.len() == 1 {
                // Single node: unwrap from list
                let single = merged.into_iter().next().unwrap();
                *node = *single;
            } else {
                node.inner = rebuild_list(merged).inner;
            }

            0
        }

        NodeInner::Alt(_) => {
            // Recurse into each alternative
            let placeholder = NodeInner::String(StrNode { s: Vec::new(), flag: 0 });
            let old_inner = std::mem::replace(&mut node.inner, placeholder);
            let alt_node = Box::new(Node {
                status: 0,
                parent: std::ptr::null_mut(),
                inner: old_inner,
            });

            // Flatten the alt chain
            let mut items = Vec::new();
            let mut current: Option<Box<Node>> = Some(alt_node);
            while let Some(n) = current {
                match n.inner {
                    NodeInner::Alt(cons) => {
                        items.push(cons.car);
                        current = cons.cdr;
                    }
                    _ => {
                        items.push(n);
                        current = None;
                    }
                }
            }

            // Recurse into each alternative
            for item in items.iter_mut() {
                let r = reduce_string_list(item, enc);
                if r != 0 {
                    // Rebuild alt chain and put back
                    let mut result = Node {
                        status: 0,
                        parent: std::ptr::null_mut(),
                        inner: NodeInner::Alt(ConsAltNode {
                            car: items.pop().unwrap(),
                            cdr: None,
                        }),
                    };
                    while let Some(item) = items.pop() {
                        result = Node {
                            status: 0,
                            parent: std::ptr::null_mut(),
                            inner: NodeInner::Alt(ConsAltNode {
                                car: item,
                                cdr: Some(Box::new(result)),
                            }),
                        };
                    }
                    *node = result;
                    return r;
                }
            }

            // Rebuild alt chain
            let mut items_rev: Vec<Box<Node>> = items;
            let last = items_rev.pop().unwrap();
            let mut result = Node {
                status: 0,
                parent: std::ptr::null_mut(),
                inner: NodeInner::Alt(ConsAltNode {
                    car: last,
                    cdr: None,
                }),
            };
            while let Some(item) = items_rev.pop() {
                result = Node {
                    status: 0,
                    parent: std::ptr::null_mut(),
                    inner: NodeInner::Alt(ConsAltNode {
                        car: item,
                        cdr: Some(Box::new(result)),
                    }),
                };
            }
            *node = result;
            0
        }

        NodeInner::Quant(ref mut q) => {
            if let Some(ref mut body) = q.body {
                reduce_string_list(body, enc)
            } else {
                0
            }
        }

        NodeInner::Anchor(ref mut a) => {
            if let Some(ref mut body) = a.body {
                let r = reduce_string_list(body, enc);
                if r != 0 { return r; }
            }
            0
        }

        NodeInner::Bag(ref mut b) => {
            if let Some(ref mut body) = b.body {
                let r = reduce_string_list(body, enc);
                if r != 0 { return r; }
            }
            if let BagData::IfElse { ref mut then_node, ref mut else_node } = b.bag_data {
                if let Some(ref mut then_n) = then_node {
                    let r = reduce_string_list(then_n, enc);
                    if r != 0 { return r; }
                }
                if let Some(ref mut else_n) = else_node {
                    let r = reduce_string_list(else_n, enc);
                    if r != 0 { return r; }
                }
            }
            0
        }

        _ => 0,
    }
}

/// Simple compilation from a pre-parsed AST tree.
/// Used internally and by tests that parse separately.
pub fn compile_from_tree(
    root: &Node,
    reg: &mut RegexType,
    env: &ParseEnv,
) -> i32 {
    // Clear previous bytecode
    reg.ops.clear();

    // Compile the tree to bytecode
    let r = compile_tree(root, reg, env);
    if r != 0 {
        return r;
    }

    // Add OP_END
    add_op(reg, OpCode::End, OperationPayload::None);

    0
}

/// Full compilation entry point - mirrors C's onig_compile().
/// Parses pattern, compiles to bytecode, sets up mem status and stack_pop_level.
pub fn onig_compile(
    reg: &mut RegexType,
    pattern: &[u8],
) -> i32 {
    // Clear previous bytecode
    reg.ops.clear();

    // Parse the pattern into AST
    let mut env = ParseEnv {
        options: reg.options,
        case_fold_flag: reg.case_fold_flag,
        enc: reg.enc,
        syntax: unsafe { &*reg.syntax },
        cap_history: 0,
        backtrack_mem: 0,
        backrefed_mem: 0,
        pattern: std::ptr::null(),
        pattern_end: std::ptr::null(),
        error: std::ptr::null(),
        error_end: std::ptr::null(),
        reg: reg as *mut RegexType,
        num_call: 0,
        num_mem: 0,
        num_named: 0,
        mem_alloc: 0,
        mem_env_static: Default::default(),
        mem_env_dynamic: None,
        backref_num: 0,
        keep_num: 0,
        id_num: 0,
        save_alloc_num: 0,
        saves: None,
        unset_addr_list: None,
        parse_depth: 0,
        flags: 0,
    };

    let mut root = match crate::regparse::onig_parse_tree(pattern, reg, &mut env) {
        Ok(node) => node,
        Err(e) => return e,
    };

    // Optimize: consolidate adjacent string nodes (mirrors C's reduce_string_list)
    let r = reduce_string_list(&mut root, reg.enc);
    if r != 0 {
        return r;
    }

    // Set capture/mem tracking from parse env (mirrors C's onig_compile post-parse setup)
    reg.capture_history = env.cap_history;
    reg.push_mem_start = env.backtrack_mem | env.cap_history;
    reg.num_mem = env.num_mem;

    // Set push_mem_end
    if mem_status_is_all_on(reg.push_mem_start) {
        reg.push_mem_end = env.backrefed_mem | env.cap_history;
    } else {
        reg.push_mem_end = reg.push_mem_start & (env.backrefed_mem | env.cap_history);
    }

    // Compile the tree to bytecode
    let r = compile_tree(&root, reg, &env);
    if r != 0 {
        return r;
    }

    // Add OP_END
    add_op(reg, OpCode::End, OperationPayload::None);

    // Set stack pop level based on what captures/features are used
    if reg.push_mem_end != 0
        || reg.num_repeat != 0
        || reg.num_empty_check != 0
        || reg.num_call > 0
    {
        reg.stack_pop_level = StackPopLevel::All;
    } else if reg.push_mem_start != 0 {
        reg.stack_pop_level = StackPopLevel::MemStart;
    } else {
        reg.stack_pop_level = StackPopLevel::Free;
    }

    0
}

/// Create and compile a new regex - mirrors C's onig_new().
/// This is the main public API entry point.
pub fn onig_new(
    pattern: &[u8],
    option: OnigOptionType,
    enc: OnigEncoding,
    syntax: *const OnigSyntaxType,
) -> Result<RegexType, i32> {
    // Validate options
    if (option & ONIG_OPTION_DONT_CAPTURE_GROUP) != 0
        && (option & ONIG_OPTION_CAPTURE_GROUP) != 0
    {
        return Err(ONIGERR_INVALID_COMBINATION_OF_OPTIONS);
    }

    // Apply syntax default options (mirrors onig_reg_init)
    let mut effective_option = option;
    let syn = unsafe { &*syntax };
    if (option & ONIG_OPTION_NEGATE_SINGLELINE) != 0 {
        effective_option |= syn.options;
        effective_option &= !ONIG_OPTION_SINGLELINE;
    } else {
        effective_option |= syn.options;
    }

    // Case fold flag setup
    let mut case_fold_flag = ONIGENC_CASE_FOLD_MIN;
    if (effective_option & ONIG_OPTION_IGNORECASE_IS_ASCII) != 0 {
        case_fold_flag &= !(INTERNAL_ONIGENC_CASE_FOLD_MULTI_CHAR
            | ONIGENC_CASE_FOLD_TURKISH_AZERI);
        case_fold_flag |= ONIGENC_CASE_FOLD_ASCII_ONLY;
    }

    let mut reg = RegexType {
        ops: Vec::new(),
        string_pool: Vec::new(),
        num_mem: 0,
        num_repeat: 0,
        num_empty_check: 0,
        num_call: 0,
        capture_history: 0,
        push_mem_start: 0,
        push_mem_end: 0,
        stack_pop_level: StackPopLevel::Free,
        repeat_range: Vec::new(),
        enc,
        options: effective_option,
        syntax,
        case_fold_flag,
        name_table: None,
        optimize: OptimizeType::None,
        threshold_len: 0,
        anchor: 0,
        anc_dist_min: 0,
        anc_dist_max: 0,
        sub_anchor: 0,
        exact: Vec::new(),
        map: [0u8; CHAR_MAP_SIZE],
        map_offset: 0,
        dist_min: 0,
        dist_max: 0,
        extp: None,
    };

    let r = onig_compile(&mut reg, pattern);
    if r != 0 {
        return Err(r);
    }

    Ok(reg)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regparse;
    use crate::regsyntax::OnigSyntaxOniguruma;

    fn make_test_context() -> (RegexType, ParseEnv) {
        let reg = RegexType {
            ops: Vec::new(),
            string_pool: Vec::new(),
            num_mem: 0,
            num_repeat: 0,
            num_empty_check: 0,
            num_call: 0,
            capture_history: 0,
            push_mem_start: 0,
            push_mem_end: 0,
            stack_pop_level: StackPopLevel::Free,
            repeat_range: Vec::new(),
            enc: &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            options: ONIG_OPTION_NONE,
            syntax: &OnigSyntaxOniguruma as *const OnigSyntaxType,
            case_fold_flag: ONIGENC_CASE_FOLD_MIN,
            name_table: None,
            optimize: OptimizeType::None,
            threshold_len: 0,
            anchor: 0,
            anc_dist_min: 0,
            anc_dist_max: 0,
            sub_anchor: 0,
            exact: Vec::new(),
            map: [0u8; CHAR_MAP_SIZE],
            map_offset: 0,
            dist_min: 0,
            dist_max: 0,
            extp: None,
        };
        let env = ParseEnv {
            options: 0,
            case_fold_flag: 0,
            enc: &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            syntax: &OnigSyntaxOniguruma,
            cap_history: 0,
            backtrack_mem: 0,
            backrefed_mem: 0,
            pattern: std::ptr::null(),
            pattern_end: std::ptr::null(),
            error: std::ptr::null(),
            error_end: std::ptr::null(),
            reg: std::ptr::null_mut(),
            num_call: 0,
            num_mem: 0,
            num_named: 0,
            mem_alloc: 0,
            mem_env_static: Default::default(),
            mem_env_dynamic: None,
            backref_num: 0,
            keep_num: 0,
            id_num: 0,
            save_alloc_num: 0,
            saves: None,
            unset_addr_list: None,
            parse_depth: 0,
            flags: 0,
        };
        (reg, env)
    }

    fn parse_and_compile(pattern: &[u8]) -> Result<RegexType, i32> {
        let (mut reg, mut env) = make_test_context();
        let root = regparse::onig_parse_tree(pattern, &mut reg, &mut env)?;
        let r = compile_from_tree(&root, &mut reg, &env);
        if r != 0 {
            return Err(r);
        }
        Ok(reg)
    }

    #[test]
    fn compile_literal_string() {
        let reg = parse_and_compile(b"abc").unwrap();
        assert!(!reg.ops.is_empty());
        // Should have string op + END
        let last = reg.ops.last().unwrap();
        assert_eq!(last.opcode, OpCode::End);
    }

    #[test]
    fn compile_alternation() {
        let reg = parse_and_compile(b"a|b").unwrap();
        // Should have PUSH + "a" + JUMP + "b" + END
        assert!(reg.ops.len() >= 4);
        assert_eq!(reg.ops[0].opcode, OpCode::Push);
        assert_eq!(reg.ops.last().unwrap().opcode, OpCode::End);
    }

    #[test]
    fn compile_star_quantifier() {
        let reg = parse_and_compile(b"a*").unwrap();
        // Should have PUSH + Str1 + JUMP + END
        assert!(reg.ops.len() >= 3);
        assert_eq!(reg.ops.last().unwrap().opcode, OpCode::End);
        // Check that a PUSH and JUMP are present
        let has_push = reg.ops.iter().any(|op| op.opcode == OpCode::Push);
        let has_jump = reg.ops.iter().any(|op| op.opcode == OpCode::Jump);
        assert!(has_push, "expected PUSH for a*");
        assert!(has_jump, "expected JUMP for a*");
    }

    #[test]
    fn compile_plus_quantifier() {
        let reg = parse_and_compile(b"a+").unwrap();
        // a+ = body + PUSH + body + JUMP
        assert!(reg.ops.len() >= 3);
        assert_eq!(reg.ops.last().unwrap().opcode, OpCode::End);
    }

    #[test]
    fn compile_capture_group() {
        let reg = parse_and_compile(b"(a)").unwrap();
        let has_mem_start = reg.ops.iter().any(|op| op.opcode == OpCode::MemStart || op.opcode == OpCode::MemStartPush);
        let has_mem_end = reg.ops.iter().any(|op| op.opcode == OpCode::MemEnd || op.opcode == OpCode::MemEndPush);
        assert!(has_mem_start, "expected MemStart for (a)");
        assert!(has_mem_end, "expected MemEnd for (a)");
    }

    #[test]
    fn compile_char_class() {
        let reg = parse_and_compile(b"[abc]").unwrap();
        let has_cclass = reg.ops.iter().any(|op| op.opcode == OpCode::CClass);
        assert!(has_cclass, "expected CClass for [abc]");
    }

    #[test]
    fn compile_anchor_begin() {
        let reg = parse_and_compile(b"^a").unwrap();
        assert_eq!(reg.ops[0].opcode, OpCode::BeginLine);
    }

    #[test]
    fn compile_word_type() {
        let reg = parse_and_compile(b"\\w").unwrap();
        let has_word = reg.ops.iter().any(|op| op.opcode == OpCode::Word || op.opcode == OpCode::WordAscii);
        assert!(has_word, "expected Word for \\w");
    }

    #[test]
    fn compile_interval_quantifier() {
        let reg = parse_and_compile(b"a{2,5}").unwrap();
        let has_repeat = reg.ops.iter().any(|op| op.opcode == OpCode::Repeat || op.opcode == OpCode::RepeatNg);
        assert!(has_repeat, "expected Repeat for a{{2,5}}");
        assert_eq!(reg.num_repeat, 1);
    }

    #[test]
    fn compile_complex_pattern() {
        let reg = parse_and_compile(b"^[a-z]+\\d{2,4}$").unwrap();
        assert_eq!(reg.ops.last().unwrap().opcode, OpCode::End);
        // Just verify it compiles without error
    }

    #[test]
    fn compile_empty_pattern() {
        let reg = parse_and_compile(b"").unwrap();
        assert_eq!(reg.ops.len(), 1); // Just END
        assert_eq!(reg.ops[0].opcode, OpCode::End);
    }

    #[test]
    fn compile_non_capturing_group() {
        let reg = parse_and_compile(b"(?:abc)").unwrap();
        // Non-capturing group should not emit MemStart/MemEnd
        let has_mem_start = reg.ops.iter().any(|op| op.opcode == OpCode::MemStart || op.opcode == OpCode::MemStartPush);
        assert!(!has_mem_start, "non-capturing group should not have MemStart");
        assert_eq!(reg.ops.last().unwrap().opcode, OpCode::End);
    }

    #[test]
    fn compile_lookahead() {
        let reg = parse_and_compile(b"(?=abc)").unwrap();
        let has_mark = reg.ops.iter().any(|op| op.opcode == OpCode::Mark);
        let has_cut = reg.ops.iter().any(|op| op.opcode == OpCode::CutToMark);
        assert!(has_mark, "expected Mark for lookahead");
        assert!(has_cut, "expected CutToMark for lookahead");
    }

    #[test]
    fn compile_negative_lookahead() {
        let reg = parse_and_compile(b"(?!abc)").unwrap();
        let has_fail = reg.ops.iter().any(|op| op.opcode == OpCode::Fail);
        assert!(has_fail, "expected Fail for negative lookahead");
    }

    // ---- onig_new API tests ----

    #[test]
    fn onig_new_basic() {
        let reg = onig_new(
            b"abc",
            ONIG_OPTION_NONE,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma as *const OnigSyntaxType,
        ).unwrap();
        assert!(!reg.ops.is_empty());
        assert_eq!(reg.ops.last().unwrap().opcode, OpCode::End);
    }

    #[test]
    fn onig_new_with_captures() {
        let reg = onig_new(
            b"(a)(b)",
            ONIG_OPTION_NONE,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma as *const OnigSyntaxType,
        ).unwrap();
        assert_eq!(reg.num_mem, 2);
    }

    #[test]
    fn onig_new_stack_pop_level_free() {
        // Simple pattern with no captures => StackPopLevel::Free
        let reg = onig_new(
            b"abc",
            ONIG_OPTION_NONE,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma as *const OnigSyntaxType,
        ).unwrap();
        assert_eq!(reg.stack_pop_level, StackPopLevel::Free);
    }

    #[test]
    fn onig_new_invalid_pattern() {
        let result = onig_new(
            b"(",
            ONIG_OPTION_NONE,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &OnigSyntaxOniguruma as *const OnigSyntaxType,
        );
        assert!(result.is_err());
    }

    #[test]
    fn reduce_string_list_merges() {
        // Parse "abc" - parser produces 3 single-char string nodes in a list
        // reduce_string_list should merge them into one "abc" node
        let (mut reg, mut env) = make_test_context();
        let mut root = regparse::onig_parse_tree(b"abc", &mut reg, &mut env).unwrap();

        // Before reduction, count the tree structure
        let before_type = root.node_type();

        // Apply reduction
        let r = reduce_string_list(&mut root, env.enc);
        assert_eq!(r, 0);

        // After reduction, "abc" should be a single string node (not a list)
        assert_eq!(root.node_type(), NodeType::String);
        let s = root.as_str().unwrap();
        assert_eq!(s.s, b"abc");
    }

    #[test]
    fn reduce_string_list_preserves_non_strings() {
        // "a.b" has string-dot-string, so strings cannot merge across the dot
        let (mut reg, mut env) = make_test_context();
        let mut root = regparse::onig_parse_tree(b"a.b", &mut reg, &mut env).unwrap();
        let r = reduce_string_list(&mut root, env.enc);
        assert_eq!(r, 0);
        // Should still be a list with 3 elements
        assert_eq!(root.node_type(), NodeType::List);
    }
}
