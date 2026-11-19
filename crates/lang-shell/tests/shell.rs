use core_config::OptionValue;
use provider_api::format_testing::{Formatting, assert_file_names, assert_formats, check_corpus};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_shell::provider(), "a.sh", input)
}

/// Every `.sh` and `.bash` file of the pinned corpus (`tests/conformance/fetch-shell-corpus.sh`)
/// that tree-sitter parses formats with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-shell --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    check_corpus(lang_shell::provider(), "a.sh", "shell", &["sh", "bash"]);
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_shell::provider(),
        &["a/install.sh", "x.bash", ".bashrc", "home/.profile"],
        &["sh", "a.shx", "profile", "bashrc.txt"],
    );
}

#[test]
fn statements_and_blank_lines() {
    assert_eq!(
        fmt(
            "#!/bin/sh\n\n\n\nset -e  # now\necho  a ;  echo b;\n\n# c\nx=1 y=2   cmd   \"$a\"  'b' \n"
        ),
        "#!/bin/sh\n\nset -e  # now\necho a\necho b\n\n# c\nx=1 y=2 cmd \"$a\" 'b'\n"
    );
    assert_eq!(fmt(""), "");
    assert_eq!(fmt("a\r\nb \\\r\n  c\r\n"), "a\r\nb \\\r\n\tc\r\n");
}

#[test]
fn compound_commands() {
    let out = fmt(
        "if [ -f a ]\nthen\n  echo x  # t\n    # before elif\nelif b; then y\nelse\n  z\n# last\nfi\nif a; then b; fi\nwhile read -r l; do echo \"$l\"; done < f\nfor i in 1 2 \\\n 3\ndo\n :\ndone\nfoo()\n{\n  { a; b; }\n  (cd x && make)\n}\n",
    );
    assert_eq!(
        out,
        "if [ -f a ]; then\n\techo x  # t\n\t# before elif\nelif b; then\n\ty\nelse\n\tz\n# last\nfi\nif a; then b; fi\nwhile read -r l; do echo \"$l\"; done <f\nfor i in 1 2 \\\n\t3; do\n\t:\ndone\nfoo() {\n\t{\n\t\ta\n\t\tb\n\t}\n\t(cd x && make)\n}\n"
    );
}

#[test]
fn case_arms() {
    let out = fmt(
        "case \"$1\" in\n  a|b)\n    echo one\n    # last\n    ;;\n  c) echo two ;;\n  *)\n    ;;\nesac\ncase $x in a) b;; esac\n",
    );
    assert_eq!(
        out,
        "case \"$1\" in\na | b)\n\techo one\n\t# last\n\t;;\nc) echo two ;;\n*)\n\t;;\nesac\ncase $x in a) b ;; esac\n"
    );
}

#[test]
fn lists_pipelines_and_continuations() {
    assert_eq!(
        fmt(
            "! cmd 2> /dev/null\na &&\n   b ||\n  c\ncmd < <(x) > f\na \\\n  && b\nx |\n y |\n z\n"
        ),
        "! cmd 2>/dev/null\na &&\n\tb ||\n\tc\ncmd < <(x) >f\na &&\n\tb\nx |\n\ty |\n\tz\n"
    );
}

#[test]
fn heredocs_and_multiline_words_are_kept() {
    let src = "cat <<EOF | grep x\n  body   \nEOF\nif a; then\n  cat <<-E >out\n\tbody\n\tE\nfi\nx=\"a  \n  b\" y=$(foo\n  bar)\necho 'a\n b'\n";
    assert_eq!(
        fmt(src),
        "cat <<EOF | grep x\n  body   \nEOF\nif a; then\n\tcat <<-E >out\n\tbody\n\tE\nfi\nx=\"a  \n  b\" y=$(\n\tfoo\n\tbar\n)\necho 'a\n b'\n"
    );
}

/// `$(...)` and arrays are laid out like statements, inside strings too; backticks are kept.
#[test]
fn substitutions_and_arrays() {
    let src = "f() {\n  x=\"a $(\n    b; c ) $( d )\" y=`e  f`\n  z=( p q\n      r\n  )\n  w=$(a |\n    b)\n  v=$(\n\n    a)\n  u=$(a # c\n  )\n}\n";
    assert_eq!(
        fmt(src),
        "f() {\n\tx=\"a $(\n\t\tb\n\t\tc\n\t) $(d)\" y=`e  f`\n\tz=(p q\n\t\tr\n\t)\n\tw=$(a |\n\t\tb)\n\tv=$(\n\n\t\ta\n\t)\n\tu=$(\n\t\ta # c\n\t)\n}\n"
    );
}

/// A trailing comment before a comment line inside an `if` condition keeps its own spacing: the
/// `;` that joins the two commands used to add a blank before it on every pass.
#[test]
fn a_trailing_comment_in_a_condition_is_stable() {
    assert_eq!(
        fmt("if a  # c1\n  # c2\n  b; then\n\tx\nfi\n"),
        "if a;  # c1\n# c2\nb; then\n\tx\nfi\n"
    );
}

/// Forked-grammar cases: two assignments and a command on the next line are two statements,
/// `for x do`, a word ending in `$`, a quoted string in a case pattern, a here-string after a
/// redirect.
#[test]
fn bash_the_upstream_grammar_rejects() {
    assert_eq!(
        fmt("a=1\nc=1 d=2\n  foo\nfoo   bar\n"),
        "a=1\nc=1 d=2\nfoo\nfoo bar\n"
    );
    assert_eq!(
        fmt("for x do :; done\nif grep -q 404$; then :; fi\ncase $a in\n --*' '*) :;;\nesac\n"),
        "for x; do :; done\nif grep -q 404$; then :; fi\ncase $a in\n--*' '*) : ;;\nesac\n"
    );
}

/// Formats with the options of the `shfmt` preset.
#[track_caller]
fn shfmt_style(input: &str) -> String {
    let options = [
        "comment_column:align",
        "heredoc_indent:reindent",
        "case_last_terminator:add",
    ]
    .map(|o| {
        let (k, v) = o.split_once(':').expect("option");
        (k, OptionValue::String(v.into()))
    });
    core_format::testing::assert_formats(
        &Formatting {
            options: options.to_vec(),
            ..Formatting::new(lang_shell::provider(), "a.sh")
        },
        input,
    )
}

#[test]
fn shfmt_options() {
    // Trailing comments of consecutive lines share a column; other lines keep one blank.
    assert_eq!(
        shfmt_style("a   # x\nlong_one   # y\n\nc    # z\n"),
        "a        # x\nlong_one # y\n\nc # z\n"
    );
    assert_eq!(
        shfmt_style("a=1 # one\nlonger=2   # two\nbb=3\n"),
        "a=1      # one\nlonger=2 # two\nbb=3\n"
    );
    // A `<<-` body and delimiter are re-indented; a blank body line stays empty.
    assert_eq!(
        shfmt_style("if x; then\n\t\t\tcat <<-EOF\n\t\ta\n\n\t\t\t\tb\n\tEOF\nfi\n"),
        "if x; then\n\tcat <<-EOF\n\t\ta\n\n\t\tb\n\tEOF\nfi\n"
    );
    assert_eq!(
        shfmt_style("cat <<EOF\n\ta\nEOF\n"),
        "cat <<EOF\n\ta\nEOF\n"
    );
    // The last arm gets its `;;`.
    assert_eq!(
        shfmt_style("case $x in\n  a) b ;;\n  *)\n    c\nesac\n"),
        "case $x in\na) b ;;\n*)\n\tc\n\t;;\nesac\n"
    );
}
