use provider_api::format_testing::{
    assert_file_names, assert_formats, assert_lossless, check_corpus,
};

#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_procfile::provider(), "Procfile", input)
}

/// Every `Procfile` of the pinned corpus (`tests/conformance/fetch-procfile-corpus.sh`) formats with
/// the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-procfile --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    check_corpus(lang_procfile::provider(), "Procfile", "procfile", &[]);
}

#[test]
fn matches_the_file_names() {
    assert_file_names(
        &lang_procfile::provider(),
        &["Procfile", "app/Procfile", "Procfile.dev", "Procfile.local"],
        &["procfile", "MyProcfile", "Procfile/x"],
    );
}

#[test]
fn one_blank_after_the_colon_none_before() {
    assert_eq!(
        fmt("web:bundle exec puma\nworker :   sidekiq\nclock   :\tcron\n"),
        "web: bundle exec puma\nworker: sidekiq\nclock: cron\n"
    );
}

#[test]
fn trims_blanks_and_collapses_blank_lines() {
    assert_eq!(
        fmt("\n\n# top  \nweb: a b   \n\n\n\nworker: c\n\n\n"),
        "# top\nweb: a b\n\nworker: c\n"
    );
}

#[test]
fn keeps_what_is_not_layout() {
    // The command is shell text: `#` and `:` inside it stay, the comment text is as written.
    let input = "web: sh -c 'a: b # c'\n#no space\nlogs:\nhttp\n";
    assert_eq!(fmt(input), input);
}

#[test]
fn an_indented_line_stays_as_written() {
    // Runners skip it; un-indenting would start the process.
    let input = "web: a\n  worker: b   \n\tclock : c\n";
    assert_eq!(fmt(input), "web: a\n  worker: b\n\tclock : c\n");
}

#[test]
fn final_newline_as_written() {
    assert_eq!(fmt("web: a"), "web: a");
    assert_eq!(fmt("web: a\n"), "web: a\n");
}

#[test]
fn crlf_becomes_lf() {
    assert_eq!(fmt("web: a\r\nworker: b\r\n"), "web: a\nworker: b\n");
}

#[test]
fn lines_are_entries_comments_or_strays() {
    let r = assert_lossless(
        &lang_procfile::provider(),
        "Procfile",
        "# c\nweb: a\n: x\nplain\n  w: a\n",
    );
    assert!(r.errors.is_empty());
    let kinds: Vec<_> = r
        .tree
        .children()
        .map(|n| {
            core_syntax::grammars::PROCFILE
                .get()
                .name(n.kind())
                .to_string()
        })
        .collect();
    assert_eq!(kinds, ["Process", "Stray", "Stray", "Stray"]);
}
