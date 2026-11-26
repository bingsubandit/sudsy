use core_syntax::{SyntaxElement, SyntaxNode};
use provider_api::LanguageProvider;
use provider_api::format_testing::{assert_file_names, assert_lossless};

fn provider() -> impl LanguageProvider {
    lang_mako::provider()
}

/// The tree without trivia: nodes as `Name[children]`, tokens as `Kind:text`.
fn shape(node: &SyntaxNode) -> String {
    let g = core_syntax::grammars::MAKO.get();
    let parts: Vec<String> = (node.children_with_tokens())
        .filter_map(|e| match e {
            SyntaxElement::Node(n) => Some(shape(&n)),
            SyntaxElement::Token(t) if g.is_trivia(t.kind()) || t.text().is_empty() => None,
            SyntaxElement::Token(t) => Some(format!("{}:{}", g.name(t.kind()), t.text())),
        })
        .collect();
    format!("{}[{}]", g.name(node.kind()), parts.join(" "))
}

/// The shape of `text` (the `File` node's brackets left off) and the codes of its errors; the
/// text must print back as written.
#[track_caller]
fn parse(text: &str) -> (String, Vec<String>) {
    let r = assert_lossless(&provider(), "a.mako", text);
    let s = shape(&r.tree);
    let s = (s.strip_prefix("File[")).and_then(|s| s.strip_suffix(']'));
    (
        s.unwrap().to_string(),
        r.errors.into_iter().map(|e| e.code).collect(),
    )
}

#[track_caller]
fn ok(text: &str) -> String {
    let (s, errors) = parse(text);
    assert!(errors.is_empty(), "{text:?}: {errors:?}");
    s
}

#[track_caller]
fn errors(text: &str) -> Vec<String> {
    parse(text).1
}

#[test]
fn file_names() {
    assert_file_names(&provider(), &["a.mako", "x/b.MAKO"], &["a.mak", "mako"]);
}

#[test]
fn text_is_one_token_per_word() {
    assert_eq!(ok("<b>a  b</b>\n"), "Text:<b>a Text:b</b>");
    assert_eq!(
        ok("a${x}b"),
        "Text:a Expression[ExprOpen:${ PyExpr:x ExprClose:}] Text:b"
    );
}

#[test]
fn an_expression_ends_at_the_first_unnested_brace() {
    assert_eq!(
        ok("${ {'a': '}'}['a'] }"),
        "Expression[ExprOpen:${ PyExpr:{'a': '}'}['a'] ExprClose:}]"
    );
    assert_eq!(
        ok("${ a | h, trim }"),
        "Expression[ExprOpen:${ PyExpr:a Pipe:| Filters:h, trim ExprClose:}]"
    );
    assert_eq!(errors("${ a\nb"), ["unterminated-expression"]);
}

#[test]
fn a_python_block_ends_at_the_first_unquoted_close() {
    assert_eq!(
        ok("<%\n    x = \"%>\"\n%>"),
        "PyBlock[PyOpen:<% CodeIndent:     PyCode:x = \"%>\" PyClose:%>]"
    );
    assert_eq!(
        ok("<%! import os %>"),
        "PyBlock[PyOpen:<%! PyCode:import os PyClose:%>]"
    );
    assert_eq!(errors("<% x = 1"), ["unterminated-block"]);
}

#[test]
fn control_lines_pair_by_keyword() {
    assert_eq!(
        ok("% if a:\n  x\n% else:\n  y\n% endif\n"),
        "ControlLine[Percent:% ControlOpen:if PyExpr:a Colon::] Text:x \
         ControlLine[Percent:% ControlArm:else Colon::] Text:y \
         ControlLine[Percent:% ControlClose:endif]"
    );
    assert_eq!(
        ok("% for x in y:\n% endfor"),
        "ControlLine[Percent:% ControlOpen:for PyExpr:x in y Colon::] \
         ControlLine[Percent:% ControlClose:endfor]"
    );
    assert_eq!(errors("% if a:\n"), ["unclosed-block"]);
    assert_eq!(errors("% endif\n"), ["unpaired-closer"]);
    assert_eq!(
        errors("% for x in y:\n% endif\n"),
        ["unclosed-block", "unpaired-closer"]
    );
    assert_eq!(
        errors("% for x in y:\n% else:\n% endfor\n"),
        Vec::<String>::new()
    );
    assert_eq!(errors("%\n"), ["invalid-control-line"]);
}

#[test]
fn percent_and_hashes_are_control_only_at_the_start_of_a_line() {
    assert_eq!(ok("a % b ## c\n"), "Text:a Text:% Text:b Text:## Text:c");
    assert_eq!(ok("  %% x\n"), "Text:%% Text:x");
    assert_eq!(ok("  ## c d\n"), "CommentLine[HashHash:## CommentText:c d]");
}

#[test]
fn tags_nested_too_deep_are_an_error() {
    let text = format!("{}{}", "<%def name='f'>".repeat(100), "</%def>".repeat(100));
    assert!(errors(&text).contains(&"nesting-too-deep".to_string()));
}

#[test]
fn tags_are_matched_by_name() {
    assert_eq!(
        ok("<%def name=\"f(x)\">a</%def>"),
        "Element[Tag[TagOpen:<%def DefName[DefKey:name Equals:= DefValue:\"f(x)\"] TagEnd:>] Body[Text:a] CloseTag:</%def>]"
    );
    assert_eq!(
        ok("<%include file='a'/>"),
        "Tag[TagOpen:<%include Attribute[AttrName:file Equals:= AttrValue:'a'] TagSelfEnd:/>]"
    );
    assert_eq!(errors("<%def name=\"f\">"), ["unclosed-tag"]);
    assert_eq!(errors("</%def>"), ["unpaired-close-tag"]);
    assert_eq!(
        errors("<%def name=\"f\"><%block>x</%def></%block>"),
        ["mismatched-close-tag", "unpaired-close-tag"]
    );
    assert_eq!(errors("<%nope/>"), ["unknown-tag"]);
    assert_eq!(errors("<%ns:go a='1'>x</%ns:go>"), Vec::<String>::new());
    assert_eq!(
        ok("<%text>${ <% </%text>"),
        "Element[Tag[TagOpen:<%text TagEnd:>] Body[RawText:${ <% ] CloseTag:</%text>]"
    );
}

#[test]
fn a_doc_block_and_a_continuation_are_trivia() {
    assert_eq!(ok("<%doc> ${ </%doc>a \\\nb"), "Text:a Text:b");
}

#[test]
fn any_text_is_lossless() {
    for text in [
        "",
        "\u{feff}${x}\r\n% if a:\r\n<% y %>\r\n% endif",
        "<%",
        "</%",
        "</%x",
        "<%x a=\"",
        "${",
        "${ '",
        "% ",
        "\\",
        "a\\\r\nb\\",
        "é<%é>\u{a0}${é}",
    ] {
        parse(text);
    }
}

/// Every `*.mako` of the pinned corpus (`tests/conformance/fetch-mako-corpus.sh`) prints back
/// byte for byte, formats stably and fits the grammar; the parse errors are the files Mako's own
/// compiler rejects.
///
/// `cargo test --profile corpus -p lang-mako --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    use provider_api::format_testing::{check_corpus, check_corpus_shapes};
    check_corpus(provider(), "a.mako", "mako", &["mako"]);
    check_corpus_shapes(provider(), "mako", &["mako"]);
}
