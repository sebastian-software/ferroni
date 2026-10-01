//! Further regex engines for the comparison benchmarks: PCRE2 (JIT and
//! interpreter), fancy-regex in its Oniguruma mode and, with the `onigmo`
//! feature, Ruby's Onigmo.
//!
//! A benchmark only times one of these after it reproduced the Oniguruma
//! result for that case. A pattern an engine rejects or evaluates differently
//! is printed as an `UNSUPPORTED` line instead, so a missing timing always
//! comes with its reason.

use std::cell::RefCell;

/// Raw capture bounds as Oniguruma reports them; -1 marks an unset group.
pub type Captures = Vec<(i32, i32)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Pcre2Jit,
    Pcre2,
    Fancy,
    #[cfg(feature = "onigmo")]
    Onigmo,
}

impl Engine {
    pub const ALL: &[Engine] = &[
        Engine::Pcre2Jit,
        Engine::Pcre2,
        Engine::Fancy,
        #[cfg(feature = "onigmo")]
        Engine::Onigmo,
    ];

    /// The engine segment of the benchmark ID.
    pub fn id(self) -> &'static str {
        match self {
            Engine::Pcre2Jit => "pcre2_jit",
            Engine::Pcre2 => "pcre2",
            Engine::Fancy => "fancy_regex",
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
        match self {
            Engine::Pcre2Jit | Engine::Pcre2 => {
                let regex = pcre2::bytes::RegexBuilder::new()
                    .utf(true)
                    .ucp(true)
                    .multi_line(true)
                    .caseless(ignore_case)
                    .jit(self == Engine::Pcre2Jit)
                    .build(pattern)
                    .map_err(|error| error.to_string())?;
                let locations = RefCell::new(regex.capture_locations());
                Ok(Compiled::Pcre2(regex, locations))
            }
            Engine::Fancy => fancy_regex::RegexBuilder::new(pattern)
                .oniguruma_mode(true)
                .multi_line(true)
                .case_insensitive(ignore_case)
                .ignore_numbered_groups_when_named_groups_exist(!capture_group)
                .build()
                .map(Compiled::Fancy)
                .map_err(|error| error.to_string()),
            #[cfg(feature = "onigmo")]
            Engine::Onigmo => onigmo::Regex::new(pattern.as_bytes(), ignore_case, capture_group)
                .map(Compiled::Onigmo),
        }
    }
}

pub enum Compiled {
    Pcre2(pcre2::bytes::Regex, RefCell<pcre2::bytes::CaptureLocations>),
    Fancy(fancy_regex::Regex),
    #[cfg(feature = "onigmo")]
    Onigmo(onigmo::Regex),
}

impl Compiled {
    /// Start of the first match at or after `start`, without capture output.
    pub fn search(&self, text: &[u8], start: usize) -> Result<Option<usize>, String> {
        match self {
            Compiled::Pcre2(regex, _) => regex
                .find_at(text, start)
                .map(|m| m.map(|m| m.start()))
                .map_err(|error| error.to_string()),
            Compiled::Fancy(regex) => regex
                .find_from_pos(text, start)
                .map(|m| m.map(|m| m.start()))
                .map_err(|error| error.to_string()),
            #[cfg(feature = "onigmo")]
            Compiled::Onigmo(regex) => regex.search(text, start, false),
        }
    }

    /// The first match at or after `start` with every capture bound.
    pub fn captures(&self, text: &[u8], start: usize) -> Result<Option<Captures>, String> {
        let bound = |range: Option<(usize, usize)>| {
            range.map_or((-1, -1), |(beg, end)| (beg as i32, end as i32))
        };
        match self {
            Compiled::Pcre2(regex, locations) => {
                let mut locations = locations.borrow_mut();
                let found = regex
                    .captures_read_at(&mut locations, text, start)
                    .map_err(|error| error.to_string())?;
                Ok(found.map(|_| {
                    (0..locations.len())
                        .map(|i| bound(locations.get(i)))
                        .collect()
                }))
            }
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
            Compiled::Onigmo(regex) => {
                Ok(regex.search(text, start, true)?.map(|_| regex.captures()))
            }
        }
    }
}

/// Prints why `engine` has no timing for the benchmark `id`.
pub fn unsupported(id: &str, engine: Engine, reason: &str) {
    println!(
        "UNSUPPORTED {}",
        serde_json::json!({"id": id, "engine": engine.id(), "reason": reason})
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
    Engine::ALL
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
        pub fn new(pattern: &[u8], ignore_case: bool, capture_group: bool) -> Result<Self, String> {
            // SAFETY: initialization takes no arguments and runs once.
            INIT.call_once(|| assert_eq!(unsafe { ferroni_onigmo_init() }, 0));
            let mut error = 0;
            // SAFETY: the pattern pointer and length describe a live slice for
            // the duration of the call; `error` is a valid out pointer.
            let raw = unsafe {
                ferroni_onigmo_new(
                    pattern.as_ptr(),
                    pattern.len(),
                    c_int::from(ignore_case),
                    c_int::from(capture_group),
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
    }

    impl Drop for Regex {
        fn drop(&mut self) {
            // SAFETY: the wrapper owns the regex and frees it exactly once.
            unsafe { ferroni_onigmo_free(self.0) }
        }
    }
}
