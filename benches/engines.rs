//! Further regex engines for the comparison benchmarks: PCRE2 (JIT and
//! interpreter), fancy-regex in its Oniguruma mode with and without seek mode,
//! fancy-regex's RegexSet over a whole scanner (scanner replays only, see
//! `engine_replay.rs`) and, with the `onigmo` feature, Ruby's Onigmo.
//!
//! Texts are `&str`, so every engine may skip UTF-8 validation of the
//! subject, as an application with known-valid text would; Ferroni and C
//! Oniguruma never validate it either.
//!
//! A benchmark only times one of these after it reproduced the Oniguruma
//! result for that case. A pattern an engine rejects or evaluates differently
//! is printed as an `UNSUPPORTED` line instead, so a missing timing always
//! comes with its reason.

/// Raw capture bounds as Oniguruma reports them; -1 marks an unset group.
pub type Captures = Vec<(i32, i32)>;

/// Explicit matching flags for source-backed production workloads.
#[derive(Clone, Copy, Default)]
pub struct CompileOptions {
    pub ignore_case: bool,
    pub multiline: bool,
    pub dotall: bool,
    pub extended: bool,
    pub capture_group: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Pcre2Jit,
    Pcre2,
    /// fancy-regex with its defaults.
    Fancy,
    /// fancy-regex with seek mode: the seek pre-filter for backtracking
    /// patterns, which is off by default.
    FancySeek,
    /// fancy-regex's `RegexSet` over all patterns of one scanner. It has no
    /// single-pattern form, so only the scanner replays run it.
    FancySet,
    #[cfg(feature = "onigmo")]
    Onigmo,
}

impl Engine {
    /// Every engine a scanner replay runs.
    pub const ALL: &[Engine] = &[
        Engine::Pcre2Jit,
        Engine::Pcre2,
        Engine::Fancy,
        Engine::FancySeek,
        Engine::FancySet,
        #[cfg(feature = "onigmo")]
        Engine::Onigmo,
    ];

    /// The engines that search one pattern at a time, which is what every
    /// per-pattern benchmark runs.
    pub const PER_PATTERN: &[Engine] = &[
        Engine::Pcre2Jit,
        Engine::Pcre2,
        Engine::Fancy,
        Engine::FancySeek,
        #[cfg(feature = "onigmo")]
        Engine::Onigmo,
    ];

    /// The engine segment of the benchmark ID.
    pub fn id(self) -> &'static str {
        match self {
            Engine::Pcre2Jit => "pcre2_jit",
            Engine::Pcre2 => "pcre2",
            Engine::Fancy => "fancy_regex",
            Engine::FancySeek => "fancy_regex_seek",
            Engine::FancySet => "fancy_regex_set",
            #[cfg(feature = "onigmo")]
            Engine::Onigmo => "onigmo",
        }
    }

    /// Compiles with Oniguruma's defaults: UTF-8, Unicode classes, and `^`/`$`
    /// as line anchors. `capture_group` keeps numbered groups next to named
    /// ones, as the TextMate scanner does.
    pub fn compile(
        self,
        pattern: &str,
        ignore_case: bool,
        capture_group: bool,
    ) -> Result<Compiled, String> {
        self.compile_with_options(
            pattern,
            CompileOptions {
                ignore_case,
                multiline: true,
                capture_group,
                ..CompileOptions::default()
            },
        )
    }

    pub fn compile_with_options(
        self,
        pattern: &str,
        options: CompileOptions,
    ) -> Result<Compiled, String> {
        match self {
            Engine::Pcre2Jit | Engine::Pcre2 => {
                pcre2::Regex::new(pattern, options, self == Engine::Pcre2Jit).map(Compiled::Pcre2)
            }
            Engine::Fancy | Engine::FancySeek => fancy_regex::RegexBuilder::new(pattern)
                .oniguruma_mode(true)
                .multi_line(options.multiline)
                .dot_matches_new_line(options.dotall)
                .ignore_whitespace(options.extended)
                .case_insensitive(options.ignore_case)
                .ignore_numbered_groups_when_named_groups_exist(!options.capture_group)
                .seek(self == Engine::FancySeek)
                .build()
                .map(Compiled::Fancy)
                .map_err(|error| error.to_string()),
            Engine::FancySet => {
                Err("a RegexSet searches a whole scanner, not one pattern".to_owned())
            }
            #[cfg(feature = "onigmo")]
            Engine::Onigmo => onigmo::Regex::new(pattern.as_bytes(), options).map(Compiled::Onigmo),
        }
    }
}

