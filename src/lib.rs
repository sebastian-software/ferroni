// ferroni - 1:1 Rust port of Oniguruma regex engine
//
// Module structure mirrors the C original:
//   oniguruma.h  -> oniguruma.rs   (public types, constants)
//   regint.h     -> regint.rs      (internal types, OpCode, Operation, regex_t)
//   regenc.h/c   -> regenc.rs      (Encoding trait, encoding utilities)
//   encodings/*  -> encodings/*    (per-encoding implementations)

pub mod oniguruma;
pub mod regint;
pub mod regenc;
pub mod regsyntax;
pub mod regparse_types;
pub mod unicode;
pub mod encodings;
pub mod regparse;
pub mod regcomp;
