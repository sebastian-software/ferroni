//! Everyday regex tasks on realistic text: markup, logs, chat with emoji,
//! Markdown, JSON, CSV and source code. Each case collects every match with
//! its captures over a document of a few dozen kilobytes, or validates a batch
//! of inputs, so the timings measure matching rather than call overhead.
//!
//! The documents are generated deterministically. Every engine is validated
//! against C Oniguruma before it is timed; an engine that rejects a pattern or
//! answers differently is reported as UNSUPPORTED instead.

use std::fmt::Write;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use ferroni::ffi;
use ferroni::oniguruma::ONIG_OPTION_NONE;

use super::super::engines::{self, Compiled};
use super::super::{c_compile, configure_battle_group, regex_compile, rust_compile};
use super::{c_trace, engine_trace, regex_trace, rust_trace, trace_engines};

#[derive(Clone, Copy, PartialEq)]
enum Syntax {
    /// The `regex` crate runs the pattern too.
    Shared,
    /// Lookaround, backreferences, possessive or atomic groups, `\X`.
    Oniguruma,
}

#[derive(Clone, Copy)]
enum Corpus {
    Html,
    Log,
    Chat,
    Markdown,
    Json,
    Csv,
    Code,
}

struct Task {
    name: &'static str,
    syntax: Syntax,
    corpus: Corpus,
    pattern: String,
    /// Guards the workload: the document must keep at least this many matches.
    min_matches: usize,
}

const EMAIL: &str = r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}";

