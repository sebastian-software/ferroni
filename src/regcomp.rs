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

    let payload = if mb_len == 1 && str_len <= 5 {
        // Single-byte encoding, Str1-Str5: use compact Exact payload
        let mut buf = [0u8; 16];
        buf[..byte_len as usize].copy_from_slice(&s[..byte_len as usize]);
        OperationPayload::Exact { s: buf }
    } else if mb_len == 1 {
        // Single-byte encoding, StrN: use ExactN payload
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
/// Collect a bitmask of capture group regnums present in a node tree.
fn collect_mem_status(node: &Node) -> u32 {
    let mut status: u32 = 0;
    match &node.inner {
        NodeInner::List(_) | NodeInner::Alt(_) => {
            let mut cur = node;
            loop {
                let (car, cdr) = match &cur.inner {
                    NodeInner::List(cons) => (&cons.car, &cons.cdr),
                    NodeInner::Alt(cons) => (&cons.car, &cons.cdr),
                    _ => break,
                };
                status |= collect_mem_status(car);
                match cdr {
                    Some(next) => cur = next,
                    None => break,
                }
            }
        }
        NodeInner::Quant(qn) => {
            if let Some(ref body) = qn.body {
                status |= collect_mem_status(body);
            }
        }
        NodeInner::Bag(bn) => {
            if bn.bag_type == BagType::Memory {
                if let BagData::Memory { regnum, .. } = bn.bag_data {
                    if regnum > 0 && regnum < 31 {
                        status |= 1u32 << regnum;
                    }
                }
            }
            if let Some(ref body) = bn.body {
                status |= collect_mem_status(body);
            }
        }
        _ => {}
    }
    status
}

fn compile_quant_body_with_empty_check(
    node: &Node,
    reg: &mut RegexType,
    env: &ParseEnv,
    emptiness: BodyEmptyType,
    qn_empty_status_mem: u32,
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
        let empty_status_mem = if emptiness == BodyEmptyType::MayBeEmptyMem
            || emptiness == BodyEmptyType::MayBeEmptyRec {
            if qn_empty_status_mem != 0 {
                qn_empty_status_mem
            } else {
                collect_mem_status(node)
            }
        } else {
            0
        };
        let opcode = match emptiness {
            BodyEmptyType::MayBeEmptyMem => {
                if qn_empty_status_mem != 0 {
                    OpCode::EmptyCheckEndMemst
                } else {
                    // No external backrefs to tracked captures → use plain empty check
                    OpCode::EmptyCheckEnd
                }
            }
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
        if qn.include_referred != 0 {
            // {0} with CALLED group: JUMP + body
            let tlen = compile_length_tree(body, reg, env);
            return OPSIZE_JUMP + tlen;
        }
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
            body_len * qn.lower + OPSIZE_PUSH + mod_tlen + OPSIZE_JUMP
        } else {
            // {n,} or {n,}?
            let n_body_len = compile_length_tree_n_times(body, qn.lower, reg, env);
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
    } else if !qn.greedy && qn.upper == 1 && qn.lower == 0 {
        // ?? path: PUSH + JUMP + body
        OPSIZE_PUSH + OPSIZE_JUMP + body_len
    } else if qn.greedy && !is_infinite_repeat(qn.upper) {
        // Greedy expansion: lower*body + (upper-lower)*(PUSH+body)
        let n = qn.upper - qn.lower;
        body_len * qn.lower + n * (OPSIZE_PUSH + body_len)
    } else {
        // {n,m} range repeat (lazy non-trivial)
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
        if qn.include_referred != 0 {
            // {0} with CALLED group: JUMP over body, then compile body
            let tlen = compile_length_tree(body, reg, env);
            add_op(reg, OpCode::Jump, OperationPayload::Jump { addr: tlen + SIZE_INC });
            return compile_tree(body, reg, env);
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
                let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness, qn.empty_status_mem);
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
                let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness, qn.empty_status_mem);
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
            // Compile body n times, then loop
            let r = compile_tree_n_times(body, qn.lower, reg, env);
            if r != 0 {
                return r;
            }

            if qn.greedy {
                add_op(reg, OpCode::Push, OperationPayload::Push {
                    addr: SIZE_INC + mod_tlen + OPSIZE_JUMP,
                });
                let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness, qn.empty_status_mem);
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
                let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness, qn.empty_status_mem);
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
        let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness, qn.empty_status_mem);
        if r != 0 {
            return r;
        }
        add_op(reg, if qn.greedy { OpCode::RepeatInc } else { OpCode::RepeatIncNg },
               OperationPayload::RepeatInc { id });
    } else if !qn.greedy && qn.upper == 1 && qn.lower == 0 {
        // ?? path: PUSH(skip JUMP + SIZE_INC) + JUMP(skip body + SIZE_INC) + body
        // C: COP(reg)->push.addr = SIZE_INC + OPSIZE_JUMP;
        add_op(reg, OpCode::Push, OperationPayload::Push {
            addr: SIZE_INC + OPSIZE_JUMP,
        });
        // C: COP(reg)->jump.addr = body_len + SIZE_INC;
        add_op(reg, OpCode::Jump, OperationPayload::Jump {
            addr: body_len + SIZE_INC,
        });
        let r = compile_tree(body, reg, env);
        if r != 0 {
            return r;
        }
    } else if qn.greedy && !is_infinite_repeat(qn.upper) {
        // Greedy expansion: body*lower + (upper-lower) * (PUSH + body)
        let r = compile_tree_n_times(body, qn.lower, reg, env);
        if r != 0 {
            return r;
        }
        let n = qn.upper - qn.lower;
        // Compute goal position for PUSH addresses
        let goal = reg.ops.len() as i32 + n * (OPSIZE_PUSH + body_len);
        for _i in 0..n {
            let push_addr = goal - reg.ops.len() as i32;
            add_op(reg, OpCode::Push, OperationPayload::Push { addr: push_addr });
            let r = compile_tree(body, reg, env);
            if r != 0 {
                return r;
            }
        }
    } else {
        // {n,m} range repeat (lazy non-trivial)
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
        let r = compile_quant_body_with_empty_check(body, reg, env, qn.emptiness, qn.empty_status_mem);
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
fn compile_length_bag_node(bag: &BagNode, node_status: u32, reg: &RegexType, env: &ParseEnv) -> i32 {
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
            let regnum = match &bag.bag_data {
                BagData::Memory { regnum, .. } => *regnum,
                _ => 0,
            };
            if regnum == 0 && (node_status & ND_ST_CALLED) != 0 {
                // \g<0> wrapper: CALL + JUMP + body + RETURN (no MEM_START/END)
                return OPSIZE_CALL + OPSIZE_JUMP + body_len + OPSIZE_RETURN;
            }
            let mut len = OPSIZE_MEM_START + body_len + OPSIZE_MEM_END;
            if (node_status & ND_ST_CALLED) != 0 {
                // Called group: CALL + JUMP + (MEM_START + body + MEM_END + RETURN)
                len += OPSIZE_CALL + OPSIZE_JUMP + OPSIZE_RETURN;
            }
            len
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
            // Conditional: MARK + PUSH + condition + CUT_TO_MARK + then + JUMP + CUT_TO_MARK + else
            let cond_len = if let Some(b) = body {
                compile_length_tree(b, reg, env)
            } else {
                0
            };
            if cond_len < 0 {
                return cond_len;
            }

            let mut len = OPSIZE_PUSH + OPSIZE_MARK + cond_len + OPSIZE_CUT_TO_MARK;

            if let BagData::IfElse { ref then_node, ref else_node } = bag.bag_data {
                if let Some(ref then_n) = then_node {
                    let tlen = compile_length_tree(then_n, reg, env);
                    if tlen < 0 { return tlen; }
                    len += tlen;
                }

                len += OPSIZE_JUMP + OPSIZE_CUT_TO_MARK;

                if let Some(ref else_n) = else_node {
                    let elen = compile_length_tree(else_n, reg, env);
                    if elen < 0 { return elen; }
                    len += elen;
                }
            }

            len
        }
    }
}

/// Compile a bag memory (capture group) node.
fn compile_bag_memory_node(bag: &BagNode, node_status: u32, reg: &mut RegexType, env: &ParseEnv) -> i32 {
    let regnum = match &bag.bag_data {
        BagData::Memory { regnum, .. } => *regnum,
        _ => return ONIGERR_TYPE_BUG as i32,
    };

    let is_called = (node_status & ND_ST_CALLED) != 0;

    if is_called {
        // Called group: emit CALL + JUMP wrapper
        let body_len = if let Some(body) = &bag.body {
            compile_length_tree(body, reg, env)
        } else {
            0
        };
        if body_len < 0 { return body_len; }

        if regnum == 0 {
            // \g<0> wrapper: simpler layout without MEM_START/END
            // Layout: CALL(entry) + JUMP(skip) + [entry: body + RETURN]
            let callable_len = body_len + OPSIZE_RETURN;

            let call_idx = reg.ops.len();
            let entry_addr = (call_idx + 2) as i32;
            add_op(reg, OpCode::Call, OperationPayload::Call { addr: entry_addr });

            add_op(reg, OpCode::Jump, OperationPayload::Jump {
                addr: callable_len + SIZE_INC,
            });

            let called_addr = reg.ops.len() as i32;
            if reg.called_addrs.is_empty() {
                reg.called_addrs.resize(1, -1);
            }
            reg.called_addrs[0] = called_addr;

            if let Some(body) = &bag.body {
                let r = compile_tree(body, reg, env);
                if r != 0 { return r; }
            }

            add_op(reg, OpCode::Return, OperationPayload::Return);
            return 0;
        }

        // Regular called group: CALL + JUMP + MEM_START + body + MEM_END + RETURN
        let callable_len = OPSIZE_MEM_START + body_len + OPSIZE_MEM_END + OPSIZE_RETURN;

        let call_idx = reg.ops.len();
        let entry_addr = (call_idx + 2) as i32;
        add_op(reg, OpCode::Call, OperationPayload::Call { addr: entry_addr });

        add_op(reg, OpCode::Jump, OperationPayload::Jump {
            addr: callable_len + SIZE_INC,
        });

        let called_addr = reg.ops.len() as i32;
        if reg.called_addrs.len() <= regnum as usize {
            reg.called_addrs.resize(regnum as usize + 1, -1);
        }
        reg.called_addrs[regnum as usize] = called_addr;
    }

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
    let is_recursion = (node_status & ND_ST_RECURSION) != 0;
    if need_push_end {
        let opcode = if is_recursion { OpCode::MemEndPushRec } else { OpCode::MemEndPush };
        add_op(reg, opcode, OperationPayload::MemoryEnd { num: regnum });
    } else {
        let opcode = if is_recursion { OpCode::MemEndRec } else { OpCode::MemEnd };
        add_op(reg, opcode, OperationPayload::MemoryEnd { num: regnum });
    }

    if is_called {
        add_op(reg, OpCode::Return, OperationPayload::Return);
    }

    0
}

