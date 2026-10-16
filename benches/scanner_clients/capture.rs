//! Capture driver, copied to Ferriki's examples/ by capture.py.
use ferriki::{Highlighter, StandardAssetCatalogs, TokenizeOptions};
use std::io::Write;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [assets, fixtures, case] = args.as_slice() else {
        return Err("usage: capture_scanner_client ASSETS FIXTURES tsx|astro|related".into());
    };
    let languages: &[&str] = match case.as_str() {
        "tsx" => &["tsx"],
        "astro" => &["astro"],
        "related" => &["typescript", "tsx"],
        _ => return Err("unknown case".into()),
    };
    let catalogs = StandardAssetCatalogs::load_from_root(Path::new(assets))?;
    let mut highlighter = Highlighter::builder()
        .with_assets(catalogs)
        .load_languages(languages.iter().copied())
        .load_themes(["github-dark"])
        .build()?;
    let options = TokenizeOptions::default().with_time_limit_millis(0);
    for &language in languages {
        let fixture = match language {
            "typescript" => "orders.ts",
            "tsx" => "Orders.tsx",
            "astro" => "Orders.astro",
            _ => unreachable!(),
        };
        let code = std::fs::read_to_string(Path::new(fixtures).join(fixture))?.repeat(16);
        let mut output = std::fs::OpenOptions::new()
            .append(true)
            .open(std::env::var("FERRONI_CLIENT_TRACE")?)?;
        writeln!(
            output,
            "{}",
            serde_json::json!({"kind":"phase", "language":language})
        )?;
        let result =
            highlighter.highlight_with_options(&code, language, "github-dark", &options)?;
        assert!(!result.tokens.is_empty());
    }
    // Explicitly capture the registry's scanner/cache destruction as well.
    drop(highlighter);
    Ok(())
}
