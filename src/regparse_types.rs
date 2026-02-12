// regparse_types.rs - Port of regparse.h
// AST node types, token types, parse environment, and supporting structures.

#![allow(non_upper_case_globals)]

use std::collections::HashMap;

use crate::oniguruma::*;
use crate::regenc::OnigEncoding;
use crate::regint::*;

// === Node Type Enum ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeType {
    String = 0,
    CClass = 1,
    CType = 2,
    BackRef = 3,
    Quant = 4,
    Bag = 5,
    Anchor = 6,
    List = 7,
    Alt = 8,
    Call = 9,
    Gimmick = 10,
}

#[inline]
pub fn nd_type2bit(t: NodeType) -> u32 {
    1 << (t as u32)
}

pub const ND_BIT_STRING: u32 = 1 << 0;
pub const ND_BIT_CCLASS: u32 = 1 << 1;
pub const ND_BIT_CTYPE: u32 = 1 << 2;
pub const ND_BIT_BACKREF: u32 = 1 << 3;
pub const ND_BIT_QUANT: u32 = 1 << 4;
pub const ND_BIT_BAG: u32 = 1 << 5;
pub const ND_BIT_ANCHOR: u32 = 1 << 6;
pub const ND_BIT_LIST: u32 = 1 << 7;
pub const ND_BIT_ALT: u32 = 1 << 8;
pub const ND_BIT_CALL: u32 = 1 << 9;
pub const ND_BIT_GIMMICK: u32 = 1 << 10;

// === Bag Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum BagType {
    Memory = 0,
    Option = 1,
    StopBacktrack = 2,
    IfElse = 3,
}

// === Gimmick Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum GimmickType {
    Fail = 0,
    Save = 1,
    UpdateVar = 2,
    Callout = 3,
}

// === Body Empty Type ===
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum BodyEmptyType {
    NotEmpty = 0,
    MayBeEmpty = 1,
    MayBeEmptyMem = 2,
    MayBeEmptyRec = 3,
}

// === Size Constants ===
pub const ND_STRING_MARGIN: usize = 16;
pub const ND_STRING_BUF_SIZE: usize = 24;
pub const ND_BACKREFS_SIZE: usize = 6;
pub const PARSEENV_MEMENV_SIZE: usize = 8;
pub const CTYPE_ANYCHAR: i32 = -1;

// === Node Status Flags ===
pub const ND_ST_FIXED_MIN: u32 = 1 << 0;
pub const ND_ST_FIXED_MAX: u32 = 1 << 1;
pub const ND_ST_FIXED_CLEN: u32 = 1 << 2;
pub const ND_ST_MARK1: u32 = 1 << 3;
pub const ND_ST_MARK2: u32 = 1 << 4;
pub const ND_ST_STRICT_REAL_REPEAT: u32 = 1 << 5;
pub const ND_ST_RECURSION: u32 = 1 << 6;
pub const ND_ST_CALLED: u32 = 1 << 7;
pub const ND_ST_FIXED_ADDR: u32 = 1 << 8;
pub const ND_ST_NAMED_GROUP: u32 = 1 << 9;
pub const ND_ST_IN_REAL_REPEAT: u32 = 1 << 10;
pub const ND_ST_IN_ZERO_REPEAT: u32 = 1 << 11;
pub const ND_ST_IN_MULTI_ENTRY: u32 = 1 << 12;
pub const ND_ST_NEST_LEVEL: u32 = 1 << 13;
pub const ND_ST_BY_NUMBER: u32 = 1 << 14;
pub const ND_ST_BY_NAME: u32 = 1 << 15;
pub const ND_ST_BACKREF: u32 = 1 << 16;
pub const ND_ST_CHECKER: u32 = 1 << 17;
pub const ND_ST_PROHIBIT_RECURSION: u32 = 1 << 18;
pub const ND_ST_SUPER: u32 = 1 << 19;
pub const ND_ST_EMPTY_STATUS_CHECK: u32 = 1 << 20;
pub const ND_ST_IGNORECASE: u32 = 1 << 21;
pub const ND_ST_MULTILINE: u32 = 1 << 22;
pub const ND_ST_TEXT_SEGMENT_WORD: u32 = 1 << 23;
pub const ND_ST_ABSENT_WITH_SIDE_EFFECTS: u32 = 1 << 24;
pub const ND_ST_FIXED_CLEN_MIN_SURE: u32 = 1 << 25;
pub const ND_ST_REFERENCED: u32 = 1 << 26;
pub const ND_ST_INPEEK: u32 = 1 << 27;
pub const ND_ST_WHOLE_OPTIONS: u32 = 1 << 28;

