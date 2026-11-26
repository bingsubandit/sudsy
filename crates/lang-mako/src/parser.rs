//! A lossless parser of Mako with error recovery: a flat list of items. Control blocks
//! (`% if` ... `% endif`) are not nodes: the shared pairing of `core_syntax::block_pair` reads the
//! keywords in order and reports an unpaired block on its own line; tags are matched by name here.
//!
//! Recovery: every construct is complete in the token stream or ends at its opener (the lexer
//! reports an unterminated `${` or `<%`), so one bad construct damages nothing after it. A closing
//! tag of no open tag, an end keyword with no block and a tag or block left open are errors on
//! their own token.
//!
//! concept: none (the productions of Mako on the parser kit of core_syntax::parse)

use crate::blocks::BLOCKS;
use crate::generated::kind as k;
use crate::lexer::lex;
use core_syntax::block_pair::Pairing;
use core_syntax::end_tags::{OpenTags, Syntax};
use core_syntax::parse::{self, MAX_DEPTH, Parsed};
use provider_api::{ParseResult, SourceFile};

/// Tags open at once; a deeper one is an error and no element.
const LEVELS: usize = MAX_DEPTH / 5;

/// `<%name>` and `</%name>`.
const TAG_SYNTAX: Syntax = Syntax {
    start: "<%",
    end: "</%",
};

/// The open tags hold the index of the opening token and whether a body node is open.
type Tags<'a> = OpenTags<'a, (usize, bool)>;

/// The tags Mako knows; a name with a `:` is a call of a namespace.
const TAGS: [&str; 8] = [
    "include",
    "inherit",
    "page",
    "namespace",
    "def",
    "block",
    "call",
    "text",
];

pub fn parse_file(file: &SourceFile) -> ParseResult {
    let Parsed { tree, errors } = parse(&file.text);
    ParseResult { tree, errors }
}

pub fn parse(src: &str) -> Parsed {
    let mut p = parse::Parser::new(src, lex(src), k::TRIVIA_MASK, k::EOF);
    let mut pairing = Pairing::new(&BLOCKS);
    let mut tags = Tags::new();
    p.start(k::FILE);
    while !p.at_eof() {
        match p.cur() {
            k::EXPR_OPEN => expression(&mut p),
            k::PY_OPEN => block(&mut p),
            k::PERCENT => control_line(&mut p, &mut pairing),
            k::HASH_HASH => comment_line(&mut p),
            k::TAG_OPEN => tag(&mut p, &mut tags),
            k::CLOSE_TAG => close_tag(&mut p, &mut tags),
            _ => p.bump(),
        }
    }
    let unclosed: Vec<_> = tags.drain().collect();
    for &(_, (_, body)) in &unclosed {
        close(&mut p, body);
    }
    p.bump(); // Eof
    p.finish();
    for f in pairing.finish() {
        p.error_at(f.at as usize, f.fault.code(), f.fault.message());
    }
    for (name, (at, _)) in unclosed {
        p.error_at(
            at,
            "unclosed-tag",
            format!("tag `<%{name}>` is never closed"),
        );
    }
    p.done()
}

/// `${ code | filters }`.
fn expression(p: &mut parse::Parser<'_>) {
    p.start(k::EXPRESSION);
    p.bump();
    for kind in [k::PY_EXPR, k::PIPE, k::FILTERS, k::EXPR_CLOSE] {
        if p.cur() == kind {
            p.bump();
        }
    }
    p.finish();
}

/// `<% code %>`.
fn block(p: &mut parse::Parser<'_>) {
    p.start(k::PY_BLOCK);
    p.bump();
    for kind in [k::CODE_INDENT, k::PY_CODE, k::PY_CLOSE] {
        if p.cur() == kind {
            p.bump();
        }
    }
    p.finish();
}

/// `## comment`.
fn comment_line(p: &mut parse::Parser<'_>) {
    p.start(k::COMMENT_LINE);
    p.bump();
    if p.cur() == k::COMMENT_TEXT {
        p.bump();
    }
    p.finish();
}

