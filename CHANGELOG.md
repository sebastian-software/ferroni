# Changelog

## [1.0.0](https://github.com/sebastian-software/ferroni/compare/v0.1.0...v1.0.0) (2026-02-15)


### ⚠ BREAKING CHANGES

* onig_new() return type changed from Result<RegexType, i32> to Result<RegexType, RegexError>. OnigOptionType is now a bitflags struct instead of a u32 type alias. RegexType fields are pub(crate). Syntax parameters take references instead of raw pointers.

### Features

* add code coverage infrastructure and improve coverage to &gt;80% ([d31210b](https://github.com/sebastian-software/ferroni/commit/d31210b6e3937fccde10c470992be9515ac2fdbc))
* add idiomatic Rust API layer and type-safe internals ([25e4505](https://github.com/sebastian-software/ferroni/commit/25e4505763efd6e44bb665dbe8cb302341be48a5))
* implement 9 missing C API functions for ~99% API parity ([d7ebbda](https://github.com/sebastian-software/ferroni/commit/d7ebbdab954cebd85977306b9c6de865f793b954))


### Bug Fixes

* **ci:** resolve clippy, format, and MSRV failures ([41580d0](https://github.com/sebastian-software/ferroni/commit/41580d057d188267f070cf2c7c850bff95b7d3e3))

## 0.1.0 (2026-02-13)


### Features

* achieve 100% C test parity with full Unicode text segmentation ([03b82fb](https://github.com/sebastian-software/ferroni/commit/03b82fbc0df09fea76fcab39db746859470a1b2a))
* add ASCII encoding implementation (encodings/ascii.rs) ([3e377c2](https://github.com/sebastian-software/ferroni/commit/3e377c21719b9ba714d5990e14a9cbe7b329ff23))
* add character class parser, group parser, and smoke tests (regparse.rs) ([46502a6](https://github.com/sebastian-software/ferroni/commit/46502a669c15ace6fd33fa8e7229fd3c9f67863b))
* add compiler infrastructure and code generation (regcomp.rs) ([7f1365f](https://github.com/sebastian-software/ferroni/commit/7f1365fd65472ce3e94727477296857c4f800c83))
* add Criterion benchmark suite comparing Rust port vs C original ([caae133](https://github.com/sebastian-software/ferroni/commit/caae1336b063028d60bf792501e368898f1fe770))
* add Encoding trait and shared utilities (regenc.rs) ([2dfb1da](https://github.com/sebastian-software/ferroni/commit/2dfb1da19273c652c3885e4f323d8fa55f905dee))
* add full Unicode property support (\p{...}, \pL) ([22dab14](https://github.com/sebastian-software/ferroni/commit/22dab14ce8840a4beac9779b4242d41f6715692f))
* add internal types, OpCode, and regex_t (regint.rs) ([791fef1](https://github.com/sebastian-software/ferroni/commit/791fef1d1f93002a3921a92ad9d4990201b8143d))
* add OnigCaptureTreeNode helpers and OnigRegion convenience methods ([8286a76](https://github.com/sebastian-software/ferroni/commit/8286a768468017e0dc2a9552d17083caa62535c8))
* add parser AST types and supporting structures (regparse_types.rs) ([ba6db3e](https://github.com/sebastian-software/ferroni/commit/ba6db3e772f94a9d5de23a50cd01f8de7c5d62e3))
* add parser infrastructure, tokenizer, and recursive descent parser (regparse.rs) ([20c878c](https://github.com/sebastian-software/ferroni/commit/20c878ceecef76d6e25f44f9684bd81f6cbbd2f2))
* add parser support for \y/\Y boundaries, capture history, (?y{g/w}) ([5cc2a6c](https://github.com/sebastian-software/ferroni/commit/5cc2a6c92ffdd1c71f978f67812b92935e32c8a0))
* add public API, UTF-8 support, integration tests, and error module ([808ca40](https://github.com/sebastian-software/ferroni/commit/808ca40512fc615139d7f8f3b14be5cdc738964b))
* add public types and constants (oniguruma.rs) ([cc56ecb](https://github.com/sebastian-software/ferroni/commit/cc56ecb479e9b02350be5949829d9d446f434720))
* add regtrav (capture tree traversal) and regset (multi-regex set) ([d1ff11e](https://github.com/sebastian-software/ferroni/commit/d1ff11eff00fdc26b4d983ad27f36484841efb14))
* add syntax definitions for all 12 regex flavors (regsyntax.rs) ([996e256](https://github.com/sebastian-software/ferroni/commit/996e25656a3d800553093708d40d5105fc1b3765))
* add tune_tree pass with empty-loop detection ([5d21f99](https://github.com/sebastian-software/ferroni/commit/5d21f999f9f677349dfde291f9aaa0c9fa9410fa))
* add Unicode module with stub functions (unicode/mod.rs) ([67d0538](https://github.com/sebastian-software/ferroni/commit/67d053876d3e33f7d1bd4067b04d0d0400ecb3d2))
* add UTF-8 encoding implementation (encodings/utf8.rs) ([bf63451](https://github.com/sebastian-software/ferroni/commit/bf63451b4c1f944b5c0f7e1b109e6a4385a113b8))
* add VM executor with backtracking engine (regexec.rs) ([964eca3](https://github.com/sebastian-software/ferroni/commit/964eca30df81a7ff8f95d9998c2d2a01b6822d28))
* case-insensitive char class range expansion via apply_all_case_fold ([7ec41c8](https://github.com/sebastian-software/ferroni/commit/7ec41c8d308fb4250cf9271f7fd31463f41b113b))
* case-insensitive matching, lookbehind, cc_intersect fix (831 pass, 0 ignored) ([0a17c14](https://github.com/sebastian-software/ferroni/commit/0a17c14fa298de387065f587093785a6c815bf5e))
* compile {0} quantifier bodies containing CALLED groups ([cdc9676](https://github.com/sebastian-software/ferroni/commit/cdc9676905559caf005f9b8ebdaf8d7f6b9aeb19))
* detect too-big repeat range in nested fixed quantifiers (1169 passing, 184 ignored) ([45c1d32](https://github.com/sebastian-software/ferroni/commit/45c1d322d3f03a1c0f7245efdc61f77c54d8e6b2))
* enhance compiler with lookbehind validation, case-fold checks, absent stopper ([2dcd9bf](https://github.com/sebastian-software/ferroni/commit/2dcd9bf50350fab14b186be324a0021ff4171b0a))
* handle IfElse in node_char_len for lookbehind (1446 passing, 22 ignored) ([9feb82b](https://github.com/sebastian-software/ferroni/commit/9feb82b7bda35b62a04d45757cbbc11b9e5ab3a4))
* implement (?C) dont-capture-group option (1297 passing, 171 ignored) ([45ca90b](https://github.com/sebastian-software/ferroni/commit/45ca90b16e8e57a91e5b3e88574c355d2377a27f))
* implement (?I), (?W), (?D), (?S), (?P) option letters (1294 passing, 174 ignored) ([fa90c05](https://github.com/sebastian-software/ferroni/commit/fa90c05b8f4de49d6752ef93e0d6cf027c4b54b0))
* implement (?L) find-longest option (1306 passing, 162 ignored) ([c3e933f](https://github.com/sebastian-software/ferroni/commit/c3e933fea85b60e0d6239f0325cec172840489f9))
* implement (*FAIL) callout and (*NAME) error (1443 passing, 25 ignored) ([1f7e74e](https://github.com/sebastian-software/ferroni/commit/1f7e74e0d0030442f9b0b51e8a6b7bc0e62ab189))
* implement \g&lt;0&gt; whole-pattern self-call support ([ba16807](https://github.com/sebastian-software/ferroni/commit/ba1680768c7c1cef8d1048d2c2a2e661cb14c44e))
* implement \g&lt;name&gt;/\g&lt;num&gt; subroutine calls with CALL/RETURN opcodes ([98c7357](https://github.com/sebastian-software/ferroni/commit/98c73578037fa133817bcc2ba964f1d3a82b946c))
* implement \k&lt;name&gt; named backref tokenizer with level syntax ([d459659](https://github.com/sebastian-software/ferroni/commit/d459659388ad1561d4a490c9fd1c585e2ce116e7))
* implement \R general newline support (1179 passing, 174 ignored) ([2c94af0](https://github.com/sebastian-software/ferroni/commit/2c94af079e95a338c206cf18439187368aed128f))
* implement \X grapheme cluster via TextSegmentBoundary (1453 passing, 15 ignored) ([5ba09fe](https://github.com/sebastian-software/ferroni/commit/5ba09fed6e8e8aff998d7cc09bedf89a482bea67))
* implement 28 missing API functions for 93% API parity ([38af410](https://github.com/sebastian-software/ferroni/commit/38af41076dc74a3a910a018d21ad837ea87557a6))
* implement 3-pass call-node tuning (tune_call, tune_call2, tune_called_state) ([bfc8a80](https://github.com/sebastian-software/ferroni/commit/bfc8a80f99a4667691c6be09ce01adca8a7c69af))
* implement backref levels \k&lt;1+n&gt; for recursion (1462 passing, 6 ignored) ([5b642fd](https://github.com/sebastian-software/ferroni/commit/5b642fd3edad63fa42531bf1432c70a491ae03ab))
* implement backward search and add safety limit tests ([3b5dbb0](https://github.com/sebastian-software/ferroni/commit/3b5dbb06bee2d657f5465c5d40e3f36cba1d57ce))
* implement case-insensitive backrefs, multi-backrefs, and backref check ([8b4f93a](https://github.com/sebastian-software/ferroni/commit/8b4f93a121d0729dae751d8275dcae0a13cbe8cc))
* implement complete executor public API ([79a3942](https://github.com/sebastian-software/ferroni/commit/79a39426d6ea0c8f83f43febde29712bdae7822b))
* implement complete optimization subsystem (~35 functions, ~2260 LOC) ([5045b6b](https://github.com/sebastian-software/ferroni/commit/5045b6b2a9db846f230297f51181377c471527ee))
* implement conditional patterns (?(1)then|else) with backref checker ([9427277](https://github.com/sebastian-software/ferroni/commit/94272779734f3cfc296372f979b9c802dc7c14b0))
* implement full 6x6 ReduceTypeTable for nested quantifier reduction ([c72ed42](https://github.com/sebastian-software/ferroni/commit/c72ed424a38f6ea4101f215d7ebeb800be30ae7c))
* implement lookbehind reduction and string expansion in tune_tree ([935faeb](https://github.com/sebastian-software/ferroni/commit/935faeb312e374bb40000f69959e9a61966efbe2))
* implement MEM_END_REC, CAPTURE_ONLY_NAMED_GROUP, and grapheme cluster \X (1458 passing, 10 ignored) ([aadafc8](https://github.com/sebastian-software/ferroni/commit/aadafc8df30f2628f334339ff0b7b52af503206f))
* implement multi-char case fold expansion in char classes (1450 passing, 18 ignored) ([5c6ec68](https://github.com/sebastian-software/ferroni/commit/5c6ec68c7ebf178b7e897db5355d321cf3c9d271))
* implement multi-codepoint \x{} and \o{} syntax with range support (1205 passing, 148 ignored) ([eba0d41](https://github.com/sebastian-software/ferroni/commit/eba0d4155560eaa6955625089ddb207125edb8b3))
* implement never-ending recursion detection (1460 passing, 8 ignored) ([e6ab6b2](https://github.com/sebastian-software/ferroni/commit/e6ab6b27d4d28bb9f055f15f1f09aac213a4ec6d))
* implement safety limits (retry/time/stack) for match engine ([fd5cb4b](https://github.com/sebastian-software/ferroni/commit/fd5cb4b21f85735666fe45b3c1d5de7571faea2d))
* implement tune_next, is_exclusive, and automatic possessification ([c4c7391](https://github.com/sebastian-software/ferroni/commit/c4c7391195e1d82e7ee8b99ce70d2f2abdb8db77))
* implement UTF-8 raw byte validation (1313 passing, 155 ignored) ([8362302](https://github.com/sebastian-software/ferroni/commit/83623028d7d63d3cb44b757bfe6c67f04637f001))
* port 257 more tests from C test_utf8.c (648 passing, 80 ignored) ([5be130d](https://github.com/sebastian-software/ferroni/commit/5be130dfab3e1fd63f009b58cbe804f9f003b0a5))
* port 273 more tests from C test_utf8.c (963 passing, 141 ignored) ([03792e2](https://github.com/sebastian-software/ferroni/commit/03792e217b04a7fbe374a8a012e64c84328b0da8))
* port 328 new tests from C test_utf8.c, fix string-split for quantifiers ([72c9f79](https://github.com/sebastian-software/ferroni/commit/72c9f79d6cec4ac7508ebcfdd6c8016a43ed1bb9))
* port 371 more tests from C test_utf8.c lines 1411-1791 (1066 passing, 287 ignored) ([341f788](https://github.com/sebastian-software/ferroni/commit/341f788815c3ab43ba987d5bac92b1d09f1bd38b))
* port remaining C tests for 100% coverage, fix \K keep support ([c4aa2ec](https://github.com/sebastian-software/ferroni/commit/c4aa2ecc3b55527314b220f24c12387cb7880439))
* support (*FAIL) in conditional condition (1447 passing, 21 ignored) ([660ad20](https://github.com/sebastian-software/ferroni/commit/660ad2089ef2f3c8157c46c6cf0baffe43250093))
* support \g&lt;n&gt; subroutine calls in lookbehind (1464 passing, 4 ignored) ([21a30cd](https://github.com/sebastian-software/ferroni/commit/21a30cdc4aca9e5b4bae4aa202ea9740d98ac834))
* un-ignore lookbehind backref tests (1440 passing, 28 ignored) ([cc3e5b7](https://github.com/sebastian-software/ferroni/commit/cc3e5b7a1dbbb3e95c9231ed039c15b7c4f327da))
* Unicode case folding with multi-char fold support (ß↔ss, ſ↔s) ([39ef77b](https://github.com/sebastian-software/ferroni/commit/39ef77b1c8b4cb439d2f21d90ccd4aa4d6b6cad9))


### Bug Fixes

* 100% syntax and options test parity with 19 fixes ([8251944](https://github.com/sebastian-software/ferroni/commit/8251944f0a52b08fb9c8c6ceb10717e0c3c79a86))
* backward search, text segment boundary, and add test suites ([71cea23](https://github.com/sebastian-software/ferroni/commit/71cea232d651bd08a56e9cb750d613eb444f41de))
* BMH forward search fallthrough causing 360x no-match regression ([5f05d11](https://github.com/sebastian-software/ferroni/commit/5f05d1152536b194b1f6e707a47702b3c2721232))
* conditional then/else splitting with prs_branch (1445 passing, 23 ignored) ([cef41d2](https://github.com/sebastian-software/ferroni/commit/cef41d23ccbb76f7abd5f1a0ba32fd21812acc00))
* correct EmptyCheckEnd vs EmptyCheckEndMemst opcode selection (1452 passing, 16 ignored) ([412fee7](https://github.com/sebastian-software/ferroni/commit/412fee7f7b83b4dedef07eb9b32f5c060951445d))
* correct test porting typo for FB06 case fold test (1451 passing, 17 ignored) ([dcedd7e](https://github.com/sebastian-software/ferroni/commit/dcedd7e2ec68366b58e7e442ae36109bb77861a0))
* handle [[::]] empty POSIX bracket as nested char class (1314 passing, 154 ignored) ([57f1b62](https://github.com/sebastian-software/ferroni/commit/57f1b62160b9fee0d4376843010d7f7d84c8f90f))
* inline (?i) option scoping — parse rest of pattern as option body ([b7a9d21](https://github.com/sebastian-software/ferroni/commit/b7a9d2163bf14f9ecb1e3c090565a0d33d6ea90a))
* negative lookbehind compilation and variable-length alt splitting ([0cc9d01](https://github.com/sebastian-software/ferroni/commit/0cc9d01ec56a3413a878ac27c0743f0be07155ed))
* parser multi-byte safety, comment groups, and string optimization ([51a0a17](https://github.com/sebastian-software/ferroni/commit/51a0a17032dcf88fe686eaa1b51fd65bcf5f75e6))
* replace broken trigraph test, add group quantifier tests ([8eedb5c](https://github.com/sebastian-software/ferroni/commit/8eedb5ce77352b0c3b189408014d71c13bf7255e))


### Performance Improvements

* enable thin LTO and update benchmarks ([b7976bc](https://github.com/sebastian-software/ferroni/commit/b7976bc9626fb16b60fff6af91e0129e4cb5c5fa))
* replace hand-written byte scans with SIMD-accelerated memchr ([bf8042d](https://github.com/sebastian-software/ferroni/commit/bf8042d2b426ae3c11fa13ee4ebe9409bfee10c2))
* reuse VM stack across match_at calls and use u32 for Unicode ranges ([96025c1](https://github.com/sebastian-software/ferroni/commit/96025c1b17c67d15e9009cbb14a67b82a0b2dfe6))
