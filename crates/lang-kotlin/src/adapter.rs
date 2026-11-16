//! ktrs-parser's tree (a port of the Kotlin compiler's PSI parser) as our lossless tree.
//!
//! The tree is one flat pre-order array of nodes and tokens, whitespace and comments included,
//! and its tokens tile the text exactly. The adapter walks it once: a node is a row of
//! `grammars/kotlin.grammar` named like the PSI element type, a token is our token of its kind
//! (every keyword is `Keyword`), and whitespace, comments and KDoc are left to `replay`, which
//! splits the text between tokens (`comments` reads `//`, nested `/* */` and a `#!` line).
//!
//! What ktrs needs from its caller, done here: it requires `\n` line breaks (the text is
//! normalized and offsets are mapped back), it overflows the stack on deep nesting (the parse
//! runs on a thread with a large stack, and `guard` refuses the shapes that recurse beyond it),
//! it is exponential on nested annotation arguments (`guard`), and it may panic (a panic is a
//! parse error). A file the parser is not run on is one `Unknown` token and one error.

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, grammars};
use ktrs_parser::FileKind;
use ktrs_syntax::SyntaxKind as P;
use provider_api::adapter::{KindNames, Kinds, Node, Tok, replay, whole_file};
use provider_api::{ParseError, ParseResult, SourceFile};
use std::panic::{AssertUnwindSafe, catch_unwind};

const KINDS: Kinds = Kinds {
    root: k::KT_FILE,
    eof: k::END_OF_FILE,
    unknown: k::UNKNOWN,
    whitespace: k::WHITESPACE,
    newline: k::NEWLINE,
};

/// The parse thread's stack: ktrs recurses once per nesting level, about 1 KB per level at most.
const STACK: usize = 256 << 20;

/// A file over this size is not parsed (the stack above bounds the recursion of a file this big).
pub const MAX_BYTES: usize = 16 << 20;

/// The deepest bracket nesting, and the longest run of prefix operators, parsed.
const MAX_DEPTH: usize = 2000;
/// The deepest nesting of annotation arguments parsed (the parser is exponential in it).
const MAX_ANNOTATION_ARGS: usize = 12;

pub fn parse_file(file: &SourceFile) -> ParseResult {
    let kind = if file.id.ends_with(".kts") {
        FileKind::Script
    } else {
        FileKind::Source
    };
    parse(&file.text, kind)
}

pub fn parse(src: &str, kind: FileKind) -> ParseResult {
    if src.len() > MAX_BYTES {
        return whole_file(src, KINDS, format!("file over {} MB", MAX_BYTES >> 20));
    }
    if let Some(why) = guard(src) {
        return whole_file(src, KINDS, why);
    }
    // ktrs wants `\n`: `crs` holds the offsets (in the normalized text) of the removed `\r`.
    let (text, crs) = normalize(src);
    let walked = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(STACK)
            .spawn_scoped(s, || {
                catch_unwind(AssertUnwindSafe(|| walk(&text, &crs, kind)))
            })
            .map(|h| h.join())
    });
    let Ok(Ok(Ok((tokens, nodes, mut errors)))) = walked else {
        return whole_file(src, KINDS, "the Kotlin parser failed on this file");
    };
    let mut result = replay(src, KINDS, &tokens, &nodes, comments, |t, _| t);
    errors.append(&mut result.errors);
    errors.sort_by_key(|e| e.start);
    result.errors = errors;
    result
}

/// Parses `text` (normalized) and collects our tokens, our nodes and the parse errors, with
/// offsets in the original text.
fn walk(text: &str, crs: &[u32], kind: FileKind) -> (Vec<Tok>, Vec<Node>, Vec<ParseError>) {
    let parsed = ktrs_parser::parse_file(text, kind);
    let to_src = |n: usize| (n + crs.partition_point(|&p| (p as usize) < n)) as u32;
    let tree = &parsed.tree;
    let grammar = grammars::KOTLIN.get();
    let mut names = KindNames::new(grammar, 512, |_| true);
    // Our token kind per ktrs kind (`Some(None)`: a token that is not ours), found on first use.
    let mut token_kinds: Vec<Option<Option<SyntaxKind>>> = vec![None; 512];
    let mut tokens = Vec::with_capacity(tree.len() / 2);
    let mut nodes = Vec::with_capacity(tree.len() / 2);
    let mut errors = Vec::new();
    let mut messages = parsed.error_messages.iter();
    let mut e = 1;
    while e < tree.len() as u32 {
        let kind = tree.kind(e);
        let range = tree.text_range(e);
        let (start, end) = (usize::from(range.start()), usize::from(range.end()));
        if tree.is_token(e) {
            e += 1;
            let ours = *token_kinds[kind as usize].get_or_insert_with(|| {
                token_name(kind).map(|n| {
                    let found = match n {
                        Some(name) => grammar.kind_by_name(name),
                        None => (grammar.tokens().iter())
                            .find(|d| d.text.as_deref() == Some(&text[start..end]))
                            .map(|d| d.kind),
                    };
                    found.filter(|k| k.is_token()).unwrap_or(k::UNKNOWN)
                })
            });
            if kind == P::BLOCK_COMMENT && block_end(&text[start..end]).is_none() {
                errors.push(unclosed(to_src(start), to_src(end)));
            }
            if let Some(kind) = ours.filter(|_| start < end) {
                tokens.push(Tok {
                    kind,
                    start: to_src(start),
                    end: to_src(end),
                });
            }
        } else if kind == P::DOC_COMMENT {
            if block_end(&text[start..end]).is_none() {
                errors.push(unclosed(to_src(start), to_src(end)));
            }
            e = tree.subtree_end(e);
        } else {
            e += 1;
            if kind == P::ERROR_ELEMENT {
                let message = messages.next().map_or("syntax error", String::as_str);
                errors.push(ParseError::syntax(
                    to_src(start),
                    to_src(end) - to_src(start),
                    message,
                ));
            }
            if let Some(kind) = names.get(kind as usize, false, || pascal(kind)) {
                nodes.push(Node {
                    start: to_src(start),
                    end: to_src(end),
                    kind,
                });
            }
        }
    }
    (tokens, nodes, errors)
}