fn tasks() -> Vec<Task> {
    use Corpus::*;
    use Syntax::*;
    let task = |name, syntax, corpus, pattern: &str, min_matches| Task {
        name,
        syntax,
        corpus,
        pattern: pattern.to_owned(),
        min_matches,
    };
    vec![
        task(
            "html_tags",
            Shared,
            Html,
            r"<([A-Za-z][A-Za-z0-9-]*)\b[^<>]*>",
            1000,
        ),
        task(
            "html_attributes",
            Shared,
            Html,
            r#"([A-Za-z_:][-A-Za-z0-9_:.]*)="([^"]*)""#,
            1000,
        ),
        task("html_comments", Shared, Html, r"<!--[\s\S]*?-->", 100),
        task(
            "hex_colors",
            Shared,
            Html,
            r"#(?:[0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b",
            200,
        ),
        task("email_addresses", Shared, Html, EMAIL, 100),
        task(
            "emoji",
            Shared,
            Chat,
            r"\p{Regional_Indicator}{2}|\p{Extended_Pictographic}\x{FE0F}?\p{Emoji_Modifier}?(?:\x{200D}\p{Extended_Pictographic}\x{FE0F}?\p{Emoji_Modifier}?)*",
            300,
        ),
        task(
            "ascii_emoticons",
            Shared,
            Chat,
            r"[:;=8][-o*']?[)\](\[dDpP/\\|]|<3",
            300,
        ),
        task(
            "hashtags_mentions",
            Shared,
            Chat,
            r"[#@][\p{L}\p{N}_]+",
            300,
        ),
        task(
            "ipv4_addresses",
            Shared,
            Log,
            r"\b(?:(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\b",
            300,
        ),
        task(
            "iso_timestamps",
            Shared,
            Log,
            r"\b([0-9]{4})-([0-9]{2})-([0-9]{2})T([0-9]{2}):([0-9]{2}):([0-9]{2})Z\b",
            300,
        ),
        task(
            "log_keywords_ignorecase",
            Shared,
            Log,
            r"(?i)\b(?:error|warning|fatal|timeout)\b",
            100,
        ),
        task(
            "markdown_links",
            Shared,
            Markdown,
            r#"\[([^\]]+)\]\(([^)\s]+)(?:\s+"([^"]*)")?\)"#,
            200,
        ),
        task(
            "semantic_versions",
            Shared,
            Markdown,
            r"\bv?([0-9]+)\.([0-9]+)\.([0-9]+)(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?\b",
            200,
        ),
        task("json_strings", Shared, Json, r#""(?:[^"\\]|\\.)*""#, 1000),
        task("csv_fields", Shared, Csv, r#""(?:[^"]|"")*"|[^,\n]+"#, 1000),
        task(
            "html_element_pairs",
            Oniguruma,
            Html,
            r"<([a-z][a-z0-9]*)\b[^>]*>(.*?)</\1>",
            300,
        ),
        task(
            "html_attribute_values",
            Oniguruma,
            Html,
            r#"\b([a-z-]+)=(?:"[^"]*+"|'[^']*+'|[^\s>]++)"#,
            1000,
        ),
        task(
            "prices_lookbehind",
            Oniguruma,
            Html,
            r"(?<=[$€£])[0-9]+(?:[.,][0-9]{2})?",
            200,
        ),
        task(
            "quoted_strings",
            Oniguruma,
            Code,
            r#"(["'])(?:\\.|(?!\1)[^\\\n])*\1"#,
            300,
        ),
        task(
            "camel_case_words",
            Oniguruma,
            Code,
            r"[A-Z]?[a-z]+|[A-Z]+(?![a-z])|[0-9]+",
            1000,
        ),
        task(
            "markdown_emphasis",
            Oniguruma,
            Markdown,
            r"(?<![\w*])\*(?!\s)([^*\n]+?)(?<!\s)\*(?![\w*])",
            100,
        ),
        task(
            "emoji_graphemes",
            Oniguruma,
            Chat,
            r"(?=\p{Extended_Pictographic}|\p{Regional_Indicator})\X",
            300,
        ),
        // Patterns at the limit of what the engines handle.
        task("rfc5322_emails", Shared, Html, RFC5322_EMAIL, 200),
        task("ipv6_addresses", Shared, Log, IPV6, 100),
        task("rfc3986_urls", Shared, Html, RFC3986_URL, 200),
        task(
            "keyword_alternation_200",
            Shared,
            Code,
            &keyword_alternation(),
            3000,
        ),
        task(
            "csv_last_column_backtracking",
            Shared,
            Csv,
            r"(?m)^(?:[^,\n]*,){6}([^,\n]*)$",
            300,
        ),
        task(
            "unicode_case_folding",
            Shared,
            Chat,
            r"(?i)\b(?:straße|grüße|ελιά|привет|café)\b",
            100,
        ),
        task(
            "json_objects_recursive",
            Oniguruma,
            Json,
            r#"\{(?:[^{}"]++|"(?:[^"\\]|\\.)*+"|\g<0>)*+\}"#,
            250,
        ),
        task(
            "html_nested_divs_recursive",
            Oniguruma,
            Html,
            r"<div\b[^>]*>(?:[^<]++|<(?!/?div\b)|\g<0>)*</div>",
            100,
        ),
        task(
            "variable_lookbehind",
            Oniguruma,
            Html,
            r"(?<=\b(?:price|total):\s{0,3})[$€£]?[0-9]+(?:[.,][0-9]{2})?",
            200,
        ),
    ]
}

/// The RFC 5322 address grammar as one expression, case-insensitive.
const RFC5322_EMAIL: &str = r#"(?i)(?:[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)*|"(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21\x23-\x5b\x5d-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])*")@(?:(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?|\[(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?|[a-z0-9-]*[a-z0-9]:(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21-\x5a\x53-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])+)\])"#;

/// Every IPv6 notation, including `::` compression, as one alternation.
const IPV6: &str = r"(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|(?:[0-9a-fA-F]{1,4}:){1,7}:|(?:[0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|(?:[0-9a-fA-F]{1,4}:){1,5}(?::[0-9a-fA-F]{1,4}){1,2}|(?:[0-9a-fA-F]{1,4}:){1,4}(?::[0-9a-fA-F]{1,4}){1,3}|(?:[0-9a-fA-F]{1,4}:){1,3}(?::[0-9a-fA-F]{1,4}){1,4}|(?:[0-9a-fA-F]{1,4}:){1,2}(?::[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:(?::[0-9a-fA-F]{1,4}){1,6}|:(?:(?::[0-9a-fA-F]{1,4}){1,7}|:)";

/// A URL with every RFC 3986 component: scheme, user info, host (name, IPv4
/// or bracketed IPv6), port, path, query and fragment.
const RFC3986_URL: &str = r"\b(?:https?|ftp|wss?)://(?:[A-Za-z0-9._~%!$&'()*+,;=:-]+@)?(?:\[[0-9A-Fa-f:.]+\]|(?:[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?\.)+[A-Za-z]{2,63}|(?:[0-9]{1,3}\.){3}[0-9]{1,3})(?::[0-9]{1,5})?(?:/[A-Za-z0-9._~%!$&'()*+,;=:@-]*)*(?:\?[A-Za-z0-9._~%!$&'()*+,;=:@/?-]*)?(?:#[A-Za-z0-9._~%!$&'()*+,;=:@/?-]*)?";

/// 200 words, as a syntax highlighter or a filter list would search them.
fn keyword_alternation() -> String {
    const WORDS: &str = "abstract arguments async await boolean break byte case catch char class const continue \
debugger default delete do double else enum eval export extends false final finally float for function goto if \
implements import in instanceof int interface let long native new null package private protected public return \
short static super switch synchronized this throw throws transient true try typeof var void volatile while with \
yield a abbr address area article aside audio b base bdi bdo blockquote body br button canvas caption cite code \
col colgroup data datalist dd del details dfn dialog div dl dt em embed fieldset figcaption figure footer form h1 \
h2 h3 h4 h5 h6 head header hgroup hr html i iframe img input ins kbd label legend li link main map mark menu meta \
meter nav noscript object ol optgroup option output p param picture pre progress q rp rt ruby s samp script \
section select slot small source span strong style sub summary sup table tbody td template textarea tfoot th \
thead time title tr track u ul video wbr fetch user profile response request client message label quoted parse \
json utf8 http get post put patch options head trace connect status header cookie session token cache query \
result error value";
    let words: Vec<_> = WORDS.split_whitespace().take(200).collect();
    assert_eq!(words.len(), 200);
    format!(r"\b(?:{})\b", words.join("|"))
}

/// Passwords for a rule check: a digit, a lower and an upper case letter, a
/// symbol, and 8 to 64 characters.
const PASSWORD_RULE: &str = r"\A(?=.*[0-9])(?=.*[a-z])(?=.*[A-Z])(?=.*[^0-9A-Za-z\s]).{8,64}\z";

const NAMES: [&str; 8] = [
    "Café Crème",
    "Smørrebrød",
    "Ελιά",
    "抹茶 Latte",
    "Jalapeño Dip",
    "Croissant",
    "Привет Box",
    "Ümlaut Tee",
];
const EMOJI: [&str; 10] = ["👍🏽", "👨‍👩‍👧‍👦", "🇩🇪", "❤️", "😂", "🚀", "🧑🏻‍💻", "🎉", "🇯🇵", "☕"];
const EMOTICONS: [&str; 6] = [":)", ";-)", ":D", "<3", ":-(", "=P"];
const WORDS: [&str; 8] = [
    "release", "deploy", "Grüße", "naïve", "数据", "ready", "build", "error",
];

/// A small deterministic generator, so the documents never change.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) % bound as u64) as usize
    }
}

