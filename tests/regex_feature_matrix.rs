//! Untimed syntax checks for the public feature matrix, separate from production benchmarks.
#![cfg(all(feature = "ffi", feature = "onigmo", ferroni_guide_docs))]

use ferroni::ffi::{CRegex, CRegion};
use ferroni::prelude::*;
use serde_json::Value;

#[allow(
    dead_code,
    reason = "Feature checks use only the single-pattern benchmark adapters."
)]
#[path = "../benches/engines.rs"]
mod engines;

#[test]
fn comparison_profiles_use_unicode_shorthand_classes() {
    for profile in [engines::Engine::Pcre2, engines::Engine::Pcre2Jit] {
        for (pattern, input, span) in [
            (r"\A\w+\z", "κόσμος", (0, 12)),
            (r"\A\d+\z", "٣٤", (0, 4)),
            (r"\A\s+\z", "\u{2003}", (0, 3)),
        ] {
            let regex = profile.compile(pattern, false, false).unwrap();
            assert_eq!(
                regex.find(input, 0).unwrap(),
                Some(span),
                "{} / {pattern}",
                profile.id()
            );
        }
    }
}

fn compiled_match(
    engine: &str,
    pattern: &str,
    input: &str,
) -> Result<Option<(usize, usize)>, String> {
    match engine {
        "rust" => {
            let regex = Regex::new(pattern).map_err(|error| error.to_string())?;
            Ok(regex
                .find_with(input, SearchOptions::new())
                .expect("execution must succeed")
                .map(|found| (found.start(), found.end())))
        }
        "regex" => {
            let regex = regex::Regex::new(pattern).map_err(|error| error.to_string())?;
            Ok(regex.find(input).map(|found| (found.start(), found.end())))
        }
        "c" => {
            let regex = CRegex::new(pattern.as_bytes(), 0).map_err(|error| error.to_string())?;
            let mut region = CRegion::new();
            let result = regex.search(input.as_bytes(), 0, input.len(), Some(&mut region), 0);
            assert!(result >= -1, "C execution error: {result}");
            Ok((result >= 0).then(|| {
                let (start, end) = region.match_range().expect("a match has bounds");
                (
                    usize::try_from(start).unwrap(),
                    usize::try_from(end).unwrap(),
                )
            }))
        }
        other => {
            let profile = match other {
                "pcre2" => engines::Engine::Pcre2,
                "pcre2_jit" => engines::Engine::Pcre2Jit,
                "fancy_regex" => engines::Engine::Fancy,
                "onigmo" => engines::Engine::Onigmo,
                _ => panic!("Unknown matrix engine: {other}"),
            };
            let regex = profile.compile(pattern, false, false)?;
            Ok(regex.find(input, 0).expect("execution must succeed"))
        }
    }
}

#[test]
fn published_feature_matrix_matches_the_pinned_engines() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/app/data/regex-capabilities.json");
    let matrix: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for feature in matrix["features"].as_array().unwrap() {
        let pattern = feature["pattern"].as_str().unwrap();
        let input = feature["input"].as_str().unwrap();
        let span = feature["span"].as_array().unwrap();
        let expected = Some((
            usize::try_from(span[0].as_u64().unwrap()).unwrap(),
            usize::try_from(span[1].as_u64().unwrap()).unwrap(),
        ));
        for engine in matrix["engines"].as_array().unwrap() {
            let id = engine["id"].as_str().unwrap();
            let supported = feature["support"][id].as_bool().unwrap();
            let profiles: &[&str] = if id == "pcre2" {
                &["pcre2", "pcre2_jit"]
            } else {
                &[id]
            };
            for profile in profiles {
                let result = compiled_match(profile, pattern, input);
                if supported {
                    assert_eq!(result.unwrap(), expected, "{} / {profile}", feature["id"]);
                } else if feature["failures"][id].as_str() == Some("semantic_difference") {
                    assert_ne!(
                        result.unwrap(),
                        expected,
                        "{} / {profile} now matches the required semantics; update the matrix",
                        feature["id"]
                    );
                } else {
                    assert!(
                        result.is_err(),
                        "{} / {profile} now accepts the syntax; update the matrix",
                        feature["id"]
                    );
                }
            }
        }
    }
}
