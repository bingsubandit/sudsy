use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_dockerfile::provider(), "Dockerfile", input)
}

/// Every file of the pinned corpus (.NET samples and images, official images, moby, buildx)
/// parses (but for the grammar gaps below), formats with the same instructions and arguments (case
/// and blanks aside: the safety check) and formats idempotently.
///
/// `cargo test --profile corpus -p lang-dockerfile --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (_, unparsed) = check_corpus(lang_dockerfile::provider(), "Dockerfile", "dockerfile", &[]);
    assert!(unparsed.is_empty(), "{}", unparsed.join("\n"));
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_dockerfile::provider(),
        &[
            "a/Dockerfile",
            "Containerfile",
            "Dockerfile.dev",
            "x.dockerfile",
        ],
        &["a/Dockerfiles", "docker.json", "my-Dockerfile"],
    );
}

#[test]
fn instructions_and_arguments() {
    let out = fmt(
        "# syntax=docker/dockerfile:1\n\n\n\nfrom  alpine:3.20   as build\nARG X=1  Y\nENV A=b   C=\"d e\"\nENV LEGACY  some   value\nCOPY --chown=1:1 a  b /dst/\nCOPY [\"a\",\"b\" , \"/d\"]\nCMD [ \"x\" ]\nUSER a:b\nstopsignal   SIGTERM\nHEALTHCHECK --interval=5s CMD curl -f x || exit 1\nonbuild run echo   hi\n# end",
    );
    assert_eq!(
        out,
        "# syntax=docker/dockerfile:1\n\nFROM alpine:3.20 AS build\nARG X=1 Y\nENV A=b C=\"d e\"\nENV LEGACY  some   value\nCOPY --chown=1:1 a b /dst/\nCOPY [\"a\", \"b\", \"/d\"]\nCMD [\"x\"]\nUSER a:b\nSTOPSIGNAL SIGTERM\nHEALTHCHECK --interval=5s CMD curl -f x || exit 1\nONBUILD RUN echo   hi\n# end\n"
    );
}

#[test]
fn continuations_are_reindented_but_shell_text_stays_without_a_shell() {
    let out = fmt(
        "RUN  apt-get update   \\\n  && apt-get install -y \\\n\n# why\n  curl \\\n  jq\nRUN echo \"a  \\\n   b\" \\\n  c\nCOPY a \\\n  b \\\n   /d\nENV A=1 \\\n    B=2\nRUN \\\n  x\n",
    );
    assert_eq!(
        out,
        "RUN apt-get update   \\\n  && apt-get install -y \\\n# why\n  curl \\\n  jq\nRUN echo \"a  \\\n   b\" \\\n  c\nCOPY a \\\n    b \\\n    /d\nENV A=1 \\\n    B=2\nRUN x\n"
    );
}

#[test]
fn nested_continuation_levels_stay_nested() {
    assert_eq!(
        fmt("RUN a \\\n  && b \\\n      c \\\n\t\td\n"),
        "RUN a \\\n  && b \\\n      c \\\n\t\td\n"
    );
}

#[test]
fn comments_between_continued_arguments_stay() {
    let out = fmt("ENV \\\n  # one\n  A=1 \\\n    # two\n  B=2\n");
    assert_eq!(out, "ENV \\\n    # one\n    A=1 \\\n    # two\n    B=2\n");
}

#[test]
fn the_escape_directive_changes_the_continuation_character() {
    let out = fmt("# escape=`\nFROM x\nRUN a  `\n  b `\n  c\nCOPY a `\n b\nCMD a  `\n b\n");
    assert_eq!(
        out,
        "# escape=`\nFROM x\nRUN a  `\n  b `\n  c\nCOPY a `\n    b\nCMD a  `\n b\n"
    );
}

#[test]
fn a_script_line_that_starts_with_a_bracket_after_a_continuation_parses() {
    let out = fmt("FROM a\nRUN set -eux; \\\n\t[ -s x ]; \\\n\t[ ! -e y ]; \\\n\tz\n");
    assert!(out.contains("[ ! -e y ]"), "{out}");
}

#[test]
fn a_hash_after_text_is_not_a_comment() {
    // `fmt` refuses a tree with an error node, and the `FROM` after the `#` must stay an instruction
    let input = "FROM a\nADD ${R}#${V} /\nARG X=${V#linux/amd64}\nFROM b\n";
    assert_eq!(fmt(input), input);
}

#[test]
fn heredocs_are_kept() {
    let out = fmt("RUN <<EOF\nset -e\n  echo x  \n\nEOF\nCOPY <<-A /a\n\tbody\nA\nCMD x\n");
    assert_eq!(
        out,
        "RUN <<EOF\nset -e\n  echo x  \n\nEOF\nCOPY <<-A /a\n\tbody\nA\nCMD x\n"
    );
}

#[test]
fn blank_lines_and_trailing_comments() {
    assert_eq!(
        fmt("FROM a\n\n\n\nRUN b\n\n# c\n"),
        "FROM a\n\nRUN b\n\n# c\n"
    );
    assert_eq!(fmt(""), "");
    assert_eq!(
        fmt("FROM a\r\nRUN b \\\r\n  c\r\n"),
        "FROM a\r\nRUN b \\\r\n  c\r\n"
    );
}

#[test]
fn words_the_grammar_used_to_reject() {
    // a backslash before the closing single quote is literal
    assert_eq!(
        fmt("FROM a\nRUN x 'a\\'; \\\n  y\n"),
        "FROM a\nRUN x 'a\\'; \\\n  y\n"
    );
    // quoted USER
    assert_eq!(
        fmt("FROM a\nUSER \"$GF_UID\"\n"),
        "FROM a\nUSER \"$GF_UID\"\n"
    );
    // template placeholder as ARG default
    assert_eq!(fmt("ARG X={{ v }}\n"), "ARG X={{ v }}\n");
    // HEALTHCHECK test command that starts with `[`
    assert_eq!(
        fmt("FROM a\nHEALTHCHECK CMD [ \"$(curl -s x)\" -eq \"200\" ]\n"),
        "FROM a\nHEALTHCHECK CMD [ \"$(curl -s x)\" -eq \"200\" ]\n"
    );
    assert_eq!(
        fmt("FROM a\nHEALTHCHECK CMD [ \"a\" ]\n"),
        "FROM a\nHEALTHCHECK CMD [\"a\"]\n"
    );
}
