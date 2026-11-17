//! Prism's tokens and AST as our lossless tree.
//!
//! One parse through `prism` (FFI) gives the tokens, from Prism's lex callback, and the spans of
//! the nodes, one row of `grammars/ruby.grammar` per Prism node type. Whitespace, newlines and
//! comments are the text between tokens, which `replay` splits (`scan` reads `#` comments,
//! `=begin` blocks and line continuations).
//!
//! A heredoc is not contiguous: its node ends with the opener (`<<~EOS`) and its body and closing
//! label are lexed right after the opener. The body, with its interpolations, is one `HeredocBody`
//! token placed after the line of the opener, and the nodes inside it are left out. Text after
//! `__END__` is one `Data` token. A panic in the parser is a parse error and the file stays as
//! written.

mod prism;

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, grammars};
use prism::{RawNode, RawToken, node_names, token_names};
use provider_api::adapter::{KindNames, Kinds, Node, Tok, replay, whole_file};
use provider_api::{ParseError, ParseResult, SourceFile};
use std::panic::catch_unwind;
use std::sync::OnceLock;

const KINDS: Kinds = Kinds {
    root: k::PROGRAM_NODE,
    eof: k::EOF,
    unknown: k::UNKNOWN,
    whitespace: k::WHITESPACE,
    newline: k::NEWLINE,
};

pub fn parse_file(file: &SourceFile) -> ParseResult {
    parse(&file.text)
}

pub fn parse(src: &str) -> ParseResult {
    match catch_unwind(|| build(src)) {
        Ok(result) => result,
        Err(_) => whole_file(src, KINDS, "the Ruby parser failed on this file"),
    }
}

/// Prism recurses up to 10,000 levels (`!!!...x`, nested `def`), a few MB of C stack in a debug
/// build: the parse runs on a segment of its own when the thread has less than `RED_ZONE` left.
const RED_ZONE: usize = 16 << 20;
const STACK: usize = 32 << 20;

fn build(src: &str) -> ParseResult {
    let parsed = stacker::maybe_grow(RED_ZONE, STACK, || prism::parse(src));
    let (mut tokens, bodies) = tokens(src, &parsed.tokens);
    // `data` is the `__END__` line; the data starts on the next one.
    if let Some(marker) = parsed.data {
        let line = marker as usize + "__END__".len();
        if let Some(at) = src[line..]
            .find('\n')
            .map(|i| line + i + 1)
            .filter(|&at| at < src.len())
        {
            tokens.push(Tok {
                kind: k::DATA,
                start: at as u32,
                end: src.len() as u32,
            });
        }
    }
    let mut nodes = nodes(src, &parsed.nodes, &bodies);
    nodes.sort_by_key(|n| (n.start, std::cmp::Reverse(n.end)));
    let mut result = replay(
        src,
        KINDS,
        &tokens,
        &nodes,
        |rest, pos| scan(src, rest, pos),
        |t, _| t,
    );
    let mut errors: Vec<ParseError> = parsed
        .errors
        .iter()
        .map(|e| ParseError::syntax(e.start, e.len, e.message.clone()))
        .collect();
    errors.append(&mut result.errors);
    errors.sort_by_key(|e| e.start);
    result.errors = errors;
    result
}

/// Nodes whose span needs a look.
#[derive(Clone, Copy, PartialEq)]
enum Special {
    None,
    /// `{ x: }`: the value shares the key's span, so it and its parts are left out.
    Implicit,
    /// A `begin` block, or the body of a `def` or block that has `rescue` or `ensure`.
    Begin,
    Regex,
    /// A local variable assigned by a named capture of a regexp (`/(?<a>.)/ =~ s`), with the span
    /// of the regexp.
    Target,
}

fn specials() -> &'static [Special] {
    static SPECIALS: OnceLock<Vec<Special>> = OnceLock::new();
    SPECIALS.get_or_init(|| {
        let special = |name: &str| match name {
            "ImplicitNode" => Special::Implicit,
            "BeginNode" => Special::Begin,
            "RegularExpressionNode" | "InterpolatedRegularExpressionNode" => Special::Regex,
            "LocalVariableTargetNode" => Special::Target,
            _ => Special::None,
        };
        node_names().iter().map(|n| special(n)).collect()
    })
}