pub enum Compiled {
    Pcre2(pcre2::Regex),
    Fancy(fancy_regex::Regex),
    #[cfg(feature = "onigmo")]
    Onigmo(onigmo::Regex),
}

impl Compiled {
    /// Boolean matching through the search adapters.
    pub fn is_match(&self, text: &str, start: usize) -> Result<bool, String> {
        self.search(text, start).map(|found| found.is_some())
    }

    /// First match span, without materializing subgroup captures.
    pub fn find(&self, text: &str, start: usize) -> Result<Option<(usize, usize)>, String> {
        match self {
            Compiled::Pcre2(regex) => regex.search(text, start, |v| (v[0], v[1])),
            Compiled::Fancy(regex) => regex
                .find_from_pos(text, start)
                .map(|found| found.map(|m| (m.start(), m.end())))
                .map_err(|error| error.to_string()),
            #[cfg(feature = "onigmo")]
            Compiled::Onigmo(regex) => regex
                .search(text.as_bytes(), start, true)
                .map(|found| found.map(|_| regex.match_span())),
        }
    }

    /// Native replacement where the library provides it. C engines use the
    /// documented adapter because their public API has no replacement method.
    pub fn native_replace(&self, text: &str, replacement: &str) -> Option<Result<String, String>> {
        match self {
            Compiled::Fancy(regex) => Some(
                regex
                    .try_replacen(text, 0, replacement)
                    .map(|output| output.into_owned())
                    .map_err(|error| error.to_string()),
            ),
            _ => None,
        }
    }

    /// Start of the first match at or after `start`, without capture output.
    pub fn search(&self, text: &str, start: usize) -> Result<Option<usize>, String> {
        match self {
            Compiled::Pcre2(regex) => regex.search(text, start, |ovector| ovector[0]),
            Compiled::Fancy(regex) => regex
                .find_from_pos(text, start)
                .map(|m| m.map(|m| m.start()))
                .map_err(|error| error.to_string()),
            #[cfg(feature = "onigmo")]
            Compiled::Onigmo(regex) => regex.search(text.as_bytes(), start, false),
        }
    }

    /// The first match at or after `start` with every capture bound.
    pub fn captures(&self, text: &str, start: usize) -> Result<Option<Captures>, String> {
        let bound = |range: Option<(usize, usize)>| {
            range.map_or((-1, -1), |(beg, end)| (beg as i32, end as i32))
        };
        match self {
            Compiled::Pcre2(regex) => regex.search(text, start, |ovector| {
                ovector
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&[beg, end]| bound((beg != usize::MAX).then_some((beg, end))))
                    .collect()
            }),
            Compiled::Fancy(regex) => {
                let found = regex
                    .captures_from_pos(text, start)
                    .map_err(|error| error.to_string())?;
                Ok(found.map(|captures| {
                    (0..captures.len())
                        .map(|i| bound(captures.get(i).map(|m| (m.start(), m.end()))))
                        .collect()
                }))
            }
            #[cfg(feature = "onigmo")]
            Compiled::Onigmo(regex) => Ok(regex
                .search(text.as_bytes(), start, true)?
                .map(|_| regex.captures())),
        }
    }

    /// The start of the first match at or after `start`. Its captures stay in
    /// the engine's own result storage, for [`Self::read_found`], until the
    /// next search with this pattern: a scanner loop reads only the winner's,
    /// as vscode-oniguruma keeps one region per pattern.
    pub fn search_found<'t>(
        &self,
        text: &'t str,
        start: usize,
    ) -> Result<Option<(usize, Found<'t>)>, String> {
        match self {
            Compiled::Pcre2(regex) => {
                regex.search(text, start, |ovector| (ovector[0], Found::Kept))
            }
            Compiled::Fancy(regex) => Ok(regex
                .captures_from_pos(text, start)
                .map_err(|error| error.to_string())?
                .map(|captures| {
                    (
                        captures.get(0).map_or(start, |m| m.start()),
                        Found::Fancy(captures),
                    )
                })),
            #[cfg(feature = "onigmo")]
            Compiled::Onigmo(regex) => Ok(regex
                .search(text.as_bytes(), start, true)?
                .map(|at| (at, Found::Kept))),
        }
    }

    /// Calls `bound` with every capture of a match from
    /// [`Self::search_found`] on this pattern, in group order, with -1
    /// bounds for an unset group.
    pub fn read_found(&self, found: &Found<'_>, mut bound: impl FnMut(i32, i32)) {
        match (self, found) {
            (Compiled::Pcre2(regex), Found::Kept) => regex.read_last(|ovector| {
                for &[beg, end] in ovector.as_chunks::<2>().0 {
                    if beg == usize::MAX {
                        bound(-1, -1);
                    } else {
                        bound(beg as i32, end as i32);
                    }
                }
            }),
            (Compiled::Fancy(_), Found::Fancy(captures)) => {
                for group in 0..captures.len() {
                    match captures.get(group) {
                        Some(m) => bound(m.start() as i32, m.end() as i32),
                        None => bound(-1, -1),
                    }
                }
            }
            #[cfg(feature = "onigmo")]
            (Compiled::Onigmo(regex), Found::Kept) => regex.read_region(bound),
            _ => unreachable!("a match is read with the pattern that found it"),
        }
    }

    /// Whole-match spans through each engine's public iterator, where available.
    pub fn native_all_matches(&self, text: &str) -> Option<Result<Vec<(usize, usize)>, String>> {
        match self {
            Self::Fancy(regex) => Some(
                regex
                    .find_iter(text)
                    .map(|result| {
                        result
                            .map(|m| (m.start(), m.end()))
                            .map_err(|error| error.to_string())
                    })
                    .collect(),
            ),
            _ => None,
        }
    }
}