fn document(corpus: Corpus) -> String {
    let mut out = String::new();
    let mut rng = Lcg(0x005e_edf3_0221);
    match corpus {
        Corpus::Html => {
            for i in 0..120 {
                let name = NAMES[i % NAMES.len()];
                let (dollars, cents) = (5 + rng.next(400), rng.next(100));
                let color = rng.next(0x100_0000);
                writeln!(
                    out,
                    "<!-- card {i}: {name} -->\n\
<article class=\"product card-{c}\" id=\"p{i}\" data-price=\"{dollars}.{cents:02}\" data-tags='new sale'>\n  \
<h2 class=\"title\"><a href=\"https://shop.example.com/p/{i}?ref=home&amp;v=2\" title=\"{name}\">{name} № {i}</a></h2>\n  \
<p style=\"color: #{color:06x}; background: #{short:03x}\">Only ${dollars}.{cents:02} or €{euro},{cents:02} — {emoji} <em>limited</em> offer, <b>{word}</b>.</p>\n  \
<img src=\"/img/{i}.webp\" alt=\"{name}\" width=320 height=240 loading=lazy>\n  \
<ul><li>Contact: <a href=\"mailto:sales{i}@example.org\">sales{i}@example.org</a></li><li>Ships in {days} days</li></ul>\n  \
<div class=\"wrap\"><div class=\"inner\"><span>price: ${dollars}.{cents:02}</span> <span>total:  €{euro},{cents:02}</span></div> <div>ftp://mirror.example.net:2121/pub/{i}/</div></div>\n\
</article>",
                    c = i % 7,
                    short = color >> 12,
                    euro = dollars * 9 / 10,
                    emoji = EMOJI[rng.next(EMOJI.len())],
                    word = WORDS[rng.next(WORDS.len())],
                    days = 1 + rng.next(9),
                )
                .unwrap();
            }
        }
        Corpus::Log => {
            const LEVELS: [&str; 6] = ["INFO", "INFO", "WARN", "Error", "DEBUG", "FATAL"];
            for i in 0..400 {
                let status = [200, 200, 201, 304, 404, 500][rng.next(6)];
                writeln!(
                    out,
                    "2026-09-{d:02}T{h:02}:{m:02}:{s:02}Z 10.{a}.{b}.{c} {level} [api-{svc}] GET /api/v{v}/items/{i}?q={word} {status} {ms}ms{extra} ua=\"Mozilla/5.0 (X11; Linux x86_64) Firefox/131.0\" build=v2.{v}.{i}{v6}",
                    d = 1 + rng.next(28),
                    h = rng.next(24),
                    m = rng.next(60),
                    s = rng.next(60),
                    a = rng.next(256),
                    b = rng.next(256),
                    c = 1 + rng.next(254),
                    level = LEVELS[rng.next(LEVELS.len())],
                    svc = rng.next(5),
                    v = 1 + rng.next(2),
                    word = WORDS[rng.next(WORDS.len())],
                    ms = rng.next(2000),
                    extra = if i % 9 == 0 { " upstream TIMEOUT after 30s" } else if i % 13 == 0 { " warning: retrying" } else { "" },
                    v6 = match i % 4 {
                        0 => format!(" peer=2001:db8:{:x}::{:x}", rng.next(0xffff), 1 + rng.next(0xfffe)),
                        1 => format!(" peer=fe80::{:x}:{:x}ff:fe{:02x}:{:x}", rng.next(0xffff), rng.next(0xff), rng.next(0xff), rng.next(0xffff)),
                        2 => format!(" peer=2001:0db8:85a3:0000:0000:8a2e:0370:{:04x}", rng.next(0xffff)),
                        _ => String::new(),
                    },
                )
                .unwrap();
            }
        }
        Corpus::Chat => {
            const USERS: [&str; 6] = ["anna", "björn", "chen_li", "dev0ps", "émile", "kai"];
            for i in 0..300 {
                writeln!(
                    out,
                    "[{h:02}:{m:02}] @{user}: {word} looks {state} {e1} {emoticon} — thanks @{other}! #{tag}{i} {e2}",
                    h = rng.next(24),
                    m = rng.next(60),
                    user = USERS[rng.next(USERS.len())],
                    word = WORDS[rng.next(WORDS.len())],
                    state = ["great", "broken", "fine", "très bien", "完璧", "on STRASSE 5", "GRÜSSE", "in der Straße"][rng.next(8)],
                    e1 = EMOJI[rng.next(EMOJI.len())],
                    emoticon = EMOTICONS[rng.next(EMOTICONS.len())],
                    other = USERS[rng.next(USERS.len())],
                    tag = ["release", "bug", "Grüße", "ship"][rng.next(4)],
                    e2 = EMOJI[rng.next(EMOJI.len())],
                )
                .unwrap();
            }
        }
        Corpus::Markdown => {
            for i in 0..200 {
                writeln!(
                    out,
                    "## Section {i}\n\n\
See [the docs](https://docs.example.com/guide/{i} \"Guide {i}\") and [*issue* #{i}](https://github.com/org/repo/issues/{i}). \
Use **bold**, *emphasis* and `code_{i}`, but not snake_case_words or 2*3*4. \
Requires v1.{minor}.{patch}-beta.{n}+build.{i} or 2.0.{patch}; *{word}* matters.\n",
                    minor = rng.next(20),
                    patch = rng.next(10),
                    n = rng.next(5),
                    word = WORDS[rng.next(WORDS.len())],
                )
                .unwrap();
            }
        }
        Corpus::Json => {
            out.push('[');
            for i in 0..250 {
                write!(
                    out,
                    "{sep}\n  {{\"id\": {i}, \"name\": \"{name} \\\"{i}\\\"\", \"path\": \"C:\\\\data\\\\{i}.json\", \"tags\": [\"new\", \"caf\\u00e9\"], \"price\": {p}.{c:02}, \"meta\": {{\"dims\": {{\"w\": {w}, \"h\": 4}}, \"flags\": [{{\"x\": true}}]}}, \"note\": \"line\\nbreak {word}\"}}",
                    sep = if i == 0 { "" } else { "," },
                    name = NAMES[i % NAMES.len()],
                    p = rng.next(500),
                    c = rng.next(100),
                    w = rng.next(9),
                    word = WORDS[rng.next(WORDS.len())],
                )
                .unwrap();
            }
            out.push_str("\n]\n");
        }
        Corpus::Csv => {
            out.push_str("id,name,comment,price,date,note\n");
            for i in 0..400 {
                writeln!(
                    out,
                    "{i},\"{name}, {i}\",\"She said \"\"{word}\"\"\",{p}.{c:02},2026-09-{d:02},plain {word}",
                    name = NAMES[i % NAMES.len()],
                    word = WORDS[rng.next(WORDS.len())],
                    p = rng.next(500),
                    c = rng.next(100),
                    d = 1 + rng.next(28),
                )
                .unwrap();
            }
        }
        Corpus::Code => {
            for i in 0..300 {
                writeln!(
                    out,
                    "const fetchUserProfile{i} = async (userId, httpClient) => {{ const label = 'It\\'s {word}' + \"a \\\"quoted\\\" HTTPResponse\"; return parseJSONResponse(await httpClient.get(`/u/${{userId}}`), 'utf8'); }};",
                    word = WORDS[rng.next(WORDS.len())],
                )
                .unwrap();
            }
        }
    }
    out
}

