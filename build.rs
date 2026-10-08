// build.rs -- Compile upstream Oniguruma sources for `ffi` benchmarks.

fn main() {
    // Allow the `coverage_nightly` cfg used by #[cfg_attr(coverage_nightly, coverage(off))].
    // This silences "unexpected cfg" warnings on stable while activating on nightly+coverage.
    println!("cargo::rustc-check-cfg=cfg(coverage_nightly)");

    // Doc-test the guide pages (src/lib.rs) only where they are present. `docs/`
    // is excluded from the published crate, so `cargo test --doc` on a packaged
    // copy must not try to include them.
    println!("cargo::rustc-check-cfg=cfg(ferroni_guide_docs)");
    if guide_pages_present() {
        println!("cargo::rustc-cfg=ferroni_guide_docs");
    }

    #[cfg(feature = "ffi")]
    build_oniguruma_c();

    #[cfg(feature = "onigmo")]
    build_onigmo();
}

fn guide_pages_present() -> bool {
    let manifest_dir = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    ["getting-started", "untrusted-input", "compatibility"]
        .iter()
        .all(|page| {
            manifest_dir
                .join("docs/app/routes/guide")
                .join(format!("{page}.mdx"))
                .is_file()
        })
}

#[cfg(feature = "ffi")]
fn build_oniguruma_c() {
    use std::env;
    use std::path::PathBuf;

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let oniguruma_dir = resolve_oniguruma_dir();
    let src_dir = oniguruma_dir.join("src");

    // Generate config.h
    let pointer_size = env::var("CARGO_CFG_TARGET_POINTER_WIDTH")
        .unwrap()
        .parse::<usize>()
        .unwrap()
        / 8;

    let config_h = format!(
        r#"
#ifndef CONFIG_H
#define CONFIG_H

#define HAVE_STDINT_H 1
#define HAVE_INTTYPES_H 1
#define HAVE_STDLIB_H 1
#define HAVE_STRING_H 1
#define HAVE_SYS_TYPES_H 1
#define HAVE_SYS_STAT_H 1
#define HAVE_UNISTD_H 1
#define HAVE_MEMORY_H 1
#define HAVE_STRINGS_H 1
#define STDC_HEADERS 1

#define SIZEOF_INT 4
#define SIZEOF_LONG {long_size}
#define SIZEOF_LONG_LONG 8
#define SIZEOF_VOIDP {pointer_size}

#define PACKAGE "onig"
#define PACKAGE_VERSION "6.9.4"
#define VERSION "6.9.4"

#endif
"#,
        long_size = if cfg!(target_os = "windows") {
            4
        } else {
            pointer_size
        },
        pointer_size = pointer_size,
    );
    std::fs::write(out_dir.join("config.h"), config_h).unwrap();

    // C source files (matches CMakeLists.txt lines 59-72 exactly).
    // Note: unicode_egcb_data.c, unicode_wb_data.c, unicode_fold_data.c,
    // unicode_property_data.c, unicode_property_data_posix.c are #include'd
    // by unicode.c and must NOT be compiled as separate translation units.
    let c_sources = [
        "regerror.c",
        "regparse.c",
        "regext.c",
        "regcomp.c",
        "regexec.c",
        "reggnu.c",
        "regenc.c",
        "regsyntax.c",
        "regtrav.c",
        "regversion.c",
        "st.c",
        "onig_init.c",
        "unicode.c",
        "ascii.c",
        "utf8.c",
        "utf16_be.c",
        "utf16_le.c",
        "utf32_be.c",
        "utf32_le.c",
        "euc_jp.c",
        "sjis.c",
        "iso8859_1.c",
        "iso8859_2.c",
        "iso8859_3.c",
        "iso8859_4.c",
        "iso8859_5.c",
        "iso8859_6.c",
        "iso8859_7.c",
        "iso8859_8.c",
        "iso8859_9.c",
        "iso8859_10.c",
        "iso8859_11.c",
        "iso8859_13.c",
        "iso8859_14.c",
        "iso8859_15.c",
        "iso8859_16.c",
        "euc_tw.c",
        "euc_kr.c",
        "big5.c",
        "gb18030.c",
        "koi8_r.c",
        "cp1251.c",
        "euc_jp_prop.c",
        "sjis_prop.c",
        "unicode_unfold_key.c",
        "unicode_fold1_key.c",
        "unicode_fold2_key.c",
        "unicode_fold3_key.c",
    ];

    let mut build = cc::Build::new();
    build
        .opt_level(3)
        .include(&src_dir)
        .include(&out_dir) // for config.h
        .define("HAVE_CONFIG_H", None)
        .define("ONIG_STATIC", None)
        .define("ONIG_EXTERN", Some("extern"));

    for file in &c_sources {
        build.file(src_dir.join(file));
    }

    // Also compile the vscode-oniguruma scanner wrapper (for benchmarks).
    // The file is excluded from the published crate (benches/ is not
    // packaged), so skip it when building from crates.io — only the
    // in-repo benchmarks link against it.
    let bench_wrapper = std::path::Path::new("benches/vscode_scanner_native.c");
    if bench_wrapper.exists() {
        build.file(bench_wrapper);
    }

    build.compile("oniguruma");
}