fn unclosed(start: u32, end: u32) -> ParseError {
    ParseError::syntax(start, end - start, "Unclosed comment")
}

/// The text with `\r\n` as `\n`, and the offsets in it where a `\r` was removed.
fn normalize(src: &str) -> (std::borrow::Cow<'_, str>, Vec<u32>) {
    if !src.contains("\r\n") {
        return (src.into(), Vec::new());
    }
    let (mut out, mut crs) = (String::with_capacity(src.len()), Vec::new());
    let mut rest = src;
    while let Some(i) = rest.find("\r\n") {
        out.push_str(&rest[..i]);
        crs.push(out.len() as u32);
        rest = &rest[i + 1..];
    }
    out.push_str(rest);
    (out.into(), crs)
}

/// The shapes ktrs cannot parse safely: nesting deeper than the stack, and nested annotation
/// arguments (exponential time). A rough scan: strings and comments are not skipped, which only
/// matters for text with thousands of unclosed brackets.
fn guard(src: &str) -> Option<&'static str> {
    let b = src.as_bytes();
    let (mut depth, mut run, mut ann) = (0usize, 0usize, Vec::new());
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'(' | b'[' | b'{' => {
                depth += 1;
                // `@Name(`: an annotation's arguments.
                let mut j = i;
                while j > 0 && (b[j - 1].is_ascii_alphanumeric() || matches!(b[j - 1], b'_' | b'.'))
                {
                    j -= 1;
                }
                ann.push(c == b'(' && j < i && j > 0 && b[j - 1] == b'@');
                if ann.iter().filter(|a| **a).count() > MAX_ANNOTATION_ARGS {
                    return Some("annotation arguments nested too deeply");
                }
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                ann.pop();
            }
            _ => {}
        }
        run = if matches!(c, b'!' | b'-' | b'+' | b'@') {
            run + 1
        } else if c == b' ' {
            run
        } else {
            0
        };
        if depth > MAX_DEPTH || run > MAX_DEPTH {
            return Some("nesting too deep");
        }
    }
    None
}

/// A `//` or (nested) `/* */` comment, or a `#!` line at the start of the file.
fn comments(rest: &str, pos: usize) -> Option<(SyntaxKind, usize)> {
    let end_of_line = |s: &str| s.find(['\n', '\r']).unwrap_or(s.len());
    if rest.starts_with("//") || (pos == 0 && rest.starts_with("#!")) {
        return Some((k::COMMENT, end_of_line(rest)));
    }
    if !rest.starts_with("/*") {
        return None;
    }
    Some((k::COMMENT, block_end(rest).unwrap_or(rest.len())))
}

/// The end of the (nested) block comment `rest` starts with; `None` when it is not closed.
fn block_end(rest: &str) -> Option<usize> {
    let (mut depth, mut i) = (0, 0);
    let b = rest.as_bytes();
    while i < b.len() {
        if b[i..].starts_with(b"/*") {
            depth += 1;
            i += 2;
        } else if b[i..].starts_with(b"*/") {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Some(i);
            }
        } else {
            i += 1;
        }
    }
    None
}

/// The grammar's token name of a ktrs token (`Some(None)`: the grammar token with its text), `None` for the ones that are not tokens of ours: trivia (left to `replay`) and
/// the parser's zero-width markers.
fn token_name(kind: P) -> Option<Option<&'static str>> {
    Some(Some(match kind {
        P::WHITE_SPACE
        | P::EOL_COMMENT
        | P::BLOCK_COMMENT
        | P::SHEBANG_COMMENT
        | P::EOF
        | P::EOL_OR_SEMICOLON
        | P::DANGLING_NEWLINE => return None,
        P::IDENTIFIER | P::FIELD_IDENTIFIER => "Identifier",
        P::INTEGER_LITERAL => "Integer",
        P::FLOAT_LITERAL => "Float",
        P::CHARACTER_LITERAL => "Character",
        P::OPEN_QUOTE => "OpenQuote",
        P::CLOSING_QUOTE => "ClosingQuote",
        P::REGULAR_STRING_PART => "StringPart",
        P::ESCAPE_SEQUENCE => "EscapeSequence",
        P::INTERPOLATION_PREFIX => "InterpolationPrefix",
        P::SHORT_TEMPLATE_ENTRY_START => "ShortTemplateEntryStart",
        P::LONG_TEMPLATE_ENTRY_START => "LongTemplateEntryStart",
        P::LONG_TEMPLATE_ENTRY_END => "LongTemplateEntryEnd",
        P::RESERVED => "Unknown",
        P::AS_SAFE | P::NOT_IN | P::NOT_IS => "Keyword",
        _ if format!("{kind:?}").ends_with("_KEYWORD") => "Keyword",
        _ => return Some(None),
    }))
}

/// `VALUE_PARAMETER_LIST` as `ValueParameterList`: the grammar's name for a PSI node kind.
fn pascal(kind: P) -> String {
    (format!("{kind:?}").split('_'))
        .map(|w| w[..1].to_string() + &w[1..].to_lowercase())
        .collect()
}