/// The nodes of the tree but the program (the root) and what is inside a heredoc body.
fn nodes(src: &str, raw: &[RawNode], bodies: &[(u32, u32)]) -> Vec<Node> {
    let (names, specials) = (node_names(), specials());
    let mut kinds = KindNames::new(grammars::RUBY.get(), names.len(), |_| false);
    let mut out = Vec::with_capacity(raw.len());
    let (mut regexps, mut skip_to) = (Vec::new(), 0);
    for (i, n) in raw.iter().enumerate().skip(1) {
        let at = bodies.partition_point(|&(start, _)| start <= n.start);
        if n.start < skip_to || (at > 0 && n.start < bodies[at - 1].1) {
            continue;
        }
        let (mut start, mut end) = (n.start, n.end);
        match specials[n.ty as usize] {
            Special::None => {}
            Special::Implicit => {
                skip_to = n.end;
                continue;
            }
            Special::Regex => regexps.push((n.start, n.end)),
            Special::Target if regexps.contains(&(n.start, n.end)) => continue,
            Special::Target => {}
            // Prism gives the body of a `def` with `rescue` the span of the whole `def`: the
            // node is what its children cover.
            Special::Begin if !is_begin_keyword(&src[start as usize..]) => {
                let inside = raw[i + 1..].iter().take_while(|c| c.start < n.end);
                (start, end) =
                    inside.fold((u32::MAX, 0), |(s, e), c| (s.min(c.start), e.max(c.end)));
                if start > end {
                    continue;
                }
            }
            Special::Begin => {}
        }
        let kind = kinds.get(n.ty as usize, false, || names[n.ty as usize].clone());
        out.extend(kind.map(|kind| Node { start, end, kind }));
    }
    out
}

/// Whether the text starts with the keyword `begin`.
fn is_begin_keyword(text: &str) -> bool {
    text.strip_prefix("begin")
        .is_some_and(|r| !r.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
}

/// What a token type becomes.
#[derive(Clone, Copy)]
enum Class {
    /// Trivia (comments, newlines) and zero-width markers: `replay` rebuilds the text.
    Skip,
    Kind(SyntaxKind),
    /// `__END__`: Prism's token runs to the end of the file, the data is a token of its own.
    EndMarker,
    HeredocStart,
    HeredocEnd,
}

/// The class of each token type, by the name Prism gives it.
fn classes() -> &'static [Class] {
    static CLASSES: OnceLock<Vec<Class>> = OnceLock::new();
    CLASSES.get_or_init(|| token_names().iter().map(|n| class(n)).collect())
}

fn class(name: &str) -> Class {
    let kind = match name {
        "" | "EOF" | "NEWLINE" | "IGNORED_NEWLINE" | "NEWLINE_TERMINATOR" | "COMMENT"
        | "EMBDOC_BEGIN" | "EMBDOC_LINE" | "EMBDOC_END" | "WORDS_SEP" | "WORDS_SEP_IMPLICIT" => {
            return Class::Skip;
        }
        "HEREDOC_START" => return Class::HeredocStart,
        "HEREDOC_END" => return Class::HeredocEnd,
        "__END__" => return Class::EndMarker,
        "IDENTIFIER" | "METHOD_NAME" => k::IDENTIFIER,
        "CONSTANT" => k::CONSTANT,
        "INSTANCE_VARIABLE" | "CLASS_VARIABLE" | "GLOBAL_VARIABLE" | "BACK_REFERENCE"
        | "NUMBERED_REFERENCE" => k::VARIABLE,
        "LABEL" => k::LABEL,
        "STRING_CONTENT" | "CHARACTER_LITERAL" => k::STRING_CONTENT,
        "STRING_BEGIN" | "STRING_END" | "SYMBOL_BEGIN" | "REGEXP_BEGIN" | "REGEXP_END"
        | "XSTRING_BEGIN" | "BACKTICK" | "LABEL_END" | "PERCENT_LOWER_I" | "PERCENT_LOWER_W"
        | "PERCENT_LOWER_X" | "PERCENT_UPPER_I" | "PERCENT_UPPER_W" => k::STRING_DELIM,
        "EMBEXPR_BEGIN" | "EMBVAR" => k::INTERP_OPEN,
        "EMBEXPR_END" => k::INTERP_CLOSE,
        "PARENTHESIS_LEFT" | "PARENTHESIS_LEFT_GROUPING" | "PARENTHESIS_LEFT_PARENTHESES" => {
            k::L_PAREN
        }
        "PARENTHESIS_RIGHT" => k::R_PAREN,
        "BRACKET_LEFT" | "BRACKET_LEFT_ARRAY" => k::L_BRACKET,
        "BRACKET_RIGHT" => k::R_BRACKET,
        "BRACE_LEFT" | "BRACE_LEFT_ARGUMENT" | "BRACE_LEFT_HASH" | "LAMBDA_BEGIN" => k::L_BRACE,
        "BRACE_RIGHT" => k::R_BRACE,
        "COMMA" => k::COMMA,
        "SEMICOLON" => k::SEMI,
        "DOT" | "AMPERSAND_DOT" => k::DOT,
        "COLON_COLON" | "UCOLON_COLON" => k::COLON_COLON,
        "COLON" => k::COLON,
        "MINUS_GREATER" => k::ARROW,
        "EQUAL_GREATER" => k::DOUBLE_ARROW,
        "EQUAL" => k::ASSIGN,
        "PIPE" => k::PIPE,
        "QUESTION_MARK" => k::QUESTION,
        _ if name.starts_with("KEYWORD_") => k::KEYWORD,
        _ if name.starts_with("INTEGER") || name.starts_with("FLOAT") => k::NUMBER,
        _ => k::OPERATOR,
    };
    Class::Kind(kind)
}