// === String Node Flags ===
pub const ND_STRING_CRUDE: u32 = 1 << 0;
pub const ND_STRING_CASE_EXPANDED: u32 = 1 << 1;

// === Memory Called States ===
pub const CALL_DELTA_UNREFERENCED: i32 = 0;
pub const CALL_DELTA_REFERENCED: i32 = 1;
pub const CALL_DELTA_CALLED: i32 = 2;

// === Parse Environment Flags ===
pub const PE_FLAG_HAS_CALL_ZERO: u32 = 1 << 0;
pub const PE_FLAG_HAS_WHOLE_OPTIONS: u32 = 1 << 1;
pub const PE_FLAG_HAS_ABSENT_STOPPER: u32 = 1 << 2;

// === BBuf (Byte Buffer) ===
// In C: struct { UChar* p; unsigned int used; unsigned int alloc; }
// In Rust: Vec<u8> handles allocation automatically.

#[derive(Clone, Debug)]
pub struct BBuf {
    pub data: Vec<u8>,
}

impl BBuf {
    pub fn new() -> Self {
        BBuf { data: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        BBuf { data: Vec::with_capacity(cap) }
    }

    pub fn used(&self) -> usize {
        self.data.len()
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    pub fn write_u32(&mut self, val: u32) {
        self.data.extend_from_slice(&val.to_ne_bytes());
    }

    pub fn clone_from(other: &BBuf) -> Self {
        BBuf { data: other.data.clone() }
    }
}

impl Default for BBuf {
    fn default() -> Self {
        Self::new()
    }
}

// === AST Node ===
// C uses a union with common base fields (node_type, status, parent, body).
// Rust: outer struct for common fields + inner enum for variant data.

pub struct Node {
    pub status: u32,
    pub parent: *mut Node,
    pub inner: NodeInner,
}

pub enum NodeInner {
    String(StrNode),
    CClass(CClassNode),
    CType(CtypeNode),
    BackRef(BackRefNode),
    Quant(QuantNode),
    Bag(BagNode),
    Anchor(AnchorNode),
    List(ConsAltNode),
    Alt(ConsAltNode),
    Call(CallNode),
    Gimmick(GimmickNode),
}

// === Node Helper Methods ===

impl Node {
    pub fn node_type(&self) -> NodeType {
        match &self.inner {
            NodeInner::String(_) => NodeType::String,
            NodeInner::CClass(_) => NodeType::CClass,
            NodeInner::CType(_) => NodeType::CType,
            NodeInner::BackRef(_) => NodeType::BackRef,
            NodeInner::Quant(_) => NodeType::Quant,
            NodeInner::Bag(_) => NodeType::Bag,
            NodeInner::Anchor(_) => NodeType::Anchor,
            NodeInner::List(_) => NodeType::List,
            NodeInner::Alt(_) => NodeType::Alt,
            NodeInner::Call(_) => NodeType::Call,
            NodeInner::Gimmick(_) => NodeType::Gimmick,
        }
    }

    pub fn node_type_bit(&self) -> u32 {
        nd_type2bit(self.node_type())
    }

    // Status helpers (matching C macros ND_STATUS_ADD, ND_STATUS_REMOVE)
    pub fn status_add(&mut self, flag: u32) {
        self.status |= flag;
    }

    pub fn status_remove(&mut self, flag: u32) {
        self.status &= !flag;
    }

    pub fn has_status(&self, flag: u32) -> bool {
        (self.status & flag) != 0
    }

    // Body access (only Quant, Bag, Anchor, Call have body)
    pub fn body(&self) -> Option<&Node> {
        match &self.inner {
            NodeInner::Quant(n) => n.body.as_ref().map(|b| b.as_ref()),
            NodeInner::Bag(n) => n.body.as_ref().map(|b| b.as_ref()),
            NodeInner::Anchor(n) => n.body.as_ref().map(|b| b.as_ref()),
            NodeInner::Call(n) => n.body.as_ref().map(|b| b.as_ref()),
            _ => None,
        }
    }

    pub fn is_anychar(&self) -> bool {
        if let NodeInner::CType(ct) = &self.inner {
            ct.ctype == CTYPE_ANYCHAR
        } else { false }
    }

    pub fn body_mut(&mut self) -> Option<&mut Node> {
        match &mut self.inner {
            NodeInner::Quant(n) => n.body.as_mut().map(|b| b.as_mut()),
            NodeInner::Bag(n) => n.body.as_mut().map(|b| b.as_mut()),
            NodeInner::Anchor(n) => n.body.as_mut().map(|b| b.as_mut()),
            NodeInner::Call(n) => n.body.as_mut().map(|b| b.as_mut()),
            _ => None,
        }
    }

    pub fn set_body(&mut self, body: Option<Box<Node>>) {
        match &mut self.inner {
            NodeInner::Quant(n) => n.body = body,
            NodeInner::Bag(n) => n.body = body,
            NodeInner::Anchor(n) => n.body = body,
            NodeInner::Call(n) => n.body = body,
            _ => {}
        }
    }

    pub fn take_body(&mut self) -> Option<Box<Node>> {
        match &mut self.inner {
            NodeInner::Quant(n) => n.body.take(),
            NodeInner::Bag(n) => n.body.take(),
            NodeInner::Anchor(n) => n.body.take(),
            NodeInner::Call(n) => n.body.take(),
            _ => None,
        }
    }

    // Variant accessors (matching C macros STR_, CCLASS_, etc.)
    pub fn as_str(&self) -> Option<&StrNode> {
        match &self.inner {
            NodeInner::String(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_str_mut(&mut self) -> Option<&mut StrNode> {
        match &mut self.inner {
            NodeInner::String(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_cclass(&self) -> Option<&CClassNode> {
        match &self.inner {
            NodeInner::CClass(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_cclass_mut(&mut self) -> Option<&mut CClassNode> {
        match &mut self.inner {
            NodeInner::CClass(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_ctype(&self) -> Option<&CtypeNode> {
        match &self.inner {
            NodeInner::CType(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_quant(&self) -> Option<&QuantNode> {
        match &self.inner {
            NodeInner::Quant(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_quant_mut(&mut self) -> Option<&mut QuantNode> {
        match &mut self.inner {
            NodeInner::Quant(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_bag(&self) -> Option<&BagNode> {
        match &self.inner {
            NodeInner::Bag(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_bag_mut(&mut self) -> Option<&mut BagNode> {
        match &mut self.inner {
            NodeInner::Bag(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_anchor(&self) -> Option<&AnchorNode> {
        match &self.inner {
            NodeInner::Anchor(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_anchor_mut(&mut self) -> Option<&mut AnchorNode> {
        match &mut self.inner {
            NodeInner::Anchor(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_backref(&self) -> Option<&BackRefNode> {
        match &self.inner {
            NodeInner::BackRef(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_backref_mut(&mut self) -> Option<&mut BackRefNode> {
        match &mut self.inner {
            NodeInner::BackRef(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_cons(&self) -> Option<&ConsAltNode> {
        match &self.inner {
            NodeInner::List(n) | NodeInner::Alt(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_cons_mut(&mut self) -> Option<&mut ConsAltNode> {
        match &mut self.inner {
            NodeInner::List(n) | NodeInner::Alt(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_call(&self) -> Option<&CallNode> {
        match &self.inner {
            NodeInner::Call(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_call_mut(&mut self) -> Option<&mut CallNode> {
        match &mut self.inner {
            NodeInner::Call(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_gimmick(&self) -> Option<&GimmickNode> {
        match &self.inner {
            NodeInner::Gimmick(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_gimmick_mut(&mut self) -> Option<&mut GimmickNode> {
        match &mut self.inner {
            NodeInner::Gimmick(n) => Some(n),
            _ => None,
        }
    }

    // ConsAltNode shortcuts (matching C macros ND_CAR, ND_CDR)
    pub fn car(&self) -> Option<&Node> {
        self.as_cons().map(|c| c.car.as_ref())
    }

    pub fn cdr(&self) -> Option<&Node> {
        self.as_cons().and_then(|c| c.cdr.as_ref().map(|b| b.as_ref()))
    }
}

// Allow sending Node across threads (raw pointers require manual impl)
unsafe impl Send for Node {}
unsafe impl Sync for Node {}

// === Node Variant Structs ===

pub struct StrNode {
    pub s: Vec<u8>,
    pub flag: u32,
}

impl StrNode {
    pub fn is_crude(&self) -> bool {
        (self.flag & ND_STRING_CRUDE) != 0
    }

    pub fn is_case_expanded(&self) -> bool {
        (self.flag & ND_STRING_CASE_EXPANDED) != 0
    }

    pub fn set_crude(&mut self) {
        self.flag |= ND_STRING_CRUDE;
    }

    pub fn clear_crude(&mut self) {
        self.flag &= !ND_STRING_CRUDE;
    }
}

pub struct CClassNode {
    pub flags: u32,
    pub bs: BitSet,
    pub mbuf: Option<BBuf>,
}

impl CClassNode {
    pub fn is_not(&self) -> bool {
        (self.flags & FLAG_NCCLASS_NOT) != 0
    }

    pub fn set_not(&mut self) {
        self.flags |= FLAG_NCCLASS_NOT;
    }

    pub fn clear_not(&mut self) {
        self.flags &= !FLAG_NCCLASS_NOT;
    }

    pub fn is_share(&self) -> bool {
        (self.flags & FLAG_NCCLASS_SHARE) != 0
    }
}

pub struct CtypeNode {
    pub ctype: i32,
    pub not: bool,
    pub ascii_mode: bool,
}

pub struct BackRefNode {
    pub back_num: i32,
    pub back_static: [i32; ND_BACKREFS_SIZE],
    pub back_dynamic: Option<Vec<i32>>,
    pub nest_level: i32,
}

impl BackRefNode {
    /// Get the backreference number array
    pub fn back_refs(&self) -> &[i32] {
        if let Some(ref dyn_refs) = self.back_dynamic {
            dyn_refs
        } else {
            &self.back_static[..self.back_num as usize]
        }
    }
}

pub struct QuantNode {
    pub body: Option<Box<Node>>,
    pub lower: i32,
    pub upper: i32,
    pub greedy: bool,
    pub emptiness: BodyEmptyType,
    pub head_exact: Option<u8>,
    pub next_head_exact: Option<u8>,
    pub include_referred: i32,
    pub empty_status_mem: MemStatusType,
}

pub struct BagNode {
    pub body: Option<Box<Node>>,
    pub bag_type: BagType,
    pub bag_data: BagData,
    pub min_len: OnigLen,
    pub max_len: OnigLen,
    pub min_char_len: OnigLen,
    pub max_char_len: OnigLen,
    pub opt_count: i32,
}

pub enum BagData {
    Memory {
        regnum: i32,
        called_addr: AbsAddrType,
        entry_count: i32,
        called_state: i32,
    },
    Option {
        options: OnigOptionType,
    },
    StopBacktrack,
    IfElse {
        then_node: Option<Box<Node>>,
        else_node: Option<Box<Node>>,
    },
}

impl BagNode {
    pub fn as_memory(&self) -> Option<(i32, AbsAddrType, i32, i32)> {
        match &self.bag_data {
            BagData::Memory { regnum, called_addr, entry_count, called_state } => {
                Some((*regnum, *called_addr, *entry_count, *called_state))
            }
            _ => None,
        }
    }

    pub fn regnum(&self) -> i32 {
        match &self.bag_data {
            BagData::Memory { regnum, .. } => *regnum,
            _ => 0,
        }
    }
}

pub struct AnchorNode {
    pub body: Option<Box<Node>>,
    pub anchor_type: i32,
    pub char_min_len: OnigLen,
    pub char_max_len: OnigLen,
    pub ascii_mode: bool,
    pub lead_node: Option<Box<Node>>,
}

pub struct ConsAltNode {
    pub car: Box<Node>,
    pub cdr: Option<Box<Node>>,
}

pub struct CallNode {
    pub body: Option<Box<Node>>,
    pub by_number: bool,
    pub called_gnum: i32,
    pub name: Vec<u8>,
    pub entry_count: i32,
    /// Raw pointer to the target BAG_MEMORY node (non-owning, for recursion detection)
    pub target_node: *mut Node,
}

pub struct GimmickNode {
    pub gimmick_type: GimmickType,
    pub detail_type: i32,
    pub num: i32,
    pub id: i32,
}

// === Token Types ===

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TokenType {
    Eot = 0,
    CrudeByte = 1,
    Char = 2,
    String = 3,
    CodePoint = 4,
    AnyChar = 5,
    CharType = 6,
    Backref = 7,
    Call = 8,
    Anchor = 9,
    Repeat = 10,
    Interval = 11,
    AnyCharAnytime = 12,
    Alt = 13,
    SubexpOpen = 14,
    SubexpClose = 15,
    OpenCC = 16,
    QuoteOpen = 17,
    CharProperty = 18,
    Keep = 19,
    GeneralNewline = 20,
    NoNewline = 21,
    TrueAnychar = 22,
    TextSegment = 23,
    // In character class context
    CcClose = 24,
    CcRange = 25,
    CcPosixBracketOpen = 26,
    CcAnd = 27,
    CcOpenCC = 28,
}

// === PToken (Parser Token) ===
// Flat struct matching C union approach: fields are only valid when token_type matches.
// This avoids needing to destructure an enum on every access (the C freely writes to
// different union members as it determines the token type).

pub struct PToken {
    pub token_type: TokenType,
    pub escaped: bool,
    pub code_point_continue: bool,
    pub base_num: i32,
    pub backp: usize,
    // Union field: code / byte (valid for TK_CHAR, TK_CODE_POINT, TK_CRUDE_BYTE)
    pub code: OnigCodePoint,
    // Union field: anchor / subtype (valid for TK_ANCHOR)
    pub anchor: i32,
    // Union field: repeat (valid for TK_REPEAT, TK_INTERVAL)
    pub repeat_lower: i32,
    pub repeat_upper: i32,
    pub repeat_greedy: bool,
    pub repeat_possessive: bool,
    // Union field: backref (valid for TK_BACKREF)
    pub backref_num: i32,
    pub backref_ref1: i32,
    pub backref_refs: Vec<i32>,
    pub backref_by_name: bool,
    pub backref_exist_level: bool,
    pub backref_level: i32,
    // Union field: call (valid for TK_CALL)
    pub call_name_start: usize,
    pub call_name_end: usize,
    pub call_gnum: i32,
    pub call_by_number: bool,
    // Union field: prop (valid for TK_CHAR_TYPE, TK_CHAR_PROPERTY)
    pub prop_ctype: i32,
    pub prop_not: bool,
    pub prop_braces: bool,
}

impl PToken {
    pub fn new() -> Self {
        PToken {
            token_type: TokenType::Eot,
            escaped: false,
            code_point_continue: false,
            base_num: 0,
            backp: 0,
            code: 0,
            anchor: 0,
            repeat_lower: 0,
            repeat_upper: 0,
            repeat_greedy: false,
            repeat_possessive: false,
            backref_num: 0,
            backref_ref1: 0,
            backref_refs: Vec::new(),
            backref_by_name: false,
            backref_exist_level: false,
            backref_level: 0,
            call_name_start: 0,
            call_name_end: 0,
            call_gnum: 0,
            call_by_number: false,
            prop_ctype: 0,
            prop_not: false,
            prop_braces: false,
        }
    }

    pub fn init(&mut self) {
        self.code_point_continue = false;
    }
}

impl Default for PToken {
    fn default() -> Self {
        Self::new()
    }
}

// === Group Number Map ===

pub struct GroupNumMap {
    pub new_val: i32,
}

// === Memory Environment ===

pub struct MemEnv {
    pub mem_node: *mut Node,
    pub empty_repeat_node: *mut Node,
}

impl Default for MemEnv {
    fn default() -> Self {
        MemEnv {
            mem_node: std::ptr::null_mut(),
            empty_repeat_node: std::ptr::null_mut(),
        }
    }
}

// Safety: MemEnv contains raw pointers that are only used within the parser
unsafe impl Send for MemEnv {}
unsafe impl Sync for MemEnv {}

// === Save Item ===

pub struct SaveItem {
    pub save_type: SaveType,
}

// === Unset Address (for USE_CALL) ===

pub struct UnsetAddr {
    pub offset: i32,
    pub target: *mut Node,
}

// Safety: UnsetAddr contains raw pointers used within the parser
unsafe impl Send for UnsetAddr {}
unsafe impl Sync for UnsetAddr {}

// === Parse Environment (ScanEnv in C) ===

pub struct ParseEnv {
    pub options: OnigOptionType,
    pub case_fold_flag: OnigCaseFoldType,
    pub enc: OnigEncoding,
    pub syntax: &'static OnigSyntaxType,
    pub cap_history: MemStatusType,
    pub backtrack_mem: MemStatusType,
    pub backrefed_mem: MemStatusType,
    pub pattern: *const u8,
    pub pattern_end: *const u8,
    pub error: *const u8,
    pub error_end: *const u8,
    pub reg: *mut RegexType,
    pub num_call: i32,
    pub num_mem: i32,
    pub num_named: i32,
    pub mem_alloc: i32,
    pub mem_env_static: [MemEnv; PARSEENV_MEMENV_SIZE],
    pub mem_env_dynamic: Option<Vec<MemEnv>>,
    pub backref_num: i32,
    pub keep_num: i32,
    pub id_num: i32,
    pub save_alloc_num: i32,
    pub saves: Option<Vec<SaveItem>>,
    pub unset_addr_list: Option<Vec<UnsetAddr>>,
    pub parse_depth: u32,
    pub flags: u32,
}

// Safety: ParseEnv contains raw pointers used within the parser scope
unsafe impl Send for ParseEnv {}
unsafe impl Sync for ParseEnv {}

// === Node Creation Helper Functions ===

pub fn node_new(inner: NodeInner) -> Box<Node> {
    Box::new(Node {
        status: 0,
        parent: std::ptr::null_mut(),
        inner,
    })
}

pub fn node_new_str(s: &[u8]) -> Box<Node> {
    node_new(NodeInner::String(StrNode {
        s: s.to_vec(),
        flag: 0,
    }))
}

pub fn node_new_str_crude(s: &[u8]) -> Box<Node> {
    node_new(NodeInner::String(StrNode {
        s: s.to_vec(),
        flag: ND_STRING_CRUDE,
    }))
}

pub fn node_new_empty() -> Box<Node> {
    node_new(NodeInner::String(StrNode {
        s: Vec::new(),
        flag: 0,
    }))
}

pub fn node_new_cclass() -> Box<Node> {
    node_new(NodeInner::CClass(CClassNode {
        flags: 0,
        bs: [0; BITSET_REAL_SIZE],
        mbuf: None,
    }))
}

pub fn node_new_ctype(ctype: i32, not: bool, ascii_mode: bool) -> Box<Node> {
    node_new(NodeInner::CType(CtypeNode {
        ctype,
        not,
        ascii_mode,
    }))
}

pub fn node_new_anychar() -> Box<Node> {
    node_new_ctype(CTYPE_ANYCHAR, false, false)
}

pub fn node_new_backref(
    back_num: i32,
    backrefs: &[i32],
    by_name: bool,
    nest_level: i32,
) -> Box<Node> {
    let mut back_static = [0i32; ND_BACKREFS_SIZE];
    let back_dynamic = if backrefs.len() <= ND_BACKREFS_SIZE {
        for (i, &r) in backrefs.iter().enumerate() {
            back_static[i] = r;
        }
        None
    } else {
        Some(backrefs.to_vec())
    };

    let mut node = node_new(NodeInner::BackRef(BackRefNode {
        back_num,
        back_static,
        back_dynamic,
        nest_level,
    }));
    if by_name {
        node.status_add(ND_ST_BY_NAME);
    }
    if nest_level != 0 {
        node.status_add(ND_ST_NEST_LEVEL);
    }
    node
}

pub fn node_new_quantifier(
    lower: i32,
    upper: i32,
    greedy: bool,
) -> Box<Node> {
    node_new(NodeInner::Quant(QuantNode {
        body: None,
        lower,
        upper,
        greedy,
        emptiness: BodyEmptyType::NotEmpty,
        head_exact: None,
        next_head_exact: None,
        include_referred: 0,
        empty_status_mem: 0,
    }))
}

pub fn node_new_bag(bag_type: BagType) -> Box<Node> {
    let bag_data = match bag_type {
        BagType::Memory => BagData::Memory {
            regnum: 0,
            called_addr: -1,
            entry_count: 0,
            called_state: 0,
        },
        BagType::Option => BagData::Option {
            options: ONIG_OPTION_NONE,
        },
        BagType::StopBacktrack => BagData::StopBacktrack,
        BagType::IfElse => BagData::IfElse {
            then_node: None,
            else_node: None,
        },
    };

    node_new(NodeInner::Bag(BagNode {
        body: None,
        bag_type,
        bag_data,
        min_len: 0,
        max_len: 0,
        min_char_len: 0,
        max_char_len: 0,
        opt_count: 0,
    }))
}

pub fn node_new_bag_memory(regnum: i32) -> Box<Node> {
    let mut node = node_new_bag(BagType::Memory);
    if let NodeInner::Bag(ref mut bn) = node.inner {
        bn.bag_data = BagData::Memory {
            regnum,
            called_addr: -1,
            entry_count: 0,
            called_state: 0,
        };
    }
    node
}

pub fn node_new_bag_if_else(
    cond: Box<Node>,
    then_node: Option<Box<Node>>,
    else_node: Option<Box<Node>>,
) -> Box<Node> {
    let mut node = node_new(NodeInner::Bag(BagNode {
        body: Some(cond),
        bag_type: BagType::IfElse,
        bag_data: BagData::IfElse {
            then_node,
            else_node,
        },
        min_len: 0,
        max_len: 0,
        min_char_len: 0,
        max_char_len: 0,
        opt_count: 0,
    }));
    node.status = 0;
    node
}

pub fn node_new_option(options: OnigOptionType) -> Box<Node> {
    let mut node = node_new_bag(BagType::Option);
    if let NodeInner::Bag(ref mut bn) = node.inner {
        bn.bag_data = BagData::Option { options };
    }
    node
}

pub fn node_new_anchor(anchor_type: i32) -> Box<Node> {
    node_new(NodeInner::Anchor(AnchorNode {
        body: None,
        anchor_type,
        char_min_len: 0,
        char_max_len: 0,
        ascii_mode: false,
        lead_node: None,
    }))
}

pub fn node_new_anchor_with_options(anchor_type: i32, options: OnigOptionType) -> Box<Node> {
    let mut node = node_new_anchor(anchor_type);
    if onig_is_option_on(options, ONIG_OPTION_IGNORECASE) {
        node.status_add(ND_ST_IGNORECASE);
    }
    if onig_is_option_on(options, ONIG_OPTION_MULTILINE) {
        node.status_add(ND_ST_MULTILINE);
    }
    if onig_is_option_on(options, ONIG_OPTION_TEXT_SEGMENT_WORD) {
        node.status_add(ND_ST_TEXT_SEGMENT_WORD);
    }
    node
}

pub fn node_new_list(car: Box<Node>, cdr: Option<Box<Node>>) -> Box<Node> {
    node_new(NodeInner::List(ConsAltNode { car, cdr }))
}

pub fn node_new_alt(car: Box<Node>, cdr: Option<Box<Node>>) -> Box<Node> {
    node_new(NodeInner::Alt(ConsAltNode { car, cdr }))
}

pub fn node_new_call(name: &[u8], gnum: i32, by_number: bool) -> Box<Node> {
    let mut node = node_new(NodeInner::Call(CallNode {
        body: None,
        by_number,
        called_gnum: gnum,
        name: name.to_vec(),
        entry_count: 0,
        target_node: std::ptr::null_mut(),
    }));
    if by_number {
        node.status_add(ND_ST_BY_NUMBER);
    }
    node
}

pub fn node_new_fail() -> Box<Node> {
    node_new(NodeInner::Gimmick(GimmickNode {
        gimmick_type: GimmickType::Fail,
        detail_type: 0,
        num: 0,
        id: 0,
    }))
}

pub fn node_new_callout(of: i32, num: i32, id: i32) -> Box<Node> {
    node_new(NodeInner::Gimmick(GimmickNode {
        gimmick_type: GimmickType::Callout,
        detail_type: of,   // 0=CONTENTS, 1=NAME
        num,               // callout list index
        id,                // builtin id for name callouts, -1 for contents
    }))
}

pub fn node_new_save_gimmick(save_type: SaveType, id: i32) -> Box<Node> {
    node_new(NodeInner::Gimmick(GimmickNode {
        gimmick_type: GimmickType::Save,
        detail_type: save_type as i32,
        num: 0,
        id,
    }))
}

pub fn node_new_update_var_gimmick(var_type: UpdateVarType, id: i32) -> Box<Node> {
    node_new(NodeInner::Gimmick(GimmickNode {
        gimmick_type: GimmickType::UpdateVar,
        detail_type: var_type as i32,
        num: 0,
        id,
    }))
}

// === Utility: make_list / make_alt ===

/// Create a 2-node list: (a . (b . nil))
pub fn make_list(a: Box<Node>, b: Box<Node>) -> Box<Node> {
    let tail = node_new_list(b, None);
    node_new_list(a, Some(tail))
}

/// Create a 2-node alternation: (a | b)
pub fn make_alt(a: Box<Node>, b: Box<Node>) -> Box<Node> {
    let tail = node_new_alt(b, None);
    node_new_alt(a, Some(tail))
}

/// Create a right-linked List chain: List(ns[0], List(ns[1], ... List(ns[n-1], nil)))
pub fn make_list_n(mut nodes: Vec<Box<Node>>) -> Box<Node> {
    assert!(!nodes.is_empty());
    let mut result = node_new_list(nodes.pop().unwrap(), None);
    while let Some(n) = nodes.pop() {
        result = node_new_list(n, Some(result));
    }
    result
}

/// Create a right-linked Alt chain: Alt(ns[0], Alt(ns[1], ... Alt(ns[n-1], nil)))
pub fn make_alt_n(mut nodes: Vec<Box<Node>>) -> Box<Node> {
    assert!(!nodes.is_empty());
    let mut result = node_new_alt(nodes.pop().unwrap(), None);
    while let Some(n) = nodes.pop() {
        result = node_new_alt(n, Some(result));
    }
    result
}

/// Anychar that matches newlines (C: node_new_true_anychar)
pub fn node_new_true_anychar() -> Box<Node> {
    let mut n = node_new_anychar();
    n.status_add(ND_ST_MULTILINE);
    n
}

// === Bitset Utility Functions (from regparse.c) ===

pub fn bitset_set_range(bs: &mut BitSet, from: usize, to: usize) {
    for i in from..=to {
        bitset_set_bit(bs, i);
    }
}

pub fn bitset_invert(bs: &mut BitSet) {
    for i in 0..BITSET_REAL_SIZE {
        bs[i] = !bs[i];
    }
}

pub fn bitset_invert_to(from: &BitSet, to: &mut BitSet) {
    for i in 0..BITSET_REAL_SIZE {
        to[i] = !from[i];
    }
}

pub fn bitset_and(dest: &mut BitSet, src: &BitSet) {
    for i in 0..BITSET_REAL_SIZE {
        dest[i] &= src[i];
    }
}

pub fn bitset_or(dest: &mut BitSet, src: &BitSet) {
    for i in 0..BITSET_REAL_SIZE {
        dest[i] |= src[i];
    }
}

pub fn bitset_copy(dest: &mut BitSet, src: &BitSet) {
    dest.copy_from_slice(src);
}

pub fn bitset_is_empty(bs: &BitSet) -> bool {
    bs.iter().all(|&b| b == 0)
}

// === String Node Helpers ===

/// Append bytes to a string node
pub fn node_str_cat(node: &mut Node, s: &[u8]) -> i32 {
    if let Some(sn) = node.as_str_mut() {
        sn.s.extend_from_slice(s);
        ONIG_NORMAL
    } else {
        ONIGERR_TYPE_BUG
    }
}

/// Set string node content
pub fn node_str_set(node: &mut Node, s: &[u8]) -> i32 {
    if let Some(sn) = node.as_str_mut() {
        sn.s.clear();
        sn.s.extend_from_slice(s);
        ONIG_NORMAL
    } else {
        ONIGERR_TYPE_BUG
    }
}

/// Clear string node
pub fn node_str_clear(node: &mut Node) {
    if let Some(sn) = node.as_str_mut() {
        sn.s.clear();
    }
}

/// Append a single code point to a string node (encoding-aware)
pub fn node_str_cat_codepoint(
    node: &mut Node,
    enc: OnigEncoding,
    code: OnigCodePoint,
) -> i32 {
    let mut buf = [0u8; ONIGENC_CODE_TO_MBC_MAXLEN];
    let len = enc.code_to_mbc(code, &mut buf);
    if len < 0 {
        return len;
    }
    node_str_cat(node, &buf[..len as usize])
}

// === Name Table (port of C's NameEntry + hash table) ===

pub struct NameEntry {
    pub name: Vec<u8>,
    pub back_num: i32,
    pub back_refs: Vec<i32>,
}

pub struct NameTable {
    pub entries: HashMap<Vec<u8>, NameEntry>,
}

impl NameTable {
    pub fn new() -> Self {
        NameTable {
            entries: HashMap::new(),
        }
    }

    pub fn find(&self, name: &[u8]) -> Option<&NameEntry> {
        self.entries.get(name)
    }

    pub fn find_mut(&mut self, name: &[u8]) -> Option<&mut NameEntry> {
        self.entries.get_mut(name)
    }

    pub fn add(
        &mut self,
        name: &[u8],
        backref: i32,
        allow_multiplex: bool,
    ) -> Result<(), i32> {
        if name.is_empty() {
            return Err(ONIGERR_EMPTY_GROUP_NAME);
        }

        if let Some(e) = self.entries.get_mut(name) {
            if e.back_num >= 1 && !allow_multiplex {
                return Err(ONIGERR_MULTIPLEX_DEFINED_NAME);
            }
            e.back_num += 1;
            e.back_refs.push(backref);
            Ok(())
        } else {
            self.entries.insert(
                name.to_vec(),
                NameEntry {
                    name: name.to_vec(),
                    back_num: 1,
                    back_refs: vec![backref],
                },
            );
            Ok(())
        }
    }

    pub fn num_entries(&self) -> i32 {
        self.entries.len() as i32
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn name_to_group_numbers(&self, name: &[u8]) -> Option<&[i32]> {
        self.entries.get(name).map(|e| e.back_refs.as_slice())
    }
}

// === Reduce Quantifier Type Table ===