/// A match from [`Compiled::search_found`].
pub enum Found<'t> {
    /// The captures are in the pattern's own match data (PCRE2) or region
    /// (Onigmo).
    Kept,
    /// fancy-regex returns them as a value.
    Fancy(fancy_regex::Captures<'t, str>),
}

/// Prints why `engine` has no timing for the benchmark `id`.
pub fn unsupported(id: &str, engine: Engine, reason: &str) {
    unsupported_engine(id, engine.id(), reason);
}

/// [`unsupported`] for an engine outside [`Engine`], such as the `regex` crate.
pub fn unsupported_engine(id: &str, engine: &str, reason: &str) {
    println!(
        "UNSUPPORTED {}",
        serde_json::json!({"id": id, "engine": engine, "reason": reason})
    );
}

/// Compiles `pattern` for every engine and keeps those whose results pass
/// `check`, which compares them with Oniguruma's.
pub fn validated(
    group: &str,
    name: &str,
    pattern: &str,
    ignore_case: bool,
    check: impl Fn(&Compiled) -> Result<(), String>,
) -> Vec<(Engine, Compiled)> {
    Engine::PER_PATTERN
        .iter()
        .filter_map(|&engine| {
            let outcome = engine
                .compile(pattern, ignore_case, false)
                .map_err(|error| format!("compile: {error}"))
                .and_then(|compiled| check(&compiled).map(|()| compiled));
            match outcome {
                Ok(compiled) => Some((engine, compiled)),
                Err(reason) => {
                    unsupported(&format!("{group}/{}/{name}", engine.id()), engine, &reason);
                    None
                }
            }
        })
        .collect()
}

/// PCRE2 through its C API, because the `pcre2` crate validates the whole
/// subject as UTF-8 on every interpreter call and offers no way to skip it.
mod pcre2 {
    use pcre2_sys::*;
    use std::cell::Cell;
    use std::ptr;

    pub struct Regex {
        code: *mut pcre2_code_8,
        match_data: *mut pcre2_match_data_8,
        context: *mut pcre2_match_context_8,
        jit_stack: *mut pcre2_jit_stack_8,
        /// The match data is reused, so searches must not overlap.
        busy: Cell<bool>,
    }

