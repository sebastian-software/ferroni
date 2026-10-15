//! `\X` outside look-behinds runs as one grapheme-boundary instruction
//! (ADR-008). Its matches are upstream's, and it pushes no stack entries, so
//! a match stack limit that stops upstream's loop does not stop it (ADR-017).

use ferroni::api::{Regex, SearchOptions};

fn clusters(text: &str) -> Vec<&str> {
    Regex::new(r"\X")
        .unwrap()
        .find_iter(text)
        .map(|m| &text[m.start()..m.end()])
        .collect()
}

#[test]
fn clusters_follow_the_extended_grapheme_rules() {
    assert_eq!(clusters("e\u{301}x"), ["e\u{301}", "x"]);
    assert_eq!(clusters("a\r\nb"), ["a", "\r\n", "b"]);
    assert_eq!(
        clusters("\u{1f468}\u{200d}\u{1f469}!"),
        ["\u{1f468}\u{200d}\u{1f469}", "!"]
    );
    assert_eq!(
        clusters("\u{1f1e9}\u{1f1ea}\u{1f1eb}\u{1f1f7}"),
        ["\u{1f1e9}\u{1f1ea}", "\u{1f1eb}\u{1f1f7}"]
    );
    assert_eq!(
        clusters("\u{1100}\u{1161}\u{11a8}"),
        ["\u{1100}\u{1161}\u{11a8}"]
    );
    assert_eq!(clusters(""), Vec::<&str>::new());
}

#[test]
fn a_long_cluster_needs_no_stack_entries() {
    let text = format!("e{}x", "\u{301}".repeat(5000));
    let re = Regex::new(r"\X").unwrap();
    let unlimited = re.find_with(&text, SearchOptions::new()).unwrap().unwrap();
    assert_eq!(unlimited.end(), "e".len() + 2 * 5000);
    // Upstream's loop pushes an entry per combining mark and stops at this
    // limit. Here the cluster is found without the limit, which ADR-017 allows
    // where C stops.
    let limited = re
        .find_with(&text, SearchOptions::new().match_stack_limit(100))
        .unwrap()
        .unwrap();
    assert_eq!(limited.range(), unlimited.range());
}
