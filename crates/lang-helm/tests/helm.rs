use core_config::OptionValue;
use provider_api::LanguageProvider;
use provider_api::format_testing::{
    Formatting, assert_file_names, assert_parses, check_corpus_where,
};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    fmt_with(input, &[])
}

#[track_caller]
fn fmt_with(input: &str, options: &[(&'static str, OptionValue)]) -> String {
    let subject = Formatting {
        options: options.to_vec(),
        ..Formatting::new(lang_helm::provider(), "templates/a.yaml")
    };
    core_format::testing::assert_formats(&subject, input)
}

#[test]
fn claims_the_files_below_templates() {
    assert_file_names(
        &lang_helm::provider(),
        &[
            "chart/templates/a.yaml",
            "chart/templates/tests/a.yml",
            "chart/templates/_helpers.tpl",
        ],
        &[
            "chart/values.yaml",
            "chart/Chart.yaml",
            "chart/templates/NOTES.txt",
        ],
    );
}

#[test]
fn parses_actions_and_text() {
    assert_parses(
        &lang_helm::provider(),
        "templates/a.yaml",
        "a: {{ .Values.a | default \"x\" | quote }}\n{{- range $k, $v := .Values.m }}\n{{ $k }}: {{ $v }}\n{{- end }}\n",
    );
}

#[test]
fn one_blank_inside_the_delimiters() {
    assert_eq!(
        fmt(
            "a: {{.Values.a}}\nb: {{   include \"x\" . | nindent 4}}\nc: {{.Values.c}}\nd: {{- .d -}}\n"
        ),
        "a: {{ .Values.a }}\nb: {{ include \"x\" . | nindent 4 }}\nc: {{ .Values.c }}\nd: {{- .d -}}\n"
    );
}

#[test]
fn comments_strings_and_multi_line_actions_stay() {
    let src =
        "{{/* a  comment */}}\n{{- /* b */ -}}\na: {{ \"}}\" }}\nb: {{ include \"x\"\n   . }}\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn control_lines_stand_at_the_content_they_open() {
    assert_eq!(
        fmt(
            "data:\n{{- if .x }}\n  a: 1\n{{- else }}\n  b: 2\n{{- end }}\nspec:\n    {{- with .y }}\n  c: 3\n    {{- end }}\n"
        ),
        "data:\n  {{- if .x }}\n  a: 1\n  {{- else }}\n  b: 2\n  {{- end }}\nspec:\n  {{- with .y }}\n  c: 3\n  {{- end }}\n"
    );
}

#[test]
fn nested_blocks_and_comments_before_the_content() {
    assert_eq!(
        fmt(
            "a:\n{{- if .x }}\n{{- /* why */ -}}\n{{- range .l }}\n    - {{ . }}\n{{- end }}\n{{- end }}\n"
        ),
        "a:\n    {{- if .x }}\n{{- /* why */ -}}\n    {{- range .l }}\n    - {{ . }}\n    {{- end }}\n    {{- end }}\n"
    );
}

#[test]
fn a_line_without_a_trim_keeps_its_blanks() {
    // Without `{{-` the blanks in front of the action are output text.
    let src = "a:\n{{ if .x }}\n  b: 1\n{{ end }}\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn define_stands_where_it_is_and_empty_blocks_stay() {
    let src = "{{- define \"x\" }}\n  body: 1\n{{- end }}\n{{- if .a }}\n{{- else }}\n{{- end }}\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn text_between_actions_is_not_touched() {
    // The blank lines and trailing blanks of a block scalar are its content.
    let src = "data: |\n  a  \n\n\n  b\n  {{- if .x }}\n  c\n  {{- end }}\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn alignment_can_be_turned_off() {
    let src = "data:\n{{- if .x }}\n  a: 1\n{{- end }}\n";
    assert_eq!(
        fmt_with(src, &[("align_control_actions", OptionValue::Bool(false))]),
        src
    );
}

#[test]
fn the_end_of_the_file_is_trimmed_and_ends_in_one_line_break() {
    assert_eq!(fmt("a: 1\n\n\n"), "a: 1\n");
    assert_eq!(fmt("a: {{ .x }}"), "a: {{ .x }}\n");
}

/// Every file below a `templates` folder of the pinned corpus parses, formats with the same tokens
/// and idempotently.
///
/// `cargo test --profile corpus -p lang-helm --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let provider = lang_helm::provider();
    let (_, unparsed) = check_corpus_where(
        lang_helm::provider(),
        "templates/a.yaml",
        "helm",
        &["yaml", "yml", "tpl"],
        |p| provider.file_matchers().iter().any(|m| m.matches(p)),
    );
    println!("{unparsed:#?}");
}

#[test]
fn the_template_keyword_is_not_the_document() {
    assert_eq!(
        fmt("a: {{ template \"x.name\" . }}\n"),
        "a: {{ template \"x.name\" . }}\n"
    );
}
