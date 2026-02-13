// build.rs -- Compile C Oniguruma from submodule (gated on `ffi` feature)

fn main() {
    #[cfg(feature = "ffi")]
    build_oniguruma_c();
}

#[cfg(feature = "ffi")]
fn build_oniguruma_c() {
    use std::env;
    use std::path::PathBuf;

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let src_dir = PathBuf::from("oniguruma-orig/src");

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

    build.compile("oniguruma");
}