fn control_line(p: &mut parse::Parser<'_>, pairing: &mut Pairing) {
    p.start(k::CONTROL_LINE);
    p.bump();
    let kind = p.cur();
    if matches!(
        kind,
        k::CONTROL_OPEN | k::CONTROL_ARM | k::CONTROL_CLOSE | k::CONTROL_WORD
    ) {
        let word = p.text_at(p.pos);
        let at = p.pos as u32;
        let step = if kind == k::CONTROL_CLOSE {
            pairing.closer(word, at)
        } else {
            pairing.word(word, at)
        };
        if kind == k::CONTROL_WORD && step.depth > 0 {
            p.error(
                "unexpected-keyword",
                "this keyword is no arm of the open block",
            );
        }
        p.bump();
        for kind in [k::PY_EXPR, k::CONTROL_REST, k::COLON] {
            if p.cur() == kind {
                p.bump();
            }
        }
    } else if p.cur() == k::CONTROL_REST {
        p.bump();
    }
    p.finish();
}

/// `<%name attributes>` or `<%name attributes/>`.
fn tag<'a>(p: &mut parse::Parser<'a>, tags: &mut Tags<'a>) {
    let at = p.pos;
    let name = &p.text_at(at)[2..];
    if !name.contains(':') && !TAGS.contains(&name) {
        p.error("unknown-tag", format!("no tag named `{name}`"));
    }
    // A tag that is not self-closing holds what stands up to its closing tag.
    let mut end = p.pos;
    while !matches!(p.kind_at(end), k::TAG_END | k::TAG_SELF_END | k::EOF) {
        end += 1;
    }
    let mut open = p.kind_at(end) == k::TAG_END;
    if open && tags.len() >= LEVELS {
        p.too_deep("tags", LEVELS);
        open = false;
    }
    if open {
        p.start(k::ELEMENT);
    }
    p.start(k::TAG);
    p.bump();
    loop {
        match p.cur() {
            k::ATTR_NAME => {
                p.start(k::ATTRIBUTE);
                p.bump();
                for kind in [k::EQUALS, k::ATTR_VALUE] {
                    if p.cur() == kind {
                        p.bump();
                    }
                }
                p.finish();
            }
            k::DEF_KEY => {
                p.start(k::DEF_NAME);
                for _ in 0..3 {
                    p.bump();
                }
                p.finish();
            }
            k::EQUALS | k::ATTR_VALUE | k::COMMA => p.bump(),
            _ => break,
        }
    }
    if matches!(p.cur(), k::TAG_END | k::TAG_SELF_END) {
        p.bump();
    }
    p.finish();
    if open {
        // An empty body is no node.
        let body = p.cur() != k::CLOSE_TAG;
        if body {
            p.start(k::BODY);
        }
        tags.push(name, (at, body));
    }
}

/// Ends the element of a tag: its body, when it has one, and the element.
fn close(p: &mut parse::Parser<'_>, body: bool) {
    if body {
        p.finish();
    }
    p.finish();
}

/// `</%name>`: closes the tag of that name, and the tags open inside it.
fn close_tag<'a>(p: &mut parse::Parser<'a>, tags: &mut Tags<'a>) {
    let text = p.text_at(p.pos);
    let name = text[3..text.len() - 1].trim_matches([' ', '\t']);
    let inner = tags.innermost();
    match tags.close(name) {
        Some(closed) => {
            if let (Some(first), false) = (inner, closed.inner.is_empty()) {
                p.error("mismatched-close-tag", TAG_SYNTAX.mismatch(first, name));
            }
            for (_, (_, body)) in &closed.inner {
                close(p, *body);
            }
            if closed.data.1 {
                p.finish();
            }
            p.bump();
            p.finish();
        }
        None => {
            p.error("unpaired-close-tag", format!("`{text}` closes no open tag"));
            p.bump();
        }
    }
}
