//! Opt-in, thread-local load-phase diagnostics. Ordinary builds have no clocks.
use std::{cell::RefCell, marker::PhantomData, rc::Rc, time::Instant};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Phase {
    Total,
    ProgramIndex,
    GroupIndex,
    GroupBindingChecksum,
    PatternCache,
    ProgramLookup,
    ProgramChecksum,
    ProgramDecode,
    ProgramValidate,
    ProgramHelpers,
    EagerHelpers,
    GroupLookup,
    GroupChecksum,
    GroupDecode,
    GroupBuild,
    PortableDecode,
    PortableValidate,
    CompatibilityHandles,
    ScannerAssembly,
    ScannerScratch,
    PortableScratch,
}
const NAMES: [&str; 21] = [
    "total",
    "program_index",
    "group_index",
    "group_binding_checksum",
    "pattern_cache",
    "program_lookup",
    "program_checksum",
    "program_decode",
    "program_validate",
    "program_helpers",
    "eager_helpers",
    "group_lookup",
    "group_checksum",
    "group_decode",
    "group_build",
    "portable_decode",
    "portable_validate",
    "compatibility_handles",
    "scanner_assembly",
    "scanner_scratch",
    "portable_scratch",
];

/// Accumulated duration of one phase during a diagnostic load.
#[derive(Clone, Debug)]
pub struct Sample {
    /// Stable diagnostic name, not a wire-format field.
    pub phase: &'static str,
    /// Number of scoped calls.
    pub calls: u64,
    /// Time including nested phases, in nanoseconds.
    pub inclusive_ns: u128,
    /// Time excluding nested phases, in nanoseconds.
    pub exclusive_ns: u128,
}

#[derive(Default)]
struct State {
    active: bool,
    children: Vec<u128>,
    totals: [(u64, u128, u128); NAMES.len()],
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub(crate) struct Scope {
    phase: Phase,
    started: Option<Instant>,
    local: PhantomData<Rc<()>>,
}

pub(crate) fn scope(phase: Phase) -> Scope {
    let active = STATE.with_borrow_mut(|state| {
        if state.active {
            state.children.push(0);
        }
        state.active
    });
    Scope {
        phase,
        started: active.then(Instant::now),
        local: PhantomData,
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        let Some(started) = self.started else {
            return;
        };
        let elapsed = started.elapsed().as_nanos();
        STATE.with_borrow_mut(|state| {
            let children = state.children.pop().expect("balanced load phase");
            let total = &mut state.totals[self.phase as usize];
            total.0 += 1;
            total.1 += elapsed;
            total.2 += elapsed.saturating_sub(children);
            if let Some(parent) = state.children.last_mut() {
                *parent += elapsed;
            }
            if matches!(self.phase, Phase::Total) {
                state.active = false;
            }
        });
    }
}

/// Measure one synchronous load on this thread. Nested measurements are invalid.
/// Phase clocks perturb execution; compare final timings without this feature.
pub fn measure<T>(load: impl FnOnce() -> T) -> (T, Vec<Sample>) {
    STATE.with_borrow_mut(|state| {
        assert!(!state.active, "load measurements cannot nest");
        *state = State::default();
        state.active = true;
    });
    let total = scope(Phase::Total);
    let value = load();
    drop(total);
    let samples = STATE.with_borrow(|state| {
        NAMES
            .iter()
            .zip(&state.totals)
            .filter(|(_, total)| total.0 != 0)
            .map(|(&phase, &(calls, inclusive_ns, exclusive_ns))| Sample {
                phase,
                calls,
                inclusive_ns,
                exclusive_ns,
            })
            .collect()
    });
    (value, samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_phases_partition_time_and_reset_after_a_panic() {
        let (_, samples) = measure(|| {
            let _outer = scope(Phase::ProgramDecode);
            let _inner = scope(Phase::ProgramValidate);
        });
        assert_eq!(samples.iter().map(|s| s.calls).sum::<u64>(), 3);
        assert_eq!(
            samples.iter().map(|s| s.exclusive_ns).sum::<u128>(),
            samples[0].inclusive_ns
        );
        assert!(samples.iter().all(|s| s.exclusive_ns <= s.inclusive_ns));
        assert!(std::panic::catch_unwind(|| measure(|| panic!("diagnostic"))).is_err());
        let (value, samples) = measure(|| 7);
        assert_eq!(value, 7);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].calls, 1);
    }
}