/// The tokens of the file in source order, trivia left out, and the span of each heredoc body
/// (one token, opener to closing label).
fn tokens(src: &str, raw: &[RawToken]) -> (Vec<Tok>, Vec<(u32, u32)>) {
    let classes = classes();
    let mut out: Vec<Tok> = Vec::with_capacity(src.len() / 4);
    let mut bodies = Vec::new();
    // The open heredocs (a heredoc inside the interpolation of another one nests) and the start
    // of the outermost body.
    let (mut depth, mut body) = (0u32, None::<u32>);
    for &RawToken { ty, start, end } in raw {
        let class = classes.get(ty as usize).copied().unwrap_or(Class::Skip);
        if depth > 0 {
            match class {
                Class::HeredocStart => depth += 1,
                Class::HeredocEnd => {
                    depth -= 1;
                    if depth == 0 {
                        let start = body.take().unwrap_or(start);
                        out.push(Tok {
                            kind: k::HEREDOC_BODY,
                            start,
                            end,
                        });
                        bodies.push((start, end));
                    }
                }
                _ => {
                    body.get_or_insert(start);
                }
            }
            continue;
        }
        match class {
            Class::Skip | Class::HeredocEnd => {}
            _ if start == end => {}
            Class::Kind(kind) => out.push(Tok { kind, start, end }),
            Class::EndMarker => {
                out.push(Tok {
                    kind: k::KEYWORD,
                    start,
                    end: start + "__END__".len() as u32,
                });
            }
            Class::HeredocStart => {
                out.push(Tok {
                    kind: k::HEREDOC_START,
                    start,
                    end,
                });
                (depth, body) = (1, None);
            }
        }
    }
    if !bodies.is_empty() {
        out.sort_by_key(|t| t.start);
        bodies.sort_unstable();
    }
    (out, bodies)
}

/// A comment, block comment or line continuation at the start of the text between two tokens.
fn scan(src: &str, rest: &str, pos: usize) -> Option<(SyntaxKind, usize)> {
    let line_end = |s: &str| s.find(['\n', '\r']).unwrap_or(s.len());
    match rest.as_bytes()[0] {
        b'#' => Some((k::COMMENT, line_end(rest))),
        b'=' if rest.starts_with("=begin") && (pos == 0 || src.as_bytes()[pos - 1] == b'\n') => {
            let end = rest
                .find("\n=end")
                .map_or(rest.len(), |i| i + 1 + line_end(&rest[i + 1..]));
            Some((k::COMMENT, end))
        }
        b'\\' => {
            let n = if rest.starts_with("\\\r\n") { 3 } else { 2 };
            rest.as_bytes()
                .get(n - 1)
                .filter(|&&b| b == b'\n')
                .map(|_| (k::WHITESPACE, n))
        }
        _ => None,
    }
}
