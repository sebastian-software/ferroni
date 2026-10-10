//! Capture driver for the JSON and Astro scanner replays, placed as
//! `examples/trace_curated.rs` in a Ferriki checkout with `capture.patch`
//! applied (see README.md). It highlights one curated fixture the way the
//! paired native benchmark does (`github-dark`, `tokenizeTimeLimit` 500,
//! the fixture repeated `copies` times for the `large` size) and records
//! every scanner call of the second, warm pass into a JSONL trace through
//! the instrumented adapter (`FERRIKI_TRACE`). The `bench` mode times the
//! whole tokenizing pipeline instead, for the pre-filter on against off.
//!
//! ```sh
//! cargo run --release --example trace_curated -- assets/shiki json \
//!   node/benchmarks/curated/fixtures/orders.json 16 /tmp/json.jsonl
//! cargo run --release --example trace_curated -- bench assets/shiki astro \
//!   node/benchmarks/curated/fixtures/Orders.astro 16 40
//! ```

use std::path::Path;

use ferriki::{Highlighter, StandardAssetCatalogs, TokenizeOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `bench ASSET_ROOT LANG FIXTURE COPIES PASSES`: the whole highlighting
    // pipeline (tokens, no HTML), warm, min and median over PASSES.
    if args.first().map(String::as_str) == Some("bench") {
        let [_, asset_root, language, fixture, copies, passes] = args.as_slice() else {
            return Err("usage: trace_curated bench ASSET_ROOT LANG FIXTURE COPIES PASSES".into());
        };
        let copies: usize = copies.parse()?;
        let passes: usize = passes.parse()?;
        let assets = StandardAssetCatalogs::load_from_root(Path::new(asset_root))?;
        let mut highlighter = Highlighter::builder()
            .with_assets(assets)
            .load_languages([language.as_str()])
            .load_themes(["github-dark"])
            .build()?;
        let code = std::fs::read_to_string(fixture)?.repeat(copies);
        let options = TokenizeOptions::default().with_time_limit_millis(500);
        let started = std::time::Instant::now();
        let first = highlighter.highlight_with_options(&code, language, "github-dark", &options)?;
        let first_ms = started.elapsed().as_secs_f64() * 1e3;
        for _ in 0..3 {
            highlighter.highlight_with_options(&code, language, "github-dark", &options)?;
        }
        let mut times = Vec::with_capacity(passes);
        for _ in 0..passes {
            let started = std::time::Instant::now();
            let result =
                highlighter.highlight_with_options(&code, language, "github-dark", &options)?;
            times.push(started.elapsed().as_secs_f64() * 1e3);
            assert_eq!(result.tokens.len(), first.tokens.len());
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{language} x{copies}: first {first_ms:.3} ms; {passes} passes: min {:.3} ms, median {:.3} ms, p90 {:.3} ms",
            times[0],
            times[times.len() / 2],
            times[times.len() * 9 / 10]
        );
        return Ok(());
    }
    let [asset_root, language, fixture, copies, trace] = args.as_slice() else {
        return Err("usage: trace_curated ASSET_ROOT LANG FIXTURE COPIES TRACE.jsonl".into());
    };
    let copies: usize = copies.parse()?;
    let assets = StandardAssetCatalogs::load_from_root(Path::new(asset_root))?;
    let mut highlighter = Highlighter::builder()
        .with_assets(assets)
        .load_languages([language.as_str()])
        .load_themes(["github-dark"])
        .build()?;
    let code = std::fs::read_to_string(fixture)?.repeat(copies);
    let options = TokenizeOptions::default().with_time_limit_millis(500);

    // The cold pass compiles the scanners; the harness renders once for
    // validation and warms up three times before timing.
    let cold = highlighter.highlight_with_options(&code, language, "github-dark", &options)?;
    std::fs::write(trace, "")?;
    // SAFETY: single-threaded driver; nothing else reads the environment.
    unsafe { std::env::set_var("FERRIKI_TRACE", trace) };
    let warm = highlighter.highlight_with_options(&code, language, "github-dark", &options)?;
    // SAFETY: as above.
    unsafe { std::env::remove_var("FERRIKI_TRACE") };
    let cold_tokens: usize = cold.tokens.iter().map(Vec::len).sum();
    let warm_tokens: usize = warm.tokens.iter().map(Vec::len).sum();
    assert_eq!(cold.tokens, warm.tokens, "cold and warm passes tokenize alike");
    let lines = std::fs::read_to_string(trace)?.lines().count();
    println!(
        "{language}: {} bytes, {} lines, {cold_tokens} tokens (warm {warm_tokens}); {lines} scanner calls recorded",
        code.len(),
        code.lines().count(),
    );
    Ok(())
}