fn passwords() -> Vec<(String, bool)> {
    let mut rng = Lcg(0xfeed_0042);
    (0..64)
        .map(|i| {
            let base = [
                "correct horse",
                "Tr0ub4dor&3",
                "hunter2",
                "P@ssw0rd!",
                "ÄpfelBäume9#",
                "short1A!",
            ][i % 6];
            let candidate = format!("{base}{}", rng.next(1000));
            let valid = candidate.chars().count() >= 8
                && candidate.chars().count() <= 64
                && candidate.chars().any(|c| c.is_ascii_digit())
                && candidate.chars().any(|c| c.is_ascii_lowercase())
                && candidate.chars().any(|c| c.is_ascii_uppercase())
                && candidate
                    .chars()
                    .any(|c| !c.is_ascii_alphanumeric() && !c.is_whitespace());
            (candidate, valid)
        })
        .collect()
}

pub fn bench_regex_tasks(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_tasks");
    configure_battle_group(&mut group);
    for task in tasks() {
        let text = document(task.corpus);
        let rust = rust_compile(task.pattern.as_bytes(), ONIG_OPTION_NONE);
        let c_regex = c_compile(task.pattern.as_bytes(), ffi::ONIG_OPTION_NONE);
        let expected = c_trace(&c_regex, text.as_bytes());
        assert!(
            expected.len() >= task.min_matches,
            "{}: {} matches, expected at least {}",
            task.name,
            expected.len(),
            task.min_matches
        );
        assert_eq!(
            rust_trace(&rust, text.as_bytes()),
            expected,
            "{}",
            task.name
        );
        println!(
            "WORKLOAD {}",
            serde_json::json!({
                "name": task.name,
                "pattern": task.pattern,
                "syntax": if task.syntax == Syntax::Shared { "shared" } else { "oniguruma" },
                "bytes": text.len(),
                "matches": expected.len(),
                "boundary": "all matches and raw capture bounds; materialized result vectors",
            })
        );
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_function(BenchmarkId::new("rust", task.name), |b| {
            b.iter(|| black_box(rust_trace(&rust, black_box(text.as_bytes()))));
        });
        group.bench_function(BenchmarkId::new("c", task.name), |b| {
            b.iter(|| black_box(c_trace(&c_regex, black_box(text.as_bytes()))));
        });
        if task.syntax == Syntax::Shared {
            let regex = regex_compile(task.pattern.as_bytes(), false);
            let actual = regex_trace(&regex, text.as_bytes());
            if actual == expected {
                group.bench_function(BenchmarkId::new("regex", task.name), |b| {
                    b.iter(|| black_box(regex_trace(&regex, black_box(text.as_bytes()))));
                });
            } else {
                engines::unsupported_engine(
                    &format!("regex_tasks/regex/{}", task.name),
                    "regex",
                    &trace_difference(&actual, &expected),
                );
            }
        }
        for (engine, compiled) in
            trace_engines("regex_tasks", task.name, &task.pattern, &text, &expected)
        {
            group.bench_function(BenchmarkId::new(engine.id(), task.name), |b| {
                b.iter(|| black_box(engine_trace(&compiled, black_box(&text)).unwrap()));
            });
        }
    }

    let inputs = passwords();
    let rust = rust_compile(PASSWORD_RULE.as_bytes(), ONIG_OPTION_NONE);
    let c_regex = c_compile(PASSWORD_RULE.as_bytes(), ffi::ONIG_OPTION_NONE);
    for (text, valid) in &inputs {
        let c_match =
            c_regex.search(text.as_bytes(), 0, text.len(), None, ffi::ONIG_OPTION_NONE) >= 0;
        assert_eq!(c_match, *valid, "password rule, C: {text}");
        assert_eq!(
            super::super::rust_search(&rust, text.as_bytes(), None).0 >= 0,
            *valid
        );
    }
    let accepts = |compiled: &Compiled| {
        for (text, valid) in &inputs {
            if compiled.search(text, 0)?.is_some() != *valid {
                return Err(format!("{text:?}: expected {valid}"));
            }
        }
        Ok(())
    };
    group.throughput(Throughput::Elements(inputs.len() as u64));
    group.bench_function(BenchmarkId::new("rust", "password_rules"), |b| {
        b.iter(|| {
            black_box(&inputs)
                .iter()
                .filter(|(text, _)| super::super::rust_search(&rust, text.as_bytes(), None).0 >= 0)
                .count()
        })
    });
    group.bench_function(BenchmarkId::new("c", "password_rules"), |b| {
        b.iter(|| {
            black_box(&inputs)
                .iter()
                .filter(|(text, _)| {
                    c_regex.search(text.as_bytes(), 0, text.len(), None, ffi::ONIG_OPTION_NONE) >= 0
                })
                .count()
        })
    });
    for (engine, compiled) in engines::validated(
        "regex_tasks",
        "password_rules",
        PASSWORD_RULE,
        false,
        accepts,
    ) {
        group.bench_function(BenchmarkId::new(engine.id(), "password_rules"), |b| {
            b.iter(|| {
                black_box(&inputs)
                    .iter()
                    .filter(|(text, _)| matches!(compiled.search(text, 0), Ok(Some(_))))
                    .count()
            })
        });
    }
    group.finish();
}

fn trace_difference(actual: &super::Trace, expected: &super::Trace) -> String {
    match actual.iter().zip(expected).position(|(a, e)| a != e) {
        Some(i) => format!("match {i}: {:?}, Oniguruma {:?}", actual[i], expected[i]),
        None => format!("{} matches, Oniguruma {}", actual.len(), expected.len()),
    }
}