    impl Regex {
        /// UTF-8 with Unicode classes and line anchors, as Oniguruma matches.
        pub fn new(pattern: &str, flags: super::CompileOptions, jit: bool) -> Result<Self, String> {
            let mut options = PCRE2_UTF | PCRE2_UCP;
            for (enabled, flag) in [
                (flags.ignore_case, PCRE2_CASELESS),
                (flags.multiline, PCRE2_MULTILINE),
                (flags.dotall, PCRE2_DOTALL),
                (flags.extended, PCRE2_EXTENDED),
            ] {
                if enabled {
                    options |= flag;
                }
            }
            let (mut error, mut offset) = (0, 0);
            // SAFETY: the pattern pointer and length describe a live slice;
            // the error out pointers are valid for the call.
            let code = unsafe {
                pcre2_compile_8(
                    pattern.as_ptr(),
                    pattern.len(),
                    options,
                    &mut error,
                    &mut offset,
                    ptr::null_mut(),
                )
            };
            if code.is_null() {
                return Err(format!("PCRE2 at offset {offset}: {}", message(error)));
            }
            // SAFETY: `code` is a freshly compiled pattern; the match data,
            // context and JIT stack created here are owned by `Regex`, whose
            // Drop frees them, also on the JIT error path.
            unsafe {
                let regex = Self {
                    code,
                    match_data: pcre2_match_data_create_from_pattern_8(code, ptr::null_mut()),
                    context: pcre2_match_context_create_8(ptr::null_mut()),
                    jit_stack: if jit {
                        pcre2_jit_stack_create_8(32 * 1024, 8 * 1024 * 1024, ptr::null_mut())
                    } else {
                        ptr::null_mut()
                    },
                    busy: Cell::new(false),
                };
                if jit {
                    let rc = pcre2_jit_compile_8(code, PCRE2_JIT_COMPLETE);
                    if rc != 0 {
                        return Err(format!("PCRE2 JIT: {}", message(rc)));
                    }
                    pcre2_jit_stack_assign_8(regex.context, None, regex.jit_stack.cast());
                }
                Ok(regex)
            }
        }

        /// Calls `read` with the capture offsets of the first match at or after
        /// `start`; PCRE2_UNSET (`usize::MAX`) marks a group that did not take
        /// part.
        pub fn search<R>(
            &self,
            text: &str,
            start: usize,
            read: impl FnOnce(&[usize]) -> R,
        ) -> Result<Option<R>, String> {
            // The match data is shared by all searches on this regex.
            assert!(!self.busy.replace(true), "overlapping PCRE2 searches");
            // SAFETY: `text` is valid UTF-8 by type, which PCRE2_NO_UTF_CHECK
            // requires; pattern, match data and context are owned by `self`.
            let rc = unsafe {
                pcre2_match_8(
                    self.code,
                    text.as_ptr(),
                    text.len(),
                    start,
                    PCRE2_NO_UTF_CHECK,
                    self.match_data,
                    self.context,
                )
            };
            let result = match rc {
                PCRE2_ERROR_NOMATCH => Ok(None),
                rc if rc > 0 => {
                    // SAFETY: after a successful match the ovector holds at
                    // least `rc` set pairs; `busy` keeps the match data from
                    // being overwritten while `read` borrows it.
                    let ovector = unsafe {
                        let count = pcre2_get_ovector_count_8(self.match_data) as usize;
                        std::slice::from_raw_parts(
                            pcre2_get_ovector_pointer_8(self.match_data),
                            2 * count,
                        )
                    };
                    Ok(Some(read(ovector)))
                }
                rc => Err(format!("PCRE2 match: {}", message(rc))),
            };
            self.busy.set(false);
            result
        }

        /// Calls `read` with the capture offsets the last successful
        /// [`Self::search`] left in the match data.
        pub fn read_last<R>(&self, read: impl FnOnce(&[usize]) -> R) -> R {
            assert!(!self.busy.get(), "PCRE2 match data read during a search");
            // SAFETY: the match data is owned by `self` and holds the last
            // match's ovector, which no search overwrites while `read`
            // borrows it: searches take `&self` and `busy` is clear.
            let ovector = unsafe {
                let count = pcre2_get_ovector_count_8(self.match_data) as usize;
                std::slice::from_raw_parts(pcre2_get_ovector_pointer_8(self.match_data), 2 * count)
            };
            read(ovector)
        }
    }

    impl Drop for Regex {
        fn drop(&mut self) {
            // SAFETY: each pointer is owned by this wrapper and freed once;
            // PCRE2's free functions accept null.
            unsafe {
                pcre2_match_context_free_8(self.context);
                pcre2_jit_stack_free_8(self.jit_stack);
                pcre2_match_data_free_8(self.match_data);
                pcre2_code_free_8(self.code);
            }
        }
    }

    fn message(code: i32) -> String {
        let mut buffer = [0u8; 256];
        // SAFETY: the buffer pointer and length describe a live array.
        let length = unsafe { pcre2_get_error_message_8(code, buffer.as_mut_ptr(), buffer.len()) };
        String::from_utf8_lossy(&buffer[..length.max(0) as usize]).into_owned()
    }
}