/// Compile a bag node to bytecode.
fn compile_bag_node(bag: &BagNode, node_status: u32, reg: &mut RegexType, env: &ParseEnv) -> i32 {
    match bag.bag_type {
        BagType::Memory => {
            compile_bag_memory_node(bag, node_status, reg, env)
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
            let id = reg.num_call;
            reg.num_call += 1;

            // Emit MARK
            add_op(reg, OpCode::Mark, OperationPayload::Mark {
                id,
                save_pos: false,
            });

            // Calculate condition and then lengths for PUSH address
            let cond_len = if let Some(body) = &bag.body {
                compile_length_tree(body, reg, env)
            } else {
                0
            };
            if cond_len < 0 { return cond_len; }

            let then_len = if let BagData::IfElse { ref then_node, .. } = bag.bag_data {
                if let Some(ref then_n) = then_node {
                    compile_length_tree(then_n, reg, env)
                } else {
                    0
                }
            } else {
                0
            };
            if then_len < 0 { return then_len; }

            let jump_len = cond_len + OPSIZE_CUT_TO_MARK + then_len + OPSIZE_JUMP;

            // Emit PUSH to else section
            add_op(reg, OpCode::Push, OperationPayload::Push {
                addr: SIZE_INC + jump_len,
            });

            // Emit condition
            if let Some(body) = &bag.body {
                let r = compile_tree(body, reg, env);
                if r != 0 { return r; }
            }

            // On condition success, cut mark
            add_op(reg, OpCode::CutToMark, OperationPayload::CutToMark {
                id,
                restore_pos: false,
            });

            // Emit then branch
            if let BagData::IfElse { ref then_node, ref else_node } = bag.bag_data {
                if let Some(ref then_n) = then_node {
                    let r = compile_tree(then_n, reg, env);
                    if r != 0 { return r; }
                }

                // Calculate else length for JUMP
                let else_len = if let Some(ref else_n) = else_node {
                    compile_length_tree(else_n, reg, env)
                } else {
                    0
                };
                if else_len < 0 { return else_len; }

                // Jump over else
                add_op(reg, OpCode::Jump, OperationPayload::Jump {
                    addr: OPSIZE_CUT_TO_MARK + else_len + SIZE_INC,
                });

                // On condition failure, cut mark
                add_op(reg, OpCode::CutToMark, OperationPayload::CutToMark {
                    id,
                    restore_pos: false,
                });

                // Emit else branch
                if let Some(ref else_n) = else_node {
                    let r = compile_tree(else_n, reg, env);
                    if r != 0 { return r; }
                }
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
        if an.char_min_len == an.char_max_len {
            // Fixed-length
            OPSIZE_MARK + OPSIZE_STEP_BACK_START + body_len + OPSIZE_CUT_TO_MARK
        } else {
            // Variable-length: SAVE_VAL + UPDATE_VAR + MARK + PUSH + JUMP +
            //   UPDATE_VAR + FAIL + STEP_BACK_START + STEP_BACK_NEXT +
            //   body + CHECK_POSITION + CUT_TO_MARK + UPDATE_VAR
            OPSIZE_SAVE_VAL + OPSIZE_UPDATE_VAR + OPSIZE_MARK + OPSIZE_PUSH +
                OPSIZE_JUMP + OPSIZE_UPDATE_VAR + OPSIZE_FAIL +
                OPSIZE_STEP_BACK_START + OPSIZE_STEP_BACK_NEXT +
                body_len + OPSIZE_CHECK_POSITION + OPSIZE_CUT_TO_MARK +
                OPSIZE_UPDATE_VAR
        }
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
        if an.char_min_len == an.char_max_len {
            // Fixed-length
            OPSIZE_MARK + OPSIZE_PUSH + OPSIZE_STEP_BACK_START + body_len +
                OPSIZE_POP_TO_MARK + OPSIZE_FAIL + OPSIZE_POP
        } else {
            // Variable-length: SAVE_VAL + UPDATE_VAR + MARK + PUSH +
            //   STEP_BACK_START + STEP_BACK_NEXT + body + CHECK_POSITION +
            //   POP_TO_MARK + UPDATE_VAR + POP + FAIL +
            //   UPDATE_VAR + POP + POP
            OPSIZE_SAVE_VAL + OPSIZE_UPDATE_VAR + OPSIZE_MARK + OPSIZE_PUSH +
                OPSIZE_STEP_BACK_START + OPSIZE_STEP_BACK_NEXT +
                body_len + OPSIZE_CHECK_POSITION +
                OPSIZE_POP_TO_MARK + OPSIZE_UPDATE_VAR + OPSIZE_POP + OPSIZE_FAIL +
                OPSIZE_UPDATE_VAR + OPSIZE_POP + OPSIZE_POP
        }
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
        if an.char_min_len == an.char_max_len {
            // (?<=...) positive lookbehind — fixed-length
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
                addr: 1,
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
        } else {
            // (?<=...) positive lookbehind — variable-length
            let mid1 = reg.num_call;
            reg.num_call += 1;
            let mid2 = reg.num_call;
            reg.num_call += 1;

            // SAVE_VAL(RightRange, mid1)
            add_op(reg, OpCode::SaveVal, OperationPayload::SaveVal {
                save_type: SaveType::RightRange,
                id: mid1,
            });
            // UPDATE_VAR(RightRangeToS)
            add_op(reg, OpCode::UpdateVar, OperationPayload::UpdateVar {
                var_type: UpdateVarType::RightRangeToS,
                id: 0,
                clear: false,
            });
            // MARK(mid2, save_pos=false)
            add_op(reg, OpCode::Mark, OperationPayload::Mark {
                id: mid2,
                save_pos: false,
            });
            // PUSH(addr → JUMP instruction, i.e. skip past JUMP to UPDATE_VAR)
            // PUSH is at position X, JUMP at X+1, UPDATE_VAR at X+2
            // So alt target = X + SIZE_INC + OPSIZE_JUMP = X + 2 → UPDATE_VAR
            add_op(reg, OpCode::Push, OperationPayload::Push {
                addr: SIZE_INC + OPSIZE_JUMP,
            });
            // JUMP(addr → past UPDATE_VAR + FAIL to STEP_BACK_START)
            add_op(reg, OpCode::Jump, OperationPayload::Jump {
                addr: SIZE_INC + OPSIZE_UPDATE_VAR + OPSIZE_FAIL,
            });
            // UPDATE_VAR(RightRangeFromStack, mid1, clear=false) — fail path restores right_range
            add_op(reg, OpCode::UpdateVar, OperationPayload::UpdateVar {
                var_type: UpdateVarType::RightRangeFromStack,
                id: mid1,
                clear: false,
            });
            // FAIL
            add_op(reg, OpCode::Fail, OperationPayload::None);

            // STEP_BACK_START(initial=min, remaining=max-min, addr=2)
            let diff = if an.char_max_len != INFINITE_LEN {
                (an.char_max_len - an.char_min_len) as i32
            } else {
                INFINITE_LEN as i32
            };
            add_op(reg, OpCode::StepBackStart, OperationPayload::StepBackStart {
                initial: an.char_min_len as i32,
                remaining: diff,
                addr: 2,
            });
            // STEP_BACK_NEXT
            add_op(reg, OpCode::StepBackNext, OperationPayload::None);

            // <body>
            if let Some(body) = &an.body {
                let r = compile_tree(body, reg, env);
                if r != 0 {
                    return r;
                }
            }

            // CHECK_POSITION(CurrentRightRange)
            add_op(reg, OpCode::CheckPosition, OperationPayload::CheckPosition {
                check_type: CheckPositionType::CurrentRightRange,
            });
            // CUT_TO_MARK(mid2, restore_pos=false)
            add_op(reg, OpCode::CutToMark, OperationPayload::CutToMark {
                id: mid2,
                restore_pos: false,
            });
            // UPDATE_VAR(RightRangeFromStack, mid1, clear=true)
            add_op(reg, OpCode::UpdateVar, OperationPayload::UpdateVar {
                var_type: UpdateVarType::RightRangeFromStack,
                id: mid1,
                clear: true,
            });
        }
        return 0;
    }

    if at == ANCR_LOOK_BEHIND_NOT {
        let body_len = if let Some(body) = &an.body {
            compile_length_tree(body, reg, env)
        } else {
            0
        };

        if an.char_min_len == an.char_max_len {
            // (?<!...) negative lookbehind — fixed-length
            let id = reg.num_call;
            reg.num_call += 1;

            add_op(reg, OpCode::Mark, OperationPayload::Mark {
                id,
                save_pos: false,
            });

            let push_addr = SIZE_INC + OPSIZE_STEP_BACK_START +
                body_len + OPSIZE_POP_TO_MARK + OPSIZE_FAIL;
            add_op(reg, OpCode::Push, OperationPayload::Push { addr: push_addr });

            let char_len = an.char_min_len as i32;
            add_op(reg, OpCode::StepBackStart, OperationPayload::StepBackStart {
                initial: char_len,
                remaining: 0,
                addr: 1,
            });

            if let Some(body) = &an.body {
                let r = compile_tree(body, reg, env);
                if r != 0 {
                    return r;
                }
            }

            add_op(reg, OpCode::PopToMark, OperationPayload::PopToMark { id });
            add_op(reg, OpCode::Fail, OperationPayload::None);
            add_op(reg, OpCode::Pop, OperationPayload::None);
        } else {
            // (?<!...) negative lookbehind — variable-length
            let mid1 = reg.num_call;
            reg.num_call += 1;
            let mid2 = reg.num_call;
            reg.num_call += 1;

            // SAVE_VAL(RightRange, mid1)
            add_op(reg, OpCode::SaveVal, OperationPayload::SaveVal {
                save_type: SaveType::RightRange,
                id: mid1,
            });
            // UPDATE_VAR(RightRangeToS)
            add_op(reg, OpCode::UpdateVar, OperationPayload::UpdateVar {
                var_type: UpdateVarType::RightRangeToS,
                id: 0,
                clear: false,
            });
            // MARK(mid2, save_pos=false)
            add_op(reg, OpCode::Mark, OperationPayload::Mark {
                id: mid2,
                save_pos: false,
            });
            // PUSH(addr → success path past body-matched-fail section)
            // From PUSH: skip STEP_BACK_START + STEP_BACK_NEXT + body + CHECK_POSITION +
            //   POP_TO_MARK + UPDATE_VAR + POP + FAIL
            let push_addr = SIZE_INC + OPSIZE_STEP_BACK_START + OPSIZE_STEP_BACK_NEXT +
                body_len + OPSIZE_CHECK_POSITION +
                OPSIZE_POP_TO_MARK + OPSIZE_UPDATE_VAR + OPSIZE_POP + OPSIZE_FAIL;
            add_op(reg, OpCode::Push, OperationPayload::Push { addr: push_addr });

            // STEP_BACK_START(initial=min, remaining=max-min, addr=2)
            let diff = if an.char_max_len != INFINITE_LEN {
                (an.char_max_len - an.char_min_len) as i32
            } else {
                INFINITE_LEN as i32
            };
            add_op(reg, OpCode::StepBackStart, OperationPayload::StepBackStart {
                initial: an.char_min_len as i32,
                remaining: diff,
                addr: 2,
            });
            // STEP_BACK_NEXT
            add_op(reg, OpCode::StepBackNext, OperationPayload::None);

            // <body>
            if let Some(body) = &an.body {
                let r = compile_tree(body, reg, env);
                if r != 0 {
                    return r;
                }
            }

            // CHECK_POSITION(CurrentRightRange) — body matched here, verify position
            add_op(reg, OpCode::CheckPosition, OperationPayload::CheckPosition {
                check_type: CheckPositionType::CurrentRightRange,
            });
            // POP_TO_MARK(mid2) — body succeeded: clean up mark
            add_op(reg, OpCode::PopToMark, OperationPayload::PopToMark { id: mid2 });
            // UPDATE_VAR(RightRangeFromStack, mid1, clear=false) — restore right_range
            add_op(reg, OpCode::UpdateVar, OperationPayload::UpdateVar {
                var_type: UpdateVarType::RightRangeFromStack,
                id: mid1,
                clear: false,
            });
            // POP — discard outer PUSH's SaveVal
            add_op(reg, OpCode::Pop, OperationPayload::None);
            // FAIL — negative lookbehind: body match = overall failure
            add_op(reg, OpCode::Fail, OperationPayload::None);

            // === Success path (body failed at all positions) ===
            // UPDATE_VAR(RightRangeFromStack, mid1, clear=false) — restore right_range
            add_op(reg, OpCode::UpdateVar, OperationPayload::UpdateVar {
                var_type: UpdateVarType::RightRangeFromStack,
                id: mid1,
                clear: false,
            });
            // POP — discard Mark
            add_op(reg, OpCode::Pop, OperationPayload::None);
            // POP — discard SaveVal
            add_op(reg, OpCode::Pop, OperationPayload::None);
        }
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
            let save_type = match gn.detail_type {
                1 => SaveType::S,
                2 => SaveType::RightRange,
                _ => SaveType::Keep,
            };
            add_op(reg, OpCode::SaveVal, OperationPayload::SaveVal {
                save_type,
                id: gn.id,
            });
        }
        GimmickType::UpdateVar => {
            let var_type = match gn.detail_type {
                1 => UpdateVarType::SFromStack,
                2 => UpdateVarType::RightRangeFromStack,
                3 => UpdateVarType::RightRangeFromSStack,
                4 => UpdateVarType::RightRangeToS,
                5 => UpdateVarType::RightRangeInit,
                _ => UpdateVarType::KeepFromStackLast,
            };
            add_op(reg, OpCode::UpdateVar, OperationPayload::UpdateVar {
                var_type,
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
            compile_length_bag_node(bag, node.status, reg, env)
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
            // Check if this Alt has SUPER status (used by absent function)
            let is_super = node.has_status(ND_ST_SUPER);
            let push_opcode = if is_super { OpCode::PushSuper } else { OpCode::Push };

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

            // Pre-calculate branch lengths
            let mut branch_lens: Vec<i32> = Vec::with_capacity(n);
            for b in &branches {
                branch_lens.push(compile_length_tree(b, reg, env));
            }

            // Calculate total length to find goal position
            // Layout: for each branch i < n-1: PUSH + body_i + JUMP; last branch: body_{n-1}
            let mut total_len = 0i32;
            for i in 0..n {
                total_len += branch_lens[i];
                if i < n - 1 {
                    total_len += OPSIZE_PUSH + OPSIZE_JUMP;
                }
            }

            let goal = reg.ops.len() as i32 + total_len;

            for i in 0..n {
                if i < n - 1 {
                    // PUSH to next alternative (skip over body + JUMP)
                    let push_addr = SIZE_INC + branch_lens[i] + OPSIZE_JUMP;
                    add_op(reg, push_opcode, OperationPayload::Push { addr: push_addr });
                }

                let r = compile_tree(branches[i], reg, env);
                if r != 0 {
                    return r;
                }

                if i < n - 1 {
                    // JUMP to end of alternation (goal position)
                    let jump_addr = goal - reg.ops.len() as i32;
                    add_op(reg, OpCode::Jump, OperationPayload::Jump {
                        addr: jump_addr,
                    });
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

            if node.has_status(ND_ST_CHECKER) {
                // BackRef checker for conditionals: (?(1)then|else)
                let ns = refs.to_vec();
                add_op(reg, OpCode::BackRefCheck, OperationPayload::BackRefGeneral {
                    num: refs.len() as i32,
                    ns,
                    nest_level: br.nest_level,
                });
            } else if refs.len() == 1 {
                let n = refs[0];
                if node.has_status(ND_ST_IGNORECASE) {
                    add_op(reg, OpCode::BackRefNIc, OperationPayload::BackRefN { n1: n });
                } else {
                    match n {
                        1 => {
                            add_op(reg, OpCode::BackRef1, OperationPayload::None);
                        }
                        2 => {
                            add_op(reg, OpCode::BackRef2, OperationPayload::None);
                        }
                        _ => {
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
            compile_bag_node(bag, node.status, reg, env)
        }

        NodeInner::Anchor(an) => {
            compile_anchor_node(an, reg, env)
        }

        NodeInner::Gimmick(gn) => {
            compile_gimmick_node(gn, reg, env)
        }

        NodeInner::Call(call) => {
            // Look up the called_addr for this group
            let gnum = call.called_gnum as usize;
            let addr = if gnum < reg.called_addrs.len() && reg.called_addrs[gnum] >= 0 {
                reg.called_addrs[gnum]
            } else {
                0 // Will be patched later if not yet compiled
            };
            add_op(reg, OpCode::Call, OperationPayload::Call { addr });
            // Record for later patching if the group hasn't been compiled yet
            if gnum >= reg.called_addrs.len() || reg.called_addrs[gnum] < 0 {
                // Store the index of this OP_CALL for patching
                let call_idx = reg.ops.len() - 1;
                reg.unset_call_addrs.push((call_idx, gnum as i32));
            }
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

// ============================================================================
// tune_tree state flags (matching C's IN_* defines from regcomp.c:4481)
// ============================================================================
const IN_ALT: i32 = 1 << 0;
const IN_NOT: i32 = 1 << 1;
const IN_REAL_REPEAT: i32 = 1 << 2;
const IN_VAR_REPEAT: i32 = 1 << 3;
const IN_MULTI_ENTRY: i32 = 1 << 5;
const IN_PREC_READ: i32 = 1 << 6;

/// Calculate minimum byte length a node can match.
/// Mirrors C's node_min_byte_len() from regcomp.c.
fn node_min_byte_len(node: &Node, env: &ParseEnv) -> OnigLen {
    match &node.inner {
        NodeInner::String(sn) => sn.s.len() as OnigLen,

        NodeInner::CType(_) | NodeInner::CClass(_) => {
            env.enc.min_enc_len() as OnigLen
        }

        NodeInner::List(_) => {
            let mut len: OnigLen = 0;
            let mut cur = node;
            loop {
                if let NodeInner::List(cons) = &cur.inner {
                    let tmin = node_min_byte_len(&cons.car, env);
                    len = distance_add(len, tmin);
                    match &cons.cdr {
                        Some(next) => cur = next,
                        None => break,
                    }
                } else {
                    break;
                }
            }
            len
        }

        NodeInner::Alt(_) => {
            let mut len: OnigLen = 0;
            let mut first = true;
            let mut cur = node;
            loop {
                if let NodeInner::Alt(cons) = &cur.inner {
                    let tmin = node_min_byte_len(&cons.car, env);
                    if first {
                        len = tmin;
                        first = false;
                    } else if len > tmin {
                        len = tmin;
                    }
                    match &cons.cdr {
                        Some(next) => cur = next,
                        None => break,
                    }
                } else {
                    break;
                }
            }
            len
        }

        NodeInner::Quant(qn) => {
            if qn.lower > 0 {
                if let Some(ref body) = qn.body {
                    let len = node_min_byte_len(body, env);
                    distance_multiply(len, qn.lower)
                } else {
                    0
                }
            } else {
                0
            }
        }

        NodeInner::Bag(bn) => {
            match bn.bag_type {
                BagType::Option | BagType::StopBacktrack => {
                    if let Some(ref body) = bn.body {
                        node_min_byte_len(body, env)
                    } else {
                        0
                    }
                }
                BagType::Memory => {
                    if node.has_status(ND_ST_FIXED_MIN) {
                        bn.min_len
                    } else if node.has_status(ND_ST_MARK1) {
                        0 // recursive cycle
                    } else {
                        // Set MARK1 for cycle detection, compute, cache with FIXED_MIN
                        unsafe {
                            let node_ptr = node as *const Node as *mut Node;
                            (*node_ptr).status_add(ND_ST_MARK1);
                            let len = if let Some(ref body) = bn.body {
                                node_min_byte_len(body, env)
                            } else {
                                0
                            };
                            (*node_ptr).status_remove(ND_ST_MARK1);
                            if let NodeInner::Bag(ref mut bn_mut) = (*node_ptr).inner {
                                bn_mut.min_len = len;
                            }
                            (*node_ptr).status_add(ND_ST_FIXED_MIN);
                            len
                        }
                    }
                }
                BagType::IfElse => {
                    if let BagData::IfElse { ref then_node, ref else_node } = bn.bag_data {
                        let mut len = if let Some(ref body) = bn.body {
                            node_min_byte_len(body, env)
                        } else {
                            0
                        };
                        if let Some(ref then_n) = then_node {
                            len += node_min_byte_len(then_n, env);
                        }
                        let elen = if let Some(ref else_n) = else_node {
                            node_min_byte_len(else_n, env)
                        } else {
                            0
                        };
                        if elen < len { elen } else { len }
                    } else {
                        0
                    }
                }
            }
        }

        NodeInner::BackRef(br) => {
            if node.has_status(ND_ST_CHECKER) {
                0
            } else {
                // Simplified: return 0 for backrefs (safe minimum)
                0
            }
        }

        NodeInner::Call(ref cn) => {
            if !cn.target_node.is_null() {
                unsafe { node_min_byte_len(&*cn.target_node, env) }
            } else {
                0
            }
        }

        NodeInner::Anchor(_) | NodeInner::Gimmick(_) => 0,
    }
}

/// Check if a quantifier body contains capture groups (Memory bags).
/// Returns the appropriate emptiness type. Mirrors C's quantifiers_memory_node_info().
fn quantifiers_memory_node_info(node: &Node) -> BodyEmptyType {
    let mut r = BodyEmptyType::MayBeEmpty;

    match &node.inner {
        NodeInner::List(_) | NodeInner::Alt(_) => {
            let mut cur = node;
            loop {
                let (car, cdr) = match &cur.inner {
                    NodeInner::List(cons) => (&cons.car, &cons.cdr),
                    NodeInner::Alt(cons) => (&cons.car, &cons.cdr),
                    _ => break,
                };
                let v = quantifiers_memory_node_info(car);
                if v as i32 > r as i32 { r = v; }
                match cdr {
                    Some(next) => cur = next,
                    None => break,
                }
            }
        }
        NodeInner::Quant(qn) => {
            if qn.upper != 0 {
                if let Some(ref body) = qn.body {
                    r = quantifiers_memory_node_info(body);
                }
            }
        }
        NodeInner::Bag(bn) => {
            match bn.bag_type {
                BagType::Memory => {
                    return BodyEmptyType::MayBeEmptyMem;
                }
                BagType::Option | BagType::StopBacktrack => {
                    if let Some(ref body) = bn.body {
                        r = quantifiers_memory_node_info(body);
                    }
                }
                BagType::IfElse => {
                    if let Some(ref body) = bn.body {
                        r = quantifiers_memory_node_info(body);
                    }
                    if let BagData::IfElse { ref then_node, ref else_node } = bn.bag_data {
                        if let Some(ref then_n) = then_node {
                            let v = quantifiers_memory_node_info(then_n);
                            if v as i32 > r as i32 { r = v; }
                        }
                        if let Some(ref else_n) = else_node {
                            let v = quantifiers_memory_node_info(else_n);
                            if v as i32 > r as i32 { r = v; }
                        }
                    }
                }
            }
        }
        _ => {}
    }

    r
}

/// Expand a case-insensitive string node into CClass/List nodes.
/// Mirrors C's unravel_case_fold_string() from regcomp.c.
///
/// For each character in the string:
/// - If it has case-fold alternatives (e.g. 'c' -> 'C'), create a CClass node [cC]
/// - Otherwise, accumulate into a plain string node
/// - Combine all resulting nodes into a List
fn unravel_case_fold_string(node: &mut Node, reg: &mut RegexType, _state: i32) -> i32 {
    let enc = reg.enc;

    // Extract string bytes and clear ignorecase flag
    let s_bytes = if let NodeInner::String(ref sn) = node.inner {
        sn.s.clone()
    } else {
        return ONIG_NORMAL;
    };
    node.status_remove(ND_ST_IGNORECASE);

    let mut items = vec![OnigCaseFoldCodeItem { byte_len: 0, code_len: 0, code: [0; ONIGENC_MAX_COMP_CASE_FOLD_CODE_LEN] }; ONIGENC_GET_CASE_FOLD_CODES_MAX_NUM];

    let mut nodes: Vec<Box<Node>> = Vec::new();
    let mut pending: Vec<u8> = Vec::new(); // accumulate non-foldable chars

    let mut pos = 0;
    while pos < s_bytes.len() {
        let char_len = enc.mbc_enc_len(&s_bytes[pos..]);
        let n = enc.get_case_fold_codes_by_str(
            reg.case_fold_flag,
            &s_bytes[pos..],
            s_bytes.len(),
            &mut items,
        );

        if n > 0 {
            // Flush pending plain string
            if !pending.is_empty() {
                nodes.push(node_new_str(&pending));
                pending.clear();
            }

            // Check if all items are single-codepoint folds
            let all_single = (0..n as usize).all(|i| items[i].code_len == 1);

            if all_single {
                // All single-char: create CClass with original + alternatives
                let mut cc_node = node_new_cclass();
                let cc = cc_node.as_cclass_mut().unwrap();
                let code = enc.mbc_to_code(&s_bytes[pos..], s_bytes.len() - pos);
                crate::regparse::add_code_into_cc(cc, code, enc);
                for i in 0..(n as usize) {
                    crate::regparse::add_code_into_cc(cc, items[i].code[0], enc);
                }
                nodes.push(cc_node);
                pos += char_len;
            } else {
                // Multi-char folds present: create Alt with string alternatives
                // Determine how many bytes all items cover (should be uniform)
                let max_byte_len = (0..n as usize).map(|i| items[i].byte_len).max().unwrap_or(char_len as i32) as usize;

                // First alternative: original string bytes
                let orig_str = &s_bytes[pos..pos + max_byte_len];
                let mut alt_node: Box<Node> = node_new_alt(
                    node_new_str(orig_str),
                    None,
                );
                let mut curr = &mut alt_node;

                for i in 0..(n as usize) {
                    // Convert codepoints to string bytes
                    let item = &items[i];
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 6]; // max UTF-8 bytes per codepoint
                    for ci in 0..(item.code_len as usize) {
                        let blen = enc.code_to_mbc(item.code[ci], &mut tmp);
                        buf.extend_from_slice(&tmp[..blen as usize]);
                    }
                    let new_alt = node_new_alt(node_new_str(&buf), None);
                    // Append to chain
                    if let NodeInner::Alt(ref mut ca) = curr.inner {
                        ca.cdr = Some(new_alt);
                        curr = ca.cdr.as_mut().unwrap();
                    }
                }

                nodes.push(alt_node);
                pos += max_byte_len;
            }
        } else {
            // No case fold: accumulate into pending string
            pending.extend_from_slice(&s_bytes[pos..pos + char_len]);
            pos += char_len;
        }
    }

    // Flush any remaining pending string
    if !pending.is_empty() {
        nodes.push(node_new_str(&pending));
    }

    // Build result: single node or List
    if nodes.is_empty() {
        node.inner = NodeInner::String(StrNode { s: Vec::new(), flag: 0 });
    } else if nodes.len() == 1 {
        let n = nodes.pop().unwrap();
        *node = *n;
    } else {
        // Build List from right to left
        let mut list: Option<Box<Node>> = None;
        for n in nodes.into_iter().rev() {
            list = Some(node_new_list(n, list));
        }
        *node = *list.unwrap();
    }

    ONIG_NORMAL
}

// ============================================================================
// Lookbehind support: node_char_len, tune_look_behind, divide_look_behind_alternatives
// ============================================================================

/// Result of computing character length for a node subtree.
enum CharLenResult {
    Fixed(OnigLen),
    Variable(OnigLen, OnigLen),
}

/// Compute character count (not byte count) for a node subtree.
fn node_char_len(node: &Node, enc: OnigEncoding) -> CharLenResult {
    match &node.inner {
        NodeInner::String(sn) => {
            let n = onigenc_strlen(enc, &sn.s, 0, sn.s.len());
            CharLenResult::Fixed(n as OnigLen)
        }
        NodeInner::CType(_) | NodeInner::CClass(_) => CharLenResult::Fixed(1),
        NodeInner::List(_) => {
            let mut sum: OnigLen = 0;
            let mut variable = false;
            let mut min_sum: OnigLen = 0;
            let mut max_sum: OnigLen = 0;
            let mut cur = node;
            loop {
                if let NodeInner::List(cons) = &cur.inner {
                    match node_char_len(&cons.car, enc) {
                        CharLenResult::Fixed(n) => {
                            if variable {
                                min_sum = distance_add(min_sum, n);
                                max_sum = distance_add(max_sum, n);
                            } else {
                                sum = distance_add(sum, n);
                            }
                        }
                        CharLenResult::Variable(mn, mx) => {
                            if !variable {
                                min_sum = sum;
                                max_sum = sum;
                                variable = true;
                            }
                            min_sum = distance_add(min_sum, mn);
                            max_sum = distance_add(max_sum, mx);
                        }
                    }
                    match &cons.cdr {
                        Some(next) => cur = next,
                        None => break,
                    }
                } else {
                    break;
                }
            }
            if variable {
                CharLenResult::Variable(min_sum, max_sum)
            } else {
                CharLenResult::Fixed(sum)
            }
        }
        NodeInner::Alt(_) => {
            let mut min: OnigLen = OnigLen::MAX;
            let mut max: OnigLen = 0;
            let mut cur = node;
            loop {
                if let NodeInner::Alt(cons) = &cur.inner {
                    let (mn, mx) = match node_char_len(&cons.car, enc) {
                        CharLenResult::Fixed(n) => (n, n),
                        CharLenResult::Variable(mn, mx) => (mn, mx),
                    };
                    if mn < min { min = mn; }
                    if mx > max { max = mx; }
                    match &cons.cdr {
                        Some(next) => cur = next,
                        None => break,
                    }
                } else {
                    break;
                }
            }
            if min == max {
                CharLenResult::Fixed(min)
            } else {
                CharLenResult::Variable(min, max)
            }
        }
        NodeInner::Quant(qn) => {
            if let Some(ref body) = qn.body {
                match node_char_len(body, enc) {
                    CharLenResult::Fixed(n) => {
                        let lo = distance_multiply(n, qn.lower);
                        let hi = if qn.upper == INFINITE_REPEAT { INFINITE_LEN } else { distance_multiply(n, qn.upper) };
                        if lo == hi { CharLenResult::Fixed(lo) } else { CharLenResult::Variable(lo, hi) }
                    }
                    CharLenResult::Variable(mn, mx) => {
                        let lo = distance_multiply(mn, qn.lower);
                        let hi = if qn.upper == INFINITE_REPEAT { INFINITE_LEN } else { distance_multiply(mx, qn.upper) };
                        CharLenResult::Variable(lo, hi)
                    }
                }
            } else {
                CharLenResult::Fixed(0)
            }
        }
        NodeInner::Bag(bn) => {
            if let BagData::IfElse { ref then_node, ref else_node } = bn.bag_data {
                // Condition (body) may consume input (non-backref pattern conditions)
                // or be zero-width (backref checker conditions).
                let cond_len = if let Some(ref body) = bn.body {
                    if body.has_status(ND_ST_CHECKER) {
                        // Backref checker: zero length
                        (0 as OnigLen, 0 as OnigLen)
                    } else {
                        match node_char_len(body, enc) {
                            CharLenResult::Fixed(n) => (n, n),
                            CharLenResult::Variable(mn, mx) => (mn, mx),
                        }
                    }
                } else {
                    (0, 0)
                };
                let then_len = if let Some(ref n) = then_node {
                    match node_char_len(n, enc) {
                        CharLenResult::Fixed(n) => (n, n),
                        CharLenResult::Variable(mn, mx) => (mn, mx),
                    }
                } else {
                    (0, 0)
                };
                let else_len = if let Some(ref n) = else_node {
                    match node_char_len(n, enc) {
                        CharLenResult::Fixed(n) => (n, n),
                        CharLenResult::Variable(mn, mx) => (mn, mx),
                    }
                } else {
                    (0, 0)
                };
                // Success path: condition + then; Failure path: else
                let success_min = distance_add(cond_len.0, then_len.0);
                let success_max = distance_add(cond_len.1, then_len.1);
                let min = std::cmp::min(success_min, else_len.0);
                let max = std::cmp::max(success_max, else_len.1);
                if min == max {
                    CharLenResult::Fixed(min)
                } else {
                    CharLenResult::Variable(min, max)
                }
            } else if let Some(ref body) = bn.body {
                node_char_len(body, enc)
            } else {
                CharLenResult::Fixed(0)
            }
        }
        NodeInner::Anchor(_) => CharLenResult::Fixed(0),
        NodeInner::BackRef(_) => CharLenResult::Variable(0, INFINITE_LEN),
        _ => CharLenResult::Fixed(0),
    }
}

/// Divide variable-length lookbehind with Alt body into per-branch fixed-length lookbehinds.
/// For positive: Alt(Anchor(LB,a), Anchor(LB,b)) — any branch must match (OR).
/// For negative: List(Anchor(LB_NOT,a), Anchor(LB_NOT,b)) — all branches must pass (AND).
fn divide_look_behind_alt(node: &mut Node, anchor_type: i32, enc: OnigEncoding) -> i32 {
    // Extract anchor fields
    let (body, ascii_mode) = if let NodeInner::Anchor(ref mut an) = node.inner {
        (an.body.take().unwrap(), an.ascii_mode)
    } else {
        return ONIGERR_INVALID_LOOK_BEHIND_PATTERN;
    };

    // Collect all Alt branches
    let mut branches: Vec<Box<Node>> = Vec::new();
    let mut cur = body;
    loop {
        if let NodeInner::Alt(cons) = cur.inner {
            branches.push(cons.car);
            match cons.cdr {
                Some(next) => cur = next,
                None => break,
            }
        } else {
            branches.push(cur);
            break;
        }
    }

    let use_list = anchor_type == ANCR_LOOK_BEHIND_NOT;

    // Build new node tree of anchors, from last to first
    let mut result: Option<Box<Node>> = None;
    for branch in branches.into_iter().rev() {
        let char_len = match node_char_len(&branch, enc) {
            CharLenResult::Fixed(n) => n,
            CharLenResult::Variable(_, _) => return ONIGERR_INVALID_LOOK_BEHIND_PATTERN,
        };

        let mut anchor = node_new_anchor(anchor_type);
        if let NodeInner::Anchor(ref mut an) = anchor.inner {
            an.body = Some(branch);
            an.char_min_len = char_len;
            an.char_max_len = char_len;
            an.ascii_mode = ascii_mode;
        }

        if use_list {
            // Negative lookbehind: ALL branches must pass (List = AND)
            result = Some(node_new_list(anchor, result));
        } else {
            // Positive lookbehind: ANY branch must match (Alt = OR)
            result = Some(node_new_alt(anchor, result));
        }
    }

    // Replace the original node with the new tree
    if let Some(new_node) = result {
        *node = *new_node;
    }

    ONIG_NORMAL
}

/// Check if a node is an Alt where all top-level branches are individually fixed-length.
/// Returns true only if: node is Alt, and every branch has CharLenResult::Fixed.
fn is_alt_all_branches_fixed(node: &Node, enc: OnigEncoding) -> bool {
    let mut cur = node;
    loop {
        if let NodeInner::Alt(cons) = &cur.inner {
            match node_char_len(&cons.car, enc) {
                CharLenResult::Fixed(_) => {}
                CharLenResult::Variable(_, _) => return false,
            }
            match &cons.cdr {
                Some(next) => cur = next,
                None => return true,
            }
        } else {
            return false;
        }
    }
}

/// Check if a node tree contains absent stoppers (ND_ST_ABSENT_WITH_SIDE_EFFECTS).
/// Returns true if invalid nodes are found inside lookbehind.
/// C: check_node_in_look_behind (simplified — we only need the absent stopper check).
fn check_node_in_look_behind(node: &Node) -> bool {
    match &node.inner {
        NodeInner::List(cons) | NodeInner::Alt(cons) => {
            if check_node_in_look_behind(&cons.car) {
                return true;
            }
            if let Some(ref cdr) = cons.cdr {
                return check_node_in_look_behind(cdr);
            }
            false
        }
        NodeInner::Quant(qn) => {
            if let Some(ref body) = qn.body {
                check_node_in_look_behind(body)
            } else {
                false
            }
        }
        NodeInner::Bag(en) => {
            if let Some(ref body) = en.body {
                if check_node_in_look_behind(body) {
                    return true;
                }
            }
            if let BagData::IfElse { ref then_node, ref else_node } = en.bag_data {
                if let Some(ref tn) = then_node {
                    if check_node_in_look_behind(tn) {
                        return true;
                    }
                }
                if let Some(ref en) = else_node {
                    if check_node_in_look_behind(en) {
                        return true;
                    }
                }
            }
            false
        }
        NodeInner::Anchor(an) => {
            if let Some(ref body) = an.body {
                check_node_in_look_behind(body)
            } else {
                false
            }
        }
        NodeInner::Gimmick(_) => {
            node.has_status(ND_ST_ABSENT_WITH_SIDE_EFFECTS)
        }
        _ => false,
    }
}

/// Tune a lookbehind anchor: compute char lengths and split variable-length alternatives.
fn tune_look_behind(node: &mut Node, enc: OnigEncoding, syntax: &OnigSyntaxType) -> i32 {
    let (anchor_type, has_body) = if let NodeInner::Anchor(ref an) = node.inner {
        (an.anchor_type, an.body.is_some())
    } else {
        return 0;
    };

    if !has_body {
        return 0;
    }

    // Check for absent stoppers inside lookbehind (C: check_node_in_look_behind)
    {
        let body = if let NodeInner::Anchor(ref an) = node.inner {
            an.body.as_ref().unwrap()
        } else {
            return 0;
        };
        if check_node_in_look_behind(body) {
            return ONIGERR_INVALID_LOOK_BEHIND_PATTERN;
        }
    }

    let body_char_len = {
        let body = if let NodeInner::Anchor(ref an) = node.inner {
            an.body.as_ref().unwrap()
        } else {
            return 0;
        };
        node_char_len(body, enc)
    };

    // Overflow check (C: #177)
    const LOOK_BEHIND_MAX_CHAR_LEN: OnigLen = 65535;
    let (cmin, cmax) = match body_char_len {
        CharLenResult::Fixed(n) => (n, n),
        CharLenResult::Variable(mn, mx) => (mn, mx),
    };
    if (cmax != INFINITE_LEN && cmax > LOOK_BEHIND_MAX_CHAR_LEN)
        || cmin > LOOK_BEHIND_MAX_CHAR_LEN
    {
        return ONIGERR_INVALID_LOOK_BEHIND_PATTERN;
    }

    match body_char_len {
        CharLenResult::Fixed(len) => {
            if let NodeInner::Anchor(ref mut an) = node.inner {
                an.char_min_len = len;
                an.char_max_len = len;
            }
            ONIG_NORMAL
        }
        CharLenResult::Variable(min, max) => {
            // Check if body is Alt with all branches individually fixed-length
            // (C's CHAR_LEN_TOP_ALT_FIXED case)
            let top_alt_fixed = if let NodeInner::Anchor(ref an) = node.inner {
                if let Some(ref body) = an.body {
                    is_alt_all_branches_fixed(body, enc)
                } else {
                    false
                }
            } else {
                false
            };

            if top_alt_fixed {
                // All alt branches are fixed-length, just different sizes
                if is_syntax_bv(syntax, ONIG_SYN_DIFFERENT_LEN_ALT_LOOK_BEHIND) {
                    let r = divide_look_behind_alt(node, anchor_type, enc);
                    if r == ONIG_NORMAL {
                        return r;
                    }
                    // Should not fail here since we checked all branches are fixed
                }
                // Fall through to variable-length path
                if is_syntax_bv(syntax, ONIG_SYN_VARIABLE_LEN_LOOK_BEHIND) {
                    if min == INFINITE_LEN {
                        return ONIGERR_INVALID_LOOK_BEHIND_PATTERN;
                    }
                    if let NodeInner::Anchor(ref mut an) = node.inner {
                        an.char_min_len = min;
                        an.char_max_len = max;
                    }
                    ONIG_NORMAL
                } else {
                    ONIGERR_INVALID_LOOK_BEHIND_PATTERN
                }
            } else {
                // Either non-alt body, or alt with variable-length branches
                if !is_syntax_bv(syntax, ONIG_SYN_VARIABLE_LEN_LOOK_BEHIND) {
                    return ONIGERR_INVALID_LOOK_BEHIND_PATTERN;
                }
                if min == INFINITE_LEN {
                    return ONIGERR_INVALID_LOOK_BEHIND_PATTERN;
                }
                if let NodeInner::Anchor(ref mut an) = node.inner {
                    an.char_min_len = min;
                    an.char_max_len = max;
                }
                ONIG_NORMAL
            }
        }
    }
}

/// Resolve all \g<name>/\g<num> call references in the tree.
/// Sets called_gnum on Call nodes and marks target groups as CALLED.
fn resolve_call_references(node: &mut Node, reg: &mut RegexType, env: &mut ParseEnv) -> i32 {
    match &mut node.inner {
        NodeInner::Call(call) => {
            // Resolve the call target
            let mem_node_ptr;
            if call.by_number {
                let gnum = call.called_gnum;
                if gnum > env.num_mem || gnum < 0 {
                    return ONIGERR_UNDEFINED_GROUP_REFERENCE;
                }
                // Mark the target group as CALLED
                mem_node_ptr = env.mem_env(gnum as usize).mem_node;
                if !mem_node_ptr.is_null() {
                    unsafe { (*mem_node_ptr).status_add(ND_ST_CALLED); }
                }
            } else {
                // Named call - look up name
                let name = call.name.clone();
                if let Some(ref nt) = reg.name_table {
                    if let Some(nums) = nt.name_to_group_numbers(&name) {
                        if nums.len() != 1 {
                            return ONIGERR_MULTIPLEX_DEFINITION_NAME_CALL;
                        }
                        call.called_gnum = nums[0];
                        mem_node_ptr = env.mem_env(nums[0] as usize).mem_node;
                        if !mem_node_ptr.is_null() {
                            unsafe { (*mem_node_ptr).status_add(ND_ST_CALLED); }
                        }
                    } else {
                        return ONIGERR_UNDEFINED_NAME_REFERENCE;
                    }
                } else {
                    return ONIGERR_UNDEFINED_NAME_REFERENCE;
                }
            }
            // Link the call node to its target (so recursive_call_check can follow calls)
            // Note: we store the raw pointer as a non-owning reference (the target node
            // is owned by the tree, not by this call). We wrap it in Box without ownership.
            if !mem_node_ptr.is_null() {
                // Store target pointer for recursion detection (not owning)
                call.target_node = mem_node_ptr;
            }
            0
        }
        NodeInner::List(cons) | NodeInner::Alt(cons) => {
            let r = resolve_call_references(&mut cons.car, reg, env);
            if r != 0 { return r; }
            if let Some(ref mut cdr) = cons.cdr {
                resolve_call_references(cdr, reg, env)
            } else {
                0
            }
        }
        NodeInner::Quant(qn) => {
            if let Some(ref mut body) = qn.body {
                resolve_call_references(body, reg, env)
            } else {
                0
            }
        }
        NodeInner::Bag(bn) => {
            if let Some(ref mut body) = bn.body {
                let r = resolve_call_references(body, reg, env);
                if r != 0 { return r; }
            }
            if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                if let Some(ref mut then_n) = then_node {
                    let r = resolve_call_references(then_n, reg, env);
                    if r != 0 { return r; }
                }
                if let Some(ref mut else_n) = else_node {
                    let r = resolve_call_references(else_n, reg, env);
                    if r != 0 { return r; }
                }
            }
            0
        }
        NodeInner::Anchor(an) => {
            if let Some(ref mut body) = an.body {
                resolve_call_references(body, reg, env)
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// Check if a node subtree contains any CALLED memory group.
/// Returns true if a ND_ST_CALLED bag node is found.
/// Inner recursive call check — detects if a call path leads back to a MARK1'd node.
/// Mirrors C's recursive_call_check(). Returns nonzero if recursion found.
/// Uses unsafe raw pointers to work around borrow checker limitations with
/// node.status vs node.inner aliasing.
fn recursive_call_check_inner(node: &mut Node) -> i32 {
    let node_ptr = node as *mut Node;
    let node_type = node.node_type();

    match node_type {
        NodeType::List | NodeType::Alt => {
            let mut r = 0;
            let mut cur: *mut Node = node;
            unsafe {
                loop {
                    let (car, cdr) = match &mut (*cur).inner {
                        NodeInner::List(cons) | NodeInner::Alt(cons) => {
                            (&mut cons.car as *mut Box<Node>, &mut cons.cdr as *mut Option<Box<Node>>)
                        }
                        _ => break,
                    };
                    r |= recursive_call_check_inner(&mut *(*car));
                    match &mut *cdr {
                        Some(ref mut next) => cur = &mut **next,
                        None => break,
                    }
                }
            }
            r
        }
        NodeType::Anchor => {
            if let NodeInner::Anchor(ref mut an) = node.inner {
                if let Some(ref mut body) = an.body {
                    recursive_call_check_inner(body)
                } else { 0 }
            } else { 0 }
        }
        NodeType::Quant => {
            if let NodeInner::Quant(ref mut qn) = node.inner {
                if let Some(ref mut body) = qn.body {
                    recursive_call_check_inner(body)
                } else { 0 }
            } else { 0 }
        }
        NodeType::Call => {
            // Follow the call target via raw pointer
            unsafe {
                if let NodeInner::Call(ref cn) = (*node_ptr).inner {
                    let target = cn.target_node;
                    if !target.is_null() {
                        let r = recursive_call_check_inner(&mut *target);
                        if r != 0 && (*target).has_status(ND_ST_MARK1) {
                            (*node_ptr).status_add(ND_ST_RECURSION);
                        }
                        r
                    } else { 0 }
                } else { 0 }
            }
        }
        NodeType::Bag => {
            let is_memory = if let NodeInner::Bag(ref bn) = node.inner {
                bn.bag_type == BagType::Memory
            } else { false };

            if is_memory {
                // Use raw pointer to check/set status while inner is borrowed
                unsafe {
                    if (*node_ptr).has_status(ND_ST_MARK2) {
                        return 0;
                    } else if (*node_ptr).has_status(ND_ST_MARK1) {
                        return 1; // recursion found!
                    }
                    (*node_ptr).status_add(ND_ST_MARK2);
                    let r = if let NodeInner::Bag(ref mut bn) = (*node_ptr).inner {
                        if let Some(ref mut body) = bn.body {
                            recursive_call_check_inner(body)
                        } else { 0 }
                    } else { 0 };
                    (*node_ptr).status_remove(ND_ST_MARK2);
                    r
                }
            } else {
                unsafe {
                    if let NodeInner::Bag(ref mut bn) = (*node_ptr).inner {
                        if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                            let mut r = 0;
                            if let Some(ref mut t) = then_node { r |= recursive_call_check_inner(t); }
                            if let Some(ref mut e) = else_node { r |= recursive_call_check_inner(e); }
                            if let Some(ref mut body) = bn.body {
                                r |= recursive_call_check_inner(body);
                            }
                            r
                        } else {
                            if let Some(ref mut body) = bn.body {
                                recursive_call_check_inner(body)
                            } else { 0 }
                        }
                    } else { 0 }
                }
            }
        }
        _ => 0,
    }
}

const IN_RECURSION: i32 = 1;
const FOUND_CALLED_NODE: i32 = 1;

/// Outer traversal — walks tree, detects recursion in called groups, sets ND_ST_RECURSION.
/// Mirrors C's recursive_call_check_trav().
fn recursive_call_check_trav(node: &mut Node, env: &mut ParseEnv, state: i32) -> i32 {
    let node_ptr = node as *mut Node;
    let node_type = node.node_type();

    match node_type {
        NodeType::List | NodeType::Alt => {
            let mut r = 0;
            let mut cur: *mut Node = node;
            unsafe {
                loop {
                    let (car, cdr) = match &mut (*cur).inner {
                        NodeInner::List(cons) | NodeInner::Alt(cons) => {
                            (&mut cons.car as *mut Box<Node>, &mut cons.cdr as *mut Option<Box<Node>>)
                        }
                        _ => break,
                    };
                    let ret = recursive_call_check_trav(&mut *(*car), env, state);
                    if ret == FOUND_CALLED_NODE { r = FOUND_CALLED_NODE; }
                    match &mut *cdr {
                        Some(ref mut next) => cur = &mut **next,
                        None => break,
                    }
                }
            }
            r
        }
        NodeType::Quant => {
            unsafe {
                let upper = if let NodeInner::Quant(ref qn) = (*node_ptr).inner { qn.upper } else { 0 };
                if let NodeInner::Quant(ref mut qn) = (*node_ptr).inner {
                    if let Some(ref mut body) = qn.body {
                        let r = recursive_call_check_trav(body, env, state);
                        if upper == 0 && r == FOUND_CALLED_NODE {
                            qn.include_referred = 1;
                        }
                        r
                    } else { 0 }
                } else { 0 }
            }
        }
        NodeType::Anchor => {
            if let NodeInner::Anchor(ref mut an) = node.inner {
                if let Some(ref mut body) = an.body {
                    recursive_call_check_trav(body, env, state)
                } else { 0 }
            } else { 0 }
        }
        NodeType::Bag => {
            // Extract info before borrowing inner mutably
            let (is_memory, is_if_else, is_called, regnum) = unsafe {
                if let NodeInner::Bag(ref bn) = (*node_ptr).inner {
                    let is_mem = bn.bag_type == BagType::Memory;
                    let is_ie = matches!(bn.bag_data, BagData::IfElse { .. });
                    let regnum = match bn.bag_data {
                        BagData::Memory { regnum, .. } => regnum,
                        _ => 0,
                    };
                    (is_mem, is_ie, (*node_ptr).has_status(ND_ST_CALLED), regnum)
                } else {
                    return 0;
                }
            };

            let mut r = 0;

            if is_memory {
                if is_called {
                    r = FOUND_CALLED_NODE;
                }
                if is_called || (state & IN_RECURSION) != 0 {
                    unsafe {
                        if !(*node_ptr).has_status(ND_ST_RECURSION) {
                            (*node_ptr).status_add(ND_ST_MARK1);
                            if let NodeInner::Bag(ref mut bn) = (*node_ptr).inner {
                                if let Some(ref mut body) = bn.body {
                                    let ret = recursive_call_check_inner(body);
                                    if ret != 0 {
                                        (*node_ptr).status_add(ND_ST_RECURSION);
                                        env.backtrack_mem |= 1u32 << (regnum as u32);
                                    }
                                }
                            }
                            (*node_ptr).status_remove(ND_ST_MARK1);
                        }
                    }
                }
            }

            let mut state1 = state;
            unsafe {
                if (*node_ptr).has_status(ND_ST_RECURSION) {
                    state1 |= IN_RECURSION;
                }
            }

            unsafe {
                if let NodeInner::Bag(ref mut bn) = (*node_ptr).inner {
                    if let Some(ref mut body) = bn.body {
                        let ret = recursive_call_check_trav(body, env, state1);
                        if ret == FOUND_CALLED_NODE { r = FOUND_CALLED_NODE; }
                    }

                    if is_if_else {
                        if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                            if let Some(ref mut t) = then_node {
                                let ret = recursive_call_check_trav(t, env, state1);
                                if ret == FOUND_CALLED_NODE { r = FOUND_CALLED_NODE; }
                            }
                            if let Some(ref mut e) = else_node {
                                let ret = recursive_call_check_trav(e, env, state1);
                                if ret == FOUND_CALLED_NODE { r = FOUND_CALLED_NODE; }
                            }
                        }
                    }
                }
            }

            r
        }
        _ => 0,
    }
}

// ============================================================================
// disable_noname_group_capture — CAPTURE_ONLY_NAMED_GROUP support
// When syntax has this flag and named groups exist, unnamed groups become
// non-capturing and group numbers are renumbered to only include named groups.
// ============================================================================

/// Traverse tree: assign new sequential numbers to named groups, remove unnamed BAG_MEMORY.
/// Returns 1 when node was replaced (parent may need to reduce nested quantifiers).
fn make_named_capture_number_map(node: &mut Node, map: &mut [GroupNumMap], counter: &mut i32) -> i32 {
    let node_type = node.node_type();
    match node_type {
        NodeType::List | NodeType::Alt => {
            let cur = node as *mut Node;
            unsafe {
                let mut p = cur;
                loop {
                    let (car_ptr, cdr_opt) = match &mut (*p).inner {
                        NodeInner::List(ref mut cons) | NodeInner::Alt(ref mut cons) => {
                            let car = &mut *cons.car as *mut Node;
                            let cdr = cons.cdr.as_mut().map(|c| &mut **c as *mut Node);
                            (car, cdr)
                        }
                        _ => break,
                    };
                    let r = make_named_capture_number_map(&mut *car_ptr, map, counter);
                    if r < 0 { return r; }
                    match cdr_opt {
                        Some(next) => p = next,
                        None => break,
                    }
                }
            }
            0
        }
        NodeType::Quant => {
            let body_ptr: Option<*mut Node> = if let NodeInner::Quant(ref mut qn) = node.inner {
                qn.body.as_mut().map(|b| &mut **b as *mut Node)
            } else {
                None
            };
            if let Some(bp) = body_ptr {
                let r = unsafe { make_named_capture_number_map(&mut *bp, map, counter) };
                if r < 0 { return r; }
                // If node was replaced and became a quantifier, could reduce nested quantifiers
                // (rare case, skip for now like C's onig_reduce_nested_quantifier)
            }
            0
        }
        NodeType::Bag => {
            let is_memory = matches!(&node.inner, NodeInner::Bag(ref bn) if bn.bag_type == BagType::Memory);
            let is_named = node.has_status(ND_ST_NAMED_GROUP);

            if is_memory && !is_named {
                // Unnamed group — remove bag wrapper, replace node with its body
                let body = if let NodeInner::Bag(ref mut bn) = node.inner {
                    bn.body.take()
                } else {
                    None
                };
                if let Some(body) = body {
                    let body = *body;
                    node.inner = body.inner;
                    node.status = body.status;
                } else {
                    node.inner = NodeInner::String(StrNode { s: Vec::new(), flag: 0 });
                }
                let r = make_named_capture_number_map(node, map, counter);
                if r < 0 { return r; }
                return 1;
            }

            if is_memory && is_named {
                if let NodeInner::Bag(ref mut bn) = node.inner {
                    *counter += 1;
                    if let BagData::Memory { ref mut regnum, .. } = bn.bag_data {
                        map[*regnum as usize].new_val = *counter;
                        *regnum = *counter;
                    }
                    if let Some(ref mut body) = bn.body {
                        let r = make_named_capture_number_map(body, map, counter);
                        if r < 0 { return r; }
                    }
                }
                return 0;
            }

            // IfElse or other bag types
            unsafe {
                let node_ptr = node as *mut Node;
                if let NodeInner::Bag(ref mut bn) = (*node_ptr).inner {
                    if bn.bag_type == BagType::IfElse {
                        if let Some(ref mut body) = bn.body {
                            let r = make_named_capture_number_map(body, map, counter);
                            if r < 0 { return r; }
                        }
                        if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                            if let Some(ref mut tn) = then_node {
                                let r = make_named_capture_number_map(tn, map, counter);
                                if r < 0 { return r; }
                            }
                            if let Some(ref mut en) = else_node {
                                let r = make_named_capture_number_map(en, map, counter);
                                if r < 0 { return r; }
                            }
                        }
                    } else {
                        if let Some(ref mut body) = bn.body {
                            let r = make_named_capture_number_map(body, map, counter);
                            if r < 0 { return r; }
                        }
                    }
                }
            }
            0
        }
        NodeType::Anchor => {
            if let NodeInner::Anchor(ref mut a) = node.inner {
                if let Some(ref mut body) = a.body {
                    let r = make_named_capture_number_map(body, map, counter);
                    if r < 0 { return r; }
                }
            }
            0
        }
        _ => 0,
    }
}

/// Renumber backrefs in a single backref node using the group number map.
fn renumber_backref_node(node: &mut Node, map: &[GroupNumMap]) -> i32 {
    if !node.has_status(ND_ST_BY_NAME) {
        return ONIGERR_NUMBERED_BACKREF_OR_CALL_NOT_ALLOWED;
    }
    if let NodeInner::BackRef(ref mut br) = node.inner {
        let old_num = br.back_num as usize;
        let mut pos = 0usize;
        if let Some(ref mut dyn_refs) = br.back_dynamic {
            for i in 0..old_num {
                let n = map[dyn_refs[i] as usize].new_val;
                if n > 0 {
                    dyn_refs[pos] = n;
                    pos += 1;
                }
            }
        } else {
            for i in 0..old_num {
                let n = map[br.back_static[i] as usize].new_val;
                if n > 0 {
                    br.back_static[pos] = n;
                    pos += 1;
                }
            }
        }
        br.back_num = pos as i32;
    }
    0
}

/// Traverse tree to renumber all backrefs using the group number map.
fn renumber_backref_traverse(node: &mut Node, map: &[GroupNumMap]) -> i32 {
    match node.node_type() {
        NodeType::List | NodeType::Alt => {
            let cur = node as *mut Node;
            unsafe {
                let mut p = cur;
                loop {
                    let (car_ptr, cdr_opt) = match &mut (*p).inner {
                        NodeInner::List(ref mut cons) | NodeInner::Alt(ref mut cons) => {
                            let car = &mut *cons.car as *mut Node;
                            let cdr = cons.cdr.as_mut().map(|c| &mut **c as *mut Node);
                            (car, cdr)
                        }
                        _ => break,
                    };
                    let r = renumber_backref_traverse(&mut *car_ptr, map);
                    if r != 0 { return r; }
                    match cdr_opt {
                        Some(next) => p = next,
                        None => break,
                    }
                }
            }
            0
        }
        NodeType::Quant => {
            if let NodeInner::Quant(ref mut qn) = node.inner {
                if let Some(ref mut body) = qn.body {
                    return renumber_backref_traverse(body, map);
                }
            }
            0
        }
        NodeType::Bag => {
            unsafe {
                let node_ptr = node as *mut Node;
                if let NodeInner::Bag(ref mut bn) = (*node_ptr).inner {
                    if let Some(ref mut body) = bn.body {
                        let r = renumber_backref_traverse(body, map);
                        if r != 0 { return r; }
                    }
                    if bn.bag_type == BagType::IfElse {
                        if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                            if let Some(ref mut tn) = then_node {
                                let r = renumber_backref_traverse(tn, map);
                                if r != 0 { return r; }
                            }
                            if let Some(ref mut en) = else_node {
                                let r = renumber_backref_traverse(en, map);
                                if r != 0 { return r; }
                            }
                        }
                    }
                }
            }
            0
        }
        NodeType::BackRef => {
            renumber_backref_node(node, map)
        }
        NodeType::Anchor => {
            if let NodeInner::Anchor(ref mut a) = node.inner {
                if let Some(ref mut body) = a.body {
                    return renumber_backref_traverse(body, map);
                }
            }
            0
        }
        _ => 0,
    }
}

/// Check that no numbered (non-named) backrefs exist in the tree.
/// Called when all captures are named (num_named == num_mem).
fn numbered_ref_check(node: &Node) -> i32 {
    match &node.inner {
        NodeInner::List(cons) | NodeInner::Alt(cons) => {
            let r = numbered_ref_check(&cons.car);
            if r != 0 { return r; }
            if let Some(ref next) = cons.cdr {
                return numbered_ref_check(next);
            }
            0
        }
        NodeInner::Quant(ref qn) => {
            if let Some(ref body) = qn.body {
                numbered_ref_check(body)
            } else { 0 }
        }
        NodeInner::Anchor(ref a) => {
            if let Some(ref body) = a.body {
                numbered_ref_check(body)
            } else { 0 }
        }
        NodeInner::Bag(ref bn) => {
            if let Some(ref body) = bn.body {
                let r = numbered_ref_check(body);
                if r != 0 { return r; }
            }
            if bn.bag_type == BagType::IfElse {
                if let BagData::IfElse { ref then_node, ref else_node } = bn.bag_data {
                    if let Some(ref tn) = then_node {
                        let r = numbered_ref_check(tn);
                        if r != 0 { return r; }
                    }
                    if let Some(ref en) = else_node {
                        let r = numbered_ref_check(en);
                        if r != 0 { return r; }
                    }
                }
            }
            0
        }
        NodeInner::BackRef(_) => {
            if !node.has_status(ND_ST_BY_NAME) {
                ONIGERR_NUMBERED_BACKREF_OR_CALL_NOT_ALLOWED
            } else { 0 }
        }
        _ => 0,
    }
}

/// When CAPTURE_ONLY_NAMED_GROUP is active and both named and unnamed groups
/// exist, remove unnamed captures and renumber everything to only use named groups.
fn disable_noname_group_capture(root: &mut Node, reg: &mut RegexType, env: &mut ParseEnv) -> i32 {
    let num_mem = env.num_mem as usize;
    let mut map: Vec<GroupNumMap> = (0..=num_mem).map(|_| GroupNumMap { new_val: 0 }).collect();
    let mut counter: i32 = 0;

    let r = make_named_capture_number_map(root, &mut map, &mut counter);
    if r < 0 { return r; }

    let r = renumber_backref_traverse(root, &map);
    if r != 0 { return r; }

    // Compact mem_env: shift named entries down to fill gaps left by removed unnamed groups
    let mut pos: usize = 1;
    for i in 1..=num_mem {
        if map[i].new_val > 0 {
            if pos != i {
                let src_node = env.mem_env(i).mem_node;
                let src_empty = env.mem_env(i).empty_repeat_node;
                let dst = env.mem_env_mut(pos);
                dst.mem_node = src_node;
                dst.empty_repeat_node = src_empty;
            }
            pos += 1;
        }
    }

    // Update cap_history bitmap with renumbered groups
    let loc = env.cap_history;
    env.cap_history = 0;
    for i in 1..=std::cmp::min(num_mem, 31) {
        if (loc & (1u32 << i)) != 0 {
            let new_val = map[i].new_val;
            if new_val > 0 && new_val <= 31 {
                env.cap_history |= 1u32 << (new_val as u32);
            }
        }
    }

    env.num_mem = env.num_named;
    reg.num_mem = env.num_named;

    // Renumber name table entries
    if let Some(ref mut nt) = reg.name_table {
        for entry in nt.entries.values_mut() {
            for back_ref in entry.back_refs.iter_mut() {
                let idx = *back_ref as usize;
                if idx < map.len() {
                    *back_ref = map[idx].new_val;
                }
            }
        }
    }

    0
}

// ============================================================================
// infinite_recursive_call_check — detect never-ending recursion
// Patterns like (?<abc>\g<abc>) or (()(?(2)\g<1>)) have no terminating path.
// ============================================================================

const RECURSION_EXIST: i32 = 1 << 0;
const RECURSION_MUST: i32 = 1 << 1;
const RECURSION_INFINITE: i32 = 1 << 2;

/// Analyze a node tree for infinite recursion.
/// `head` indicates whether we are still at the "head" position (no non-empty prefix consumed).
fn infinite_recursive_call_check(node: &mut Node, env: &ParseEnv, head: i32) -> i32 {
    let mut r: i32 = 0;
    match node.node_type() {
        NodeType::List => {
            let mut head = head;
            let cur = node as *mut Node;
            unsafe {
                let mut p = cur;
                loop {
                    match &mut (*p).inner {
                        NodeInner::List(ref mut cons) => {
                            let ret = infinite_recursive_call_check(&mut cons.car, env, head);
                            if ret < 0 || (ret & RECURSION_INFINITE) != 0 { return ret; }
                            r |= ret;
                            if head != 0 {
                                let min = node_min_byte_len(&cons.car, env);
                                if min != 0 { head = 0; }
                            }
                            match cons.cdr {
                                Some(ref mut next) => p = &mut **next,
                                None => break,
                            }
                        }
                        _ => break,
                    }
                }
            }
        }
        NodeType::Alt => {
            let mut must = RECURSION_MUST;
            let cur = node as *mut Node;
            unsafe {
                let mut p = cur;
                loop {
                    match &mut (*p).inner {
                        NodeInner::Alt(ref mut cons) => {
                            let ret = infinite_recursive_call_check(&mut cons.car, env, head);
                            if ret < 0 || (ret & RECURSION_INFINITE) != 0 { return ret; }
                            r |= ret & RECURSION_EXIST;
                            must &= ret;
                            match cons.cdr {
                                Some(ref mut next) => p = &mut **next,
                                None => break,
                            }
                        }
                        _ => break,
                    }
                }
            }
            r |= must;
        }
        NodeType::Quant => {
            if let NodeInner::Quant(ref mut qn) = node.inner {
                if qn.upper == 0 { return 0; }
                if let Some(ref mut body) = qn.body {
                    r = infinite_recursive_call_check(body, env, head);
                    if r < 0 { return r; }
                    if (r & RECURSION_MUST) != 0 && qn.lower == 0 {
                        r &= !RECURSION_MUST;
                    }
                }
            }
        }
        NodeType::Anchor => {
            if let NodeInner::Anchor(ref mut a) = node.inner {
                if let Some(ref mut body) = a.body {
                    r = infinite_recursive_call_check(body, env, head);
                }
            }
        }
        NodeType::Call => {
            // Follow call to its target (the BAG_MEMORY node it references)
            if let NodeInner::Call(ref cn) = node.inner {
                if !cn.target_node.is_null() {
                    r = unsafe { infinite_recursive_call_check(&mut *cn.target_node, env, head) };
                }
            }
        }
        NodeType::Bag => {
            let bag_type = if let NodeInner::Bag(ref bn) = node.inner { bn.bag_type } else { BagType::Option };
            match bag_type {
                BagType::Memory => {
                    if node.has_status(ND_ST_MARK2) {
                        return 0;
                    } else if node.has_status(ND_ST_MARK1) {
                        // Recursion back to a marked node
                        return if head == 0 {
                            RECURSION_EXIST | RECURSION_MUST
                        } else {
                            RECURSION_EXIST | RECURSION_MUST | RECURSION_INFINITE
                        };
                    } else {
                        node.status_add(ND_ST_MARK2);
                        if let NodeInner::Bag(ref mut bn) = node.inner {
                            if let Some(ref mut body) = bn.body {
                                r = infinite_recursive_call_check(body, env, head);
                            }
                        }
                        node.status_remove(ND_ST_MARK2);
                    }
                }
                BagType::IfElse => {
                    unsafe {
                        let node_ptr = node as *mut Node;
                        if let NodeInner::Bag(ref mut bn) = (*node_ptr).inner {
                            // Check condition (body)
                            if let Some(ref mut body) = bn.body {
                                let ret = infinite_recursive_call_check(body, env, head);
                                if ret < 0 || (ret & RECURSION_INFINITE) != 0 { return ret; }
                                r |= ret;
                            }
                            if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                                // Check then branch
                                if let Some(ref mut tn) = then_node {
                                    let min = if head != 0 {
                                        if let Some(ref body) = bn.body {
                                            node_min_byte_len(body, env)
                                        } else { 0 }
                                    } else { 0 };
                                    let ret = infinite_recursive_call_check(tn, env, if min != 0 { 0 } else { head });
                                    if ret < 0 || (ret & RECURSION_INFINITE) != 0 { return ret; }
                                    r |= ret;
                                }
                                // Check else branch
                                if let Some(ref mut en) = else_node {
                                    let eret = infinite_recursive_call_check(en, env, head);
                                    if eret < 0 || (eret & RECURSION_INFINITE) != 0 { return eret; }
                                    r |= eret & RECURSION_EXIST;
                                    if (eret & RECURSION_MUST) == 0 {
                                        r &= !RECURSION_MUST;
                                    }
                                } else {
                                    r &= !RECURSION_MUST;
                                }
                            }
                        }
                    }
                }
                _ => {
                    if let NodeInner::Bag(ref mut bn) = node.inner {
                        if let Some(ref mut body) = bn.body {
                            r = infinite_recursive_call_check(body, env, head);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    r
}

/// Traverse tree to find recursive call definitions, check each for infinite recursion.
fn infinite_recursive_call_check_trav(node: &mut Node, env: &ParseEnv) -> i32 {
    let r;
    match node.node_type() {
        NodeType::List | NodeType::Alt => {
            let cur = node as *mut Node;
            unsafe {
                let mut p = cur;
                loop {
                    match &mut (*p).inner {
                        NodeInner::List(ref mut cons) | NodeInner::Alt(ref mut cons) => {
                            let ret = infinite_recursive_call_check_trav(&mut cons.car, env);
                            if ret != 0 { return ret; }
                            match cons.cdr {
                                Some(ref mut next) => p = &mut **next,
                                None => break,
                            }
                        }
                        _ => break,
                    }
                }
            }
            r = 0;
        }
        NodeType::Anchor => {
            if let NodeInner::Anchor(ref mut a) = node.inner {
                if let Some(ref mut body) = a.body {
                    return infinite_recursive_call_check_trav(body, env);
                }
            }
            r = 0;
        }
        NodeType::Quant => {
            if let NodeInner::Quant(ref mut qn) = node.inner {
                if let Some(ref mut body) = qn.body {
                    return infinite_recursive_call_check_trav(body, env);
                }
            }
            r = 0;
        }
        NodeType::Bag => {
            let bag_type = if let NodeInner::Bag(ref bn) = node.inner { bn.bag_type } else { BagType::Option };
            if bag_type == BagType::Memory {
                if node.has_status(ND_ST_RECURSION) && node.has_status(ND_ST_CALLED) {
                    node.status_add(ND_ST_MARK1);
                    let ret = if let NodeInner::Bag(ref mut bn) = node.inner {
                        if let Some(ref mut body) = bn.body {
                            infinite_recursive_call_check(body, env, 1)
                        } else { 0 }
                    } else { 0 };
                    if ret < 0 { return ret; }
                    if (ret & (RECURSION_MUST | RECURSION_INFINITE)) != 0 {
                        return ONIGERR_NEVER_ENDING_RECURSION;
                    }
                    node.status_remove(ND_ST_MARK1);
                }
            }
            if bag_type == BagType::IfElse {
                unsafe {
                    let node_ptr = node as *mut Node;
                    if let NodeInner::Bag(ref mut bn) = (*node_ptr).inner {
                        if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                            if let Some(ref mut tn) = then_node {
                                let ret = infinite_recursive_call_check_trav(tn, env);
                                if ret != 0 { return ret; }
                            }
                            if let Some(ref mut en) = else_node {
                                let ret = infinite_recursive_call_check_trav(en, env);
                                if ret != 0 { return ret; }
                            }
                        }
                    }
                }
            }
            // Also recurse into body for all bag types
            if let NodeInner::Bag(ref mut bn) = node.inner {
                if let Some(ref mut body) = bn.body {
                    return infinite_recursive_call_check_trav(body, env);
                }
            }
            r = 0;
        }
        _ => { r = 0; }
    }
    r
}

/// Tree tuning pass - sets emptiness on quantifier nodes and propagates state.
/// Mirrors C's tune_tree() from regcomp.c.
pub fn tune_tree(node: &mut Node, reg: &mut RegexType, state: i32, env: &mut ParseEnv) -> i32 {
    // Case-fold expansion: before the main match to get full &mut Node access
    if let NodeInner::String(ref sn) = node.inner {
        if node.has_status(ND_ST_IGNORECASE) && !sn.is_crude() {
            let r = unravel_case_fold_string(node, reg, state);
            if r != 0 { return r; }
            // After expansion, the node may have changed type (CClass, List, etc.)
            // Recurse to tune the expanded tree
            return tune_tree(node, reg, state, env);
        }
    }

    match &mut node.inner {
        NodeInner::List(_) => {
            // Walk the list iteratively to avoid borrow issues
            let mut cur: *mut Node = node;
            unsafe {
                loop {
                    if let NodeInner::List(ref mut cons) = (*cur).inner {
                        let r = tune_tree(&mut cons.car, reg, state, env);
                        if r != 0 { return r; }
                        match cons.cdr {
                            Some(ref mut next) => cur = &mut **next,
                            None => break,
                        }
                    } else {
                        break;
                    }
                }
            }
            0
        }

        NodeInner::Alt(_) => {
            let mut cur: *mut Node = node;
            unsafe {
                loop {
                    if let NodeInner::Alt(ref mut cons) = (*cur).inner {
                        let r = tune_tree(&mut cons.car, reg, state | IN_ALT, env);
                        if r != 0 { return r; }
                        match cons.cdr {
                            Some(ref mut next) => cur = &mut **next,
                            None => break,
                        }
                    } else {
                        break;
                    }
                }
            }
            0
        }

        NodeInner::Quant(ref mut qn) => {
            // Propagate repeat status flags
            if (state & IN_REAL_REPEAT) != 0 {
                node.status |= ND_ST_IN_REAL_REPEAT;
            }
            if (state & IN_MULTI_ENTRY) != 0 {
                node.status |= ND_ST_IN_MULTI_ENTRY;
            }

            // Check if body can match empty
            if is_infinite_repeat(qn.upper) || qn.upper >= 1 {
                if let Some(ref body) = qn.body {
                    let d = node_min_byte_len(body, env);
                    if d == 0 {
                        // Use quantifiers_memory_node_info to detect captures in body
                        qn.emptiness = quantifiers_memory_node_info(body);
                    }
                }
            }

            // Update state for recursive call
            let mut new_state = state;
            if is_infinite_repeat(qn.upper) || qn.upper >= 2 {
                new_state |= IN_REAL_REPEAT;
            }
            if qn.lower != qn.upper {
                new_state |= IN_VAR_REPEAT;
            }

            // Recurse into body
            if let Some(ref mut body) = qn.body {
                let r = tune_tree(body, reg, new_state, env);
                if r != 0 { return r; }
            }

            0
        }

        NodeInner::Bag(ref mut bn) => {
            match bn.bag_type {
                BagType::Option => {
                    let saved_options = reg.options;
                    if let BagData::Option { options } = bn.bag_data {
                        reg.options = options;
                    }
                    let r = if let Some(ref mut body) = bn.body {
                        tune_tree(body, reg, state, env)
                    } else {
                        0
                    };
                    reg.options = saved_options;
                    r
                }
                BagType::Memory => {
                    if (state & (IN_ALT | IN_NOT | IN_VAR_REPEAT | IN_MULTI_ENTRY)) != 0 {
                        // Backtrack mem needed for captures in alternation/variable repeat
                        if let BagData::Memory { regnum, .. } = bn.bag_data {
                            mem_status_on(&mut env.backtrack_mem, regnum as usize);
                        }
                    }
                    if let Some(ref mut body) = bn.body {
                        tune_tree(body, reg, state, env)
                    } else {
                        0
                    }
                }
                BagType::StopBacktrack => {
                    if let Some(ref mut body) = bn.body {
                        tune_tree(body, reg, state, env)
                    } else {
                        0
                    }
                }
                BagType::IfElse => {
                    if let Some(ref mut body) = bn.body {
                        let r = tune_tree(body, reg, state | IN_ALT, env);
                        if r != 0 { return r; }
                    }
                    if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                        if let Some(ref mut then_n) = then_node {
                            let r = tune_tree(then_n, reg, state | IN_ALT, env);
                            if r != 0 { return r; }
                        }
                        if let Some(ref mut else_n) = else_node {
                            let r = tune_tree(else_n, reg, state | IN_ALT, env);
                            if r != 0 { return r; }
                        }
                    }
                    0
                }
            }
        }

        NodeInner::Anchor(ref mut an) => {
            let at = an.anchor_type;
            // For lookbehind anchors, compute char lengths (may transform node into Alt)
            if at == ANCR_LOOK_BEHIND || at == ANCR_LOOK_BEHIND_NOT {
                let enc = env.enc;
                let r = tune_look_behind(node, enc, env.syntax);
                if r != 0 { return r; }
                // tune_look_behind may have transformed node into an Alt;
                // if so, recurse on the new node structure
                if !matches!(node.inner, NodeInner::Anchor(_)) {
                    return tune_tree(node, reg, state, env);
                }
            }
            // Now recurse into the body
            if let NodeInner::Anchor(ref mut an) = node.inner {
                if let Some(ref mut body) = an.body {
                    let new_state = if an.anchor_type == ANCR_PREC_READ {
                        state | IN_PREC_READ
                    } else if an.anchor_type == ANCR_PREC_READ_NOT {
                        state | IN_PREC_READ | IN_NOT
                    } else {
                        state
                    };
                    tune_tree(body, reg, new_state, env)
                } else {
                    0
                }
            } else {
                0
            }
        }

        NodeInner::BackRef(ref br) => {
            // Set backrefed_mem for each referenced group
            for &back in br.back_refs() {
                if back > 0 {
                    mem_status_on(&mut env.backrefed_mem, back as usize);
                }
            }
            0
        }

        // Terminal nodes - nothing to tune
        NodeInner::String(_) | NodeInner::CType(_) | NodeInner::CClass(_)
        | NodeInner::Call(_) | NodeInner::Gimmick(_) => 0,
    }
}

// ============================================================================
// setup_empty_status_mem: compute qn.empty_status_mem for quantifiers
// ============================================================================

/// Pass 1: For each quantifier with emptiness >= MayBeEmptyMem, set
/// empty_repeat_node on all captures in its body.
fn mark_empty_repeat_node(node: &mut Node, env: &mut ParseEnv) {
    let node_ptr = node as *mut Node;
    match &mut node.inner {
        NodeInner::Quant(ref mut qn) => {
            let is_empty = qn.emptiness == BodyEmptyType::MayBeEmptyMem
                || qn.emptiness == BodyEmptyType::MayBeEmptyRec;
            if is_empty {
                if let Some(ref body) = qn.body {
                    set_empty_repeat_node_in_body(body, node_ptr as *const Node, env);
                }
            }
            if let Some(ref mut body) = qn.body {
                mark_empty_repeat_node(body, env);
            }
        }
        NodeInner::List(_) | NodeInner::Alt(_) => {
            let mut cur: *mut Node = node;
            unsafe {
                loop {
                    let (car, cdr) = match &mut (*cur).inner {
                        NodeInner::List(cons) | NodeInner::Alt(cons) => {
                            (&mut cons.car as *mut Box<Node>, &mut cons.cdr as *mut Option<Box<Node>>)
                        }
                        _ => break,
                    };
                    mark_empty_repeat_node(&mut *(*car), env);
                    match &mut *cdr {
                        Some(ref mut next) => cur = &mut **next,
                        None => break,
                    }
                }
            }
        }
        NodeInner::Bag(ref mut bn) => {
            if let Some(ref mut body) = bn.body {
                mark_empty_repeat_node(body, env);
            }
            if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                if let Some(ref mut t) = then_node { mark_empty_repeat_node(t, env); }
                if let Some(ref mut e) = else_node { mark_empty_repeat_node(e, env); }
            }
        }
        NodeInner::Anchor(ref mut an) => {
            if let Some(ref mut body) = an.body {
                mark_empty_repeat_node(body, env);
            }
        }
        _ => {}
    }
}

/// Helper: set empty_repeat_node for all BAG_MEMORY nodes in `node`.
fn set_empty_repeat_node_in_body(node: &Node, quant_ptr: *const Node, env: &mut ParseEnv) {
    match &node.inner {
        NodeInner::Bag(bn) => {
            if bn.bag_type == BagType::Memory {
                if let BagData::Memory { regnum, .. } = bn.bag_data {
                    let regnum = regnum as usize;
                    let entry = env.mem_env_mut(regnum);
                    entry.empty_repeat_node = quant_ptr as *mut Node;
                }
            }
            if let Some(ref body) = bn.body {
                set_empty_repeat_node_in_body(body, quant_ptr, env);
            }
            if let BagData::IfElse { ref then_node, ref else_node } = bn.bag_data {
                if let Some(ref t) = then_node { set_empty_repeat_node_in_body(t, quant_ptr, env); }
                if let Some(ref e) = else_node { set_empty_repeat_node_in_body(e, quant_ptr, env); }
            }
        }
        NodeInner::List(_) | NodeInner::Alt(_) => {
            let mut cur: &Node = node;
            loop {
                let (car, cdr) = match &cur.inner {
                    NodeInner::List(cons) | NodeInner::Alt(cons) => (&cons.car, &cons.cdr),
                    _ => break,
                };
                set_empty_repeat_node_in_body(car, quant_ptr, env);
                match cdr {
                    Some(ref next) => cur = next,
                    None => break,
                }
            }
        }
        NodeInner::Quant(qn) => {
            if let Some(ref body) = qn.body {
                set_empty_repeat_node_in_body(body, quant_ptr, env);
            }
        }
        NodeInner::Anchor(an) => {
            if let Some(ref body) = an.body {
                set_empty_repeat_node_in_body(body, quant_ptr, env);
            }
        }
        _ => {}
    }
}

/// Pass 2: Walk tree with a stack of enclosing empty-quantifier pointers.
/// When a backref is found, check if its target's empty_repeat_node is NOT
/// in the enclosing stack → set empty_status_mem on that quantifier.
fn resolve_empty_status_backrefs(
    node: &mut Node,
    enclosing_quants: &mut Vec<*const Node>,
    env: &ParseEnv,
) {
    let node_ptr = node as *const Node;
    match &mut node.inner {
        NodeInner::Quant(ref mut qn) => {
            let is_empty_quant = qn.emptiness == BodyEmptyType::MayBeEmptyMem
                || qn.emptiness == BodyEmptyType::MayBeEmptyRec;
            if is_empty_quant {
                enclosing_quants.push(node_ptr);
            }
            if let Some(ref mut body) = qn.body {
                resolve_empty_status_backrefs(body, enclosing_quants, env);
            }
            if is_empty_quant {
                enclosing_quants.pop();
            }
        }
        NodeInner::BackRef(ref br) => {
            for &back in br.back_refs() {
                if back <= 0 { continue; }
                let back = back as usize;
                let entry = env.mem_env(back);
                let er_node = entry.empty_repeat_node;
                if !er_node.is_null() {
                    // Check if the backref is inside the quantifier
                    if !enclosing_quants.contains(&(er_node as *const Node)) {
                        // Backref is OUTSIDE the quantifier → set empty_status_mem
                        unsafe {
                            if let NodeInner::Quant(ref mut qn) = (*er_node).inner {
                                qn.empty_status_mem |= 1u32 << back;
                                (*er_node).status |= ND_ST_EMPTY_STATUS_CHECK;
                            }
                        }
                    }
                }
            }
        }
        NodeInner::List(_) | NodeInner::Alt(_) => {
            let mut cur: *mut Node = node;
            unsafe {
                loop {
                    let (car, cdr) = match &mut (*cur).inner {
                        NodeInner::List(cons) | NodeInner::Alt(cons) => {
                            (&mut cons.car as *mut Box<Node>, &mut cons.cdr as *mut Option<Box<Node>>)
                        }
                        _ => break,
                    };
                    resolve_empty_status_backrefs(&mut *(*car), enclosing_quants, env);
                    match &mut *cdr {
                        Some(ref mut next) => cur = &mut **next,
                        None => break,
                    }
                }
            }
        }
        NodeInner::Bag(ref mut bn) => {
            if let Some(ref mut body) = bn.body {
                resolve_empty_status_backrefs(body, enclosing_quants, env);
            }
            if let BagData::IfElse { ref mut then_node, ref mut else_node } = bn.bag_data {
                if let Some(ref mut t) = then_node { resolve_empty_status_backrefs(t, enclosing_quants, env); }
                if let Some(ref mut e) = else_node { resolve_empty_status_backrefs(e, enclosing_quants, env); }
            }
        }
        NodeInner::Anchor(ref mut an) => {
            if let Some(ref mut body) = an.body {
                resolve_empty_status_backrefs(body, enclosing_quants, env);
            }
        }
        _ => {}
    }
}

/// Compute qn.empty_status_mem for all quantifiers in the tree.
fn setup_empty_status_mem(root: &mut Node, env: &mut ParseEnv) {
    // Pass 1: mark empty_repeat_node on captures inside empty quantifiers
    mark_empty_repeat_node(root, env);
    // Pass 2: resolve backrefs to set empty_status_mem
    let mut enclosing = Vec::new();
    resolve_empty_status_backrefs(root, &mut enclosing, env);
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
            let saved_status = node.status; // preserve flags like ND_ST_SUPER
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

            // Rebuild alt chain, preserving the original root status (e.g. ND_ST_SUPER)
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
            result.status = saved_status;
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

    // CAPTURE_ONLY_NAMED_GROUP: when named groups exist, disable unnamed captures
    if env.num_named > 0
        && is_syntax_bv(env.syntax, ONIG_SYN_CAPTURE_ONLY_NAMED_GROUP)
        && !opton_capture_group(reg.options)
    {
        let r = if env.num_named != env.num_mem {
            disable_noname_group_capture(&mut root, reg, &mut env)
        } else {
            numbered_ref_check(&root)
        };
        if r != 0 {
            return r;
        }
    }

    // Optimize: consolidate adjacent string nodes (mirrors C's reduce_string_list)
    let r = reduce_string_list(&mut root, reg.enc);
    if r != 0 {
        return r;
    }

    // Resolve subroutine call references before tune_tree
    if env.num_call > 0 {
        let r = resolve_call_references(&mut root, reg, &mut env);
        if r != 0 {
            return r;
        }
        // Detect recursion and set ND_ST_RECURSION on recursive capture groups
        recursive_call_check_trav(&mut root, &mut env, 0);
        // Check for never-ending recursion (e.g. (?<abc>\g<abc>))
        let r = infinite_recursive_call_check_trav(&mut root, &env);
        if r != 0 {
            return r;
        }
    }

    // Tune tree: detect empty loops, propagate state (mirrors C's tune_tree)
    let r = tune_tree(&mut root, reg, 0, &mut env);
    if r != 0 {
        return r;
    }

    // Compute empty_status_mem for quantifiers (determines EmptyCheckEnd vs EmptyCheckEndMemst)
    setup_empty_status_mem(&mut root, &mut env);

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

    // Initialize mark/save ID counter from parse env to avoid collisions
    // (C uses ID_ENTRY(env, id) which shares env->id_num between parser and compiler)
    reg.num_call = env.id_num;

    // Compile the tree to bytecode
    let r = compile_tree(&root, reg, &env);
    if r != 0 {
        return r;
    }

    // Patch unresolved subroutine call addresses
    if !reg.unset_call_addrs.is_empty() {
        for &(op_idx, gnum) in &reg.unset_call_addrs.clone() {
            let gnum = gnum as usize;
            if gnum < reg.called_addrs.len() && reg.called_addrs[gnum] >= 0 {
                let addr = reg.called_addrs[gnum];
                reg.ops[op_idx].payload = OperationPayload::Call { addr };
            }
        }
    }

    // Emit UPDATE_VAR(KeepFromStackLast) before OP_END if \K was used
    if env.keep_num > 0 {
        add_op(reg, OpCode::UpdateVar, OperationPayload::UpdateVar {
            var_type: UpdateVarType::KeepFromStackLast,
            id: 0,
            clear: false,
        });
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
        called_addrs: vec![],
        unset_call_addrs: vec![],
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
            called_addrs: vec![],
        unset_call_addrs: vec![],
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
        // a{2,5} is compiled via greedy expansion: body*2 + 3*(PUSH+body)
        let reg = parse_and_compile(b"a{2,5}").unwrap();
        let has_push = reg.ops.iter().any(|op| op.opcode == OpCode::Push);
        assert!(has_push, "expected Push for a{{2,5}} greedy expansion");
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

    #[test]
    fn test_never_ending_recursion_direct() {
        let mut reg = make_test_context().0;
        let r = onig_compile(&mut reg, b"(?<abc>\\g<abc>)");
        assert_eq!(r, ONIGERR_NEVER_ENDING_RECURSION);
    }

    #[test]
    fn test_never_ending_recursion_conditional() {
        let mut reg = make_test_context().0;
        let r = onig_compile(&mut reg, b"(()(?(2)\\g<1>))");
        assert_eq!(r, ONIGERR_NEVER_ENDING_RECURSION);
    }
}
