use provider_api::format_testing::assert_formats;

fn provider() -> impl provider_api::LanguageProvider {
    lang_mako::provider()
}

#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(provider(), "a.mako", input)
}

#[test]
fn a_formatted_file_is_unchanged() {
    let text = "<%inherit file=\"base.mako\"/>\n% for x in y:\n  <li>${x | h}</li>\n% endfor\n";
    assert_eq!(fmt(text), text);
}

#[test]
fn blanks_and_ends_are_normalised() {
    assert_eq!(
        fmt("\n\n<p>a</p>   \n\n\n\n<p>b</p>\n\n\n"),
        "<p>a</p>\n\n<p>b</p>\n"
    );
    assert_eq!(fmt("<p>a</p>"), "<p>a</p>\n");
    assert_eq!(
        fmt("<p>a</p>\r\n\r\n\r\n<p>b</p>"),
        "<p>a</p>\r\n\r\n<p>b</p>\r\n"
    );
}

#[test]
fn control_lines_and_comments_are_spaced() {
    assert_eq!(fmt("%if x:\n<b/>\n%endif\n"), "% if x:\n<b/>\n% endif\n");
    assert_eq!(fmt("##note\n"), "## note\n");
    assert_eq!(fmt("${ x | h }\n"), "${x | h}\n");
}

#[test]
fn a_tag_with_an_empty_body_keeps_its_lines() {
    let text = "<%block name=\"b\">\n</%block>\n<%def name=\"f()\">\n  <b/>\n</%def>\n";
    assert_eq!(fmt(text), text);
}

#[test]
fn a_template_of_another_host_keeps_its_blanks() {
    let text = "x = 1   \n\n\n\ty = 2\n";
    assert_eq!(assert_formats(provider(), "script.py.mako", text), text);
    assert_eq!(
        assert_formats(provider(), "a.html.mako", "<p>a</p>   \n\n\n\n<p/>\n"),
        "<p>a</p>\n\n<p/>\n"
    );
}

#[test]
fn tabs_become_spaces() {
    assert_eq!(
        fmt("% if x:\n\t<b/>\n% endif\n"),
        "% if x:\n    <b/>\n% endif\n"
    );
}