#[cfg(feature = "ffi")]
fn resolve_oniguruma_dir() -> std::path::PathBuf {
    use std::env;
    use std::path::PathBuf;

    const DEFAULT_ONIGURUMA_DIR: &str = ".cache/upstream/oniguruma-orig";
    const LEGACY_ONIGURUMA_DIR: &str = "oniguruma-orig";

    println!("cargo:rerun-if-env-changed=FERRONI_ONIGURUMA_DIR");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=scripts/prepare-oniguruma-sources.sh");

    if let Ok(path) = env::var("FERRONI_ONIGURUMA_DIR") {
        let dir = PathBuf::from(path);
        if dir.join("src").is_dir() {
            println!("cargo:rerun-if-changed={}", dir.join("src").display());
            return dir;
        }
        panic!("FERRONI_ONIGURUMA_DIR must point to an Oniguruma checkout root containing `src/`.");
    }

    for candidate in [DEFAULT_ONIGURUMA_DIR, LEGACY_ONIGURUMA_DIR] {
        let dir = PathBuf::from(candidate);
        if dir.join("src").is_dir() {
            println!("cargo:rerun-if-changed={}", dir.join("src").display());
            return dir;
        }
    }

    panic!(
        "ffi benchmarks require local Oniguruma sources.\n\
Run `./scripts/prepare-oniguruma-sources.sh` first,\n\
or set FERRONI_ONIGURUMA_DIR=/path/to/oniguruma."
    );
}

