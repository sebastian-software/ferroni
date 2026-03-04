use ferroni::encodings::ONIG_ENCODING_UTF8;
use ferroni::oniguruma::*;
use ferroni::regcomp::onig_new;
use ferroni::regexec::onig_match;
use ferroni::regexec::onig_region_new;
use ferroni::regexec::onig_search;
use ferroni::regsyntax::OnigSyntaxOniguruma;

fn main() {
    let text = b"Event on 2025-12-31 at venue, next on 2026-01-15.";

    // Named capture version
    let pat_named = b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})";
    let reg_named = onig_new(
        pat_named,
        ONIG_OPTION_NONE,
        &ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
    )
    .unwrap();

    // Non-capture version (same pattern, no named groups)
    let pat_bare = b"\\d{4}-\\d{2}-\\d{2}";
    let reg_bare = onig_new(
        pat_bare,
        ONIG_OPTION_NONE,
        &ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma,
    )
    .unwrap();

    let n = 10_000_000u64;

    // Warm up
    let region = Some(onig_region_new());
    let (_, region) = onig_search(
        &reg_named,
        text,
        text.len(),
        0,
        text.len(),
        region,
        ONIG_OPTION_NONE,
    );

    // Bench named with region
    let start = std::time::Instant::now();
    let mut region = region;
    for _ in 0..n {
        let mut r = region.take().unwrap();
        let (pos, ret) = onig_search(
            &reg_named,
            std::hint::black_box(text),
            text.len(),
            0,
            text.len(),
            Some(r),
            ONIG_OPTION_NONE,
        );
        region = ret;
        std::hint::black_box(pos);
    }
    let named_time = start.elapsed();
    eprintln!(
        "named + region:  {:>6.1} ns/iter",
        named_time.as_nanos() as f64 / n as f64
    );

    // Bench named without region
    let start = std::time::Instant::now();
    for _ in 0..n {
        let (pos, _) = onig_search(
            &reg_named,
            std::hint::black_box(text),
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        std::hint::black_box(pos);
    }
    let named_noreg = start.elapsed();
    eprintln!(
        "named no region: {:>6.1} ns/iter",
        named_noreg.as_nanos() as f64 / n as f64
    );

    // Bench bare (no captures) with region
    let start = std::time::Instant::now();
    let mut region = Some(onig_region_new());
    for _ in 0..n {
        let mut r = region.take().unwrap();
        let (pos, ret) = onig_search(
            &reg_bare,
            std::hint::black_box(text),
            text.len(),
            0,
            text.len(),
            Some(r),
            ONIG_OPTION_NONE,
        );
        region = ret;
        std::hint::black_box(pos);
    }
    let bare_time = start.elapsed();
    eprintln!(
        "bare + region:   {:>6.1} ns/iter",
        bare_time.as_nanos() as f64 / n as f64
    );

    // Bench bare without region
    let start = std::time::Instant::now();
    for _ in 0..n {
        let (pos, _) = onig_search(
            &reg_bare,
            std::hint::black_box(text),
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        std::hint::black_box(pos);
    }
    let bare_noreg = start.elapsed();
    eprintln!(
        "bare no region:  {:>6.1} ns/iter",
        bare_noreg.as_nanos() as f64 / n as f64
    );

    eprintln!(
        "\ncapture overhead (region):    {:>6.1} ns",
        (named_time - bare_time).as_nanos() as f64 / n as f64
    );
    eprintln!(
        "capture overhead (no region): {:>6.1} ns",
        (named_noreg - bare_noreg).as_nanos() as f64 / n as f64
    );
    eprintln!(
        "region overhead (named):      {:>6.1} ns",
        (named_time - named_noreg).as_nanos() as f64 / n as f64
    );
    eprintln!(
        "region overhead (bare):       {:>6.1} ns",
        (bare_time - bare_noreg).as_nanos() as f64 / n as f64
    );

    // Two-pass: search without region, then match with region at found position
    let start = std::time::Instant::now();
    let mut region = Some(onig_region_new());
    for _ in 0..n {
        let (pos, _) = onig_search(
            &reg_named,
            std::hint::black_box(text),
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        let at = pos as usize;
        let r = region.take().unwrap();
        let (_len, ret) = onig_match(
            &reg_named,
            std::hint::black_box(text),
            text.len(),
            at,
            Some(r),
            ONIG_OPTION_NONE,
        );
        region = ret;
        std::hint::black_box(pos);
    }
    let two_pass = start.elapsed();
    eprintln!(
        "two-pass (search+match): {:>6.1} ns/iter",
        two_pass.as_nanos() as f64 / n as f64
    );
}