#[cfg(feature = "onigmo")]
mod onigmo {
    use std::ffi::{c_int, c_long, c_uchar, c_void};

    // benches/onigmo_shim.c, compiled with the renamed Onigmo by build.rs.
    unsafe extern "C" {
        fn ferroni_onigmo_init() -> c_int;
        fn ferroni_onigmo_new(
            pattern: *const c_uchar,
            length: usize,
            ignorecase: c_int,
            capture_group: c_int,
            multiline: c_int,
            dotall: c_int,
            extended: c_int,
            error: *mut c_int,
        ) -> *mut c_void;
        fn ferroni_onigmo_free(regex: *mut c_void);
        fn ferroni_onigmo_search(
            regex: *mut c_void,
            text: *const c_uchar,
            length: usize,
            start: usize,
            want_region: c_int,
        ) -> c_long;
        fn ferroni_onigmo_num_regs(regex: *const c_void) -> c_int;
        fn ferroni_onigmo_beg(regex: *const c_void, group: c_int) -> c_long;
        fn ferroni_onigmo_end(regex: *const c_void, group: c_int) -> c_long;
    }

    static INIT: std::sync::Once = std::sync::Once::new();

    /// One compiled Onigmo regex with its reusable region.
    pub struct Regex(*mut c_void);

    impl Regex {
        pub fn new(pattern: &[u8], options: super::CompileOptions) -> Result<Self, String> {
            // SAFETY: initialization takes no arguments and runs once.
            INIT.call_once(|| assert_eq!(unsafe { ferroni_onigmo_init() }, 0));
            let mut error = 0;
            // SAFETY: the pattern pointer and length describe a live slice for
            // the duration of the call; `error` is a valid out pointer.
            let raw = unsafe {
                ferroni_onigmo_new(
                    pattern.as_ptr(),
                    pattern.len(),
                    c_int::from(options.ignore_case),
                    c_int::from(options.capture_group),
                    c_int::from(options.multiline),
                    c_int::from(options.dotall),
                    c_int::from(options.extended),
                    &mut error,
                )
            };
            if raw.is_null() {
                Err(format!("Onigmo error {error}"))
            } else {
                Ok(Self(raw))
            }
        }

        /// The match start; with `want_region`, `captures` reads its groups.
        pub fn search(
            &self,
            text: &[u8],
            start: usize,
            want_region: bool,
        ) -> Result<Option<usize>, String> {
            // SAFETY: `self.0` is a live regex and the text slice outlives the
            // synchronous call.
            let position = unsafe {
                ferroni_onigmo_search(
                    self.0,
                    text.as_ptr(),
                    text.len(),
                    start,
                    c_int::from(want_region),
                )
            };
            match position {
                -1 => Ok(None),
                p if p >= 0 => Ok(Some(p as usize)),
                error => Err(format!("Onigmo search error {error}")),
            }
        }

        pub fn captures(&self) -> super::Captures {
            // SAFETY: the region belongs to `self.0` and was filled by the
            // preceding successful search with `want_region`.
            unsafe {
                (0..ferroni_onigmo_num_regs(self.0))
                    .map(|i| {
                        (
                            ferroni_onigmo_beg(self.0, i) as i32,
                            ferroni_onigmo_end(self.0, i) as i32,
                        )
                    })
                    .collect()
            }
        }

        /// Calls `bound` with each group of the region, -1 for an unset one,
        /// without collecting them as [`Self::captures`] does.
        pub fn read_region(&self, mut bound: impl FnMut(i32, i32)) {
            // SAFETY: the region belongs to `self.0` and was filled by the
            // preceding successful search with `want_region`.
            unsafe {
                for group in 0..ferroni_onigmo_num_regs(self.0) {
                    bound(
                        ferroni_onigmo_beg(self.0, group) as i32,
                        ferroni_onigmo_end(self.0, group) as i32,
                    );
                }
            }
        }

        pub fn match_span(&self) -> (usize, usize) {
            // SAFETY: a successful region-producing search populated group 0
            // on this owned regex before this method is called.
            unsafe {
                (
                    ferroni_onigmo_beg(self.0, 0) as usize,
                    ferroni_onigmo_end(self.0, 0) as usize,
                )
            }
        }
    }

    impl Drop for Regex {
        fn drop(&mut self) {
            // SAFETY: the wrapper owns the regex and frees it exactly once.
            unsafe { ferroni_onigmo_free(self.0) }
        }
    }
}