/// Ruby's Onigmo for the engine comparison benchmarks. Onigmo exports the same
/// symbol names as Oniguruma (`onig_new`, `onig_search`, its own `st.c`), so a
/// first pass compiles it to objects, `nm` lists every symbol it defines, and
/// the second pass compiles it with a header that prefixes all of them.
#[cfg(feature = "onigmo")]
fn build_onigmo() {
    use std::fmt::Write;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    println!("cargo:rerun-if-env-changed=FERRONI_ONIGMO_DIR");
    println!("cargo:rerun-if-changed=benches/onigmo_shim.c");
    let dir = PathBuf::from(
        std::env::var("FERRONI_ONIGMO_DIR")
            .unwrap_or_else(|_| ".cache/upstream/onigmo-ruby".to_owned()),
    );
    assert!(
        dir.join("regexec.c").is_file(),
        "the onigmo feature needs Onigmo sources.\n\
Run `./scripts/prepare-onigmo-sources.sh` first, or set FERRONI_ONIGMO_DIR."
    );
    println!("cargo:rerun-if-changed={}", dir.display());
    let shim = Path::new("benches/onigmo_shim.c");
    assert!(
        shim.is_file(),
        "the onigmo feature only serves the in-repository benchmarks"
    );
    let unicode = std::fs::read_dir(dir.join("enc/unicode"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.join("casefold.h").is_file())
        .expect("enc/unicode/<version>/casefold.h");

    // Ruby's build normally supplies these; the regex sources need only a few.
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("onigmo");
    std::fs::create_dir_all(out.join("internal")).unwrap();
    // LP64 or ILP32 Unix targets only; the benchmarks do not run on Windows.
    let pointer = std::env::var("CARGO_CFG_TARGET_POINTER_WIDTH")
        .unwrap()
        .parse::<usize>()
        .unwrap()
        / 8;
    std::fs::write(
        out.join("config.h"),
        format!(
            "#include <stdbool.h>\n#include <stdint.h>\n\
#define HAVE_STDARG_H 1\n#define HAVE_STDLIB_H 1\n#define HAVE_STRING_H 1\n\
#define SIZEOF_INT 4\n#define SIZEOF_LONG {pointer}\n#define SIZEOF_LONG_LONG 8\n\
#define SIZEOF_VOIDP {pointer}\n#define SIZEOF_SIZE_T {pointer}\n\
#define RB_GNUC_EXTENSION __extension__\n\
#define RB_GNUC_EXTENSION_BLOCK(x) __extension__ ({{ x; }})\n\
#define UNREACHABLE_RETURN(v) return (v)\n"
        ),
    )
    .unwrap();
    std::fs::write(
        out.join("internal/sanitizers.h"),
        "#define NO_SANITIZE(sanitizer, declaration) declaration\n",
    )
    .unwrap();

    let sources = [
        "regcomp.c",
        "regenc.c",
        "regerror.c",
        "regexec.c",
        "regparse.c",
        "regsyntax.c",
        "st.c",
        "enc/unicode.c",
        "enc/utf_8.c",
        "enc/ascii.c",
        "enc/us_ascii.c",
    ];
    let build = |rename: Option<&Path>| {
        let mut build = cc::Build::new();
        build
            .opt_level(3)
            .warnings(false)
            .flag_if_supported("-w")
            .flag("-include")
            .flag(out.join("config.h").to_str().unwrap())
            .include(&out)
            .include(&dir)
            .include(dir.join("include/ruby"))
            .include(&unicode);
        if let Some(rename) = rename {
            build.flag("-include").flag(rename.to_str().unwrap());
        }
        for source in sources {
            build.file(dir.join(source));
        }
        build
    };

    let nm = std::env::var("NM").unwrap_or_else(|_| "nm".to_owned());
    let underscore = std::env::var("CARGO_CFG_TARGET_VENDOR").as_deref() == Ok("apple");
    let mut symbols = std::collections::BTreeSet::new();
    for object in build(None).compile_intermediates() {
        let listing = Command::new(&nm)
            .arg("-g")
            .arg(&object)
            .output()
            .expect("nm lists the Onigmo objects");
        assert!(
            listing.status.success(),
            "nm failed on {}",
            object.display()
        );
        for line in String::from_utf8_lossy(&listing.stdout).lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            if let [_, kind, name] = fields[..]
                && kind != "U"
            {
                let name = if underscore {
                    name.strip_prefix('_').unwrap_or(name)
                } else {
                    name
                };
                symbols.insert(name.to_owned());
            }
        }
    }
    assert!(
        symbols.contains("onig_search"),
        "nm found no Onigmo symbols"
    );
    let mut header = String::new();
    for symbol in &symbols {
        writeln!(header, "#define {symbol} ferroni_onigmo__{symbol}").unwrap();
    }
    let rename = out.join("rename.h");
    std::fs::write(&rename, header).unwrap();

    let mut build = build(Some(&rename));
    build.file(shim);
    build.compile("onigmo");
}
