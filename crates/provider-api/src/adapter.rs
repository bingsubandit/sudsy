//! What an adapter from a parser with its own AST and tokens (oxc, ruff) shares: the lossless
//! tree from the parser's tokens, the spans of its AST nodes and the text between tokens.
//!
//! The parser has no punctuation nodes and no trivia. The adapter takes its tokens (every token
//! of the file, with its span), the spans of the AST nodes in pre-order, and [`replay`] does the
//! rest: a node opens before the first token that starts in it and closes after its last, so a
//! token belongs to the innermost node that contains it; the text between two tokens holds only
//! whitespace, comments and what the language's `scan` recognizes, and is split into trivia
//! tokens that follow the shared attachment rule (`core_syntax::trivia`), the way
//! `core-treesitter` does.

use crate::provider::{ParseError, ParseResult};
use core_syntax::trivia::is_line_break;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TreeBuilder};

/// A token of the parser: our kind and its span.
#[derive(Clone, Copy)]
pub struct Tok {
    pub kind: SyntaxKind,
    pub start: u32,
    pub end: u32,
}

/// An AST node: its span and our kind. A node with no text is an empty node.
pub struct Node {
    pub start: u32,
    pub end: u32,
    pub kind: SyntaxKind,
}

/// The kinds `replay` itself emits.
#[derive(Clone, Copy)]
pub struct Kinds {
    pub root: SyntaxKind,
    /// The zero-width token that ends the root and owns the trivia of the tail.
    pub eof: SyntaxKind,
    pub unknown: SyntaxKind,
    pub whitespace: SyntaxKind,
    pub newline: SyntaxKind,
}

/// Our kinds for the parser's, found by name in the grammar on first use. `index` is the parser's
/// kind as a number (`kind as usize`).
pub struct KindNames {
    grammar: &'static Grammar,
    /// Kinds of the parser that have no row in the grammar on purpose.
    exempt: fn(&str) -> bool,
    slots: Vec<Option<Option<SyntaxKind>>>,
}

impl KindNames {
    pub fn new(grammar: &'static Grammar, kinds: usize, exempt: fn(&str) -> bool) -> Self {
        KindNames {
            grammar,
            exempt,
            slots: vec![None; kinds],
        }
    }

    /// The token (`token`) or node kind of the grammar called `name`; `None` when it has none. A
    /// name the grammar lacks is a bug in debug builds (a new parser version) unless `exempt`
    /// allows it.
    pub fn get(
        &mut self,
        index: usize,
        token: bool,
        name: impl FnOnce() -> String,
    ) -> Option<SyntaxKind> {
        let (grammar, exempt) = (self.grammar, self.exempt);
        *self.slots[index].get_or_insert_with(|| {
            let name = name();
            let found = grammar
                .kind_by_name(&name)
                .filter(|k| k.is_token() == token);
            debug_assert!(
                found.is_some() || exempt(&name),
                "{name} has no {} in the grammar",
                if token { "token" } else { "node" }
            );
            found
        })
    }
}

/// The trivia attachment loop shared with `core-treesitter`: each real token takes the trivia
/// before it, itself, and the trivia after it up to the end of its line; the rest of the gap is
/// carried to the next token. `lex(a, b, out)` splits the text `a..b` between two tokens into
/// trivia tokens.
pub struct Attach<'a> {
    src: &'a str,
    carry: Vec<Tok>,
    gap: Vec<Tok>,
}

impl<'a> Attach<'a> {
    pub fn new(src: &'a str) -> Self {
        Attach {
            src,
            carry: Vec::new(),
            gap: Vec::new(),
        }
    }

    /// Everything before the first real token is its leading trivia.
    pub fn begin(&mut self, first: u32, mut lex: impl FnMut(u32, u32, &mut Vec<Tok>)) {
        lex(0, first, &mut self.carry);
    }

    /// Emits the carried trivia, `t`, and its trailing trivia before the token that starts at
    /// `next` (none after the last).
    pub fn token(
        &mut self,
        b: &mut TreeBuilder,
        t: Tok,
        next: Option<u32>,
        mut lex: impl FnMut(u32, u32, &mut Vec<Tok>),
    ) {
        let src = self.src;
        let emit =
            |b: &mut TreeBuilder, t: &Tok| b.token(t.kind, &src[t.start as usize..t.end as usize]);
        self.carry.drain(..).for_each(|c| emit(b, &c));
        emit(b, &t);
        if let Some(next) = next {
            self.gap.clear();
            lex(t.end, next, &mut self.gap);
            // ASCII last bytes decide without decoding.
            let ends_line = |c: &Tok| match src.as_bytes()[c.end as usize - 1] {
                b'\n' | b'\r' => true,
                0..=0x7f => false,
                _ => src[..c.end as usize]
                    .chars()
                    .next_back()
                    .is_some_and(is_line_break),
            };
            let cut = (self.gap.iter().position(ends_line)).map_or(self.gap.len(), |i| i + 1);
            self.gap[..cut].iter().for_each(|c| emit(b, c));
            self.carry.extend_from_slice(&self.gap[cut..]);
        }
    }
}

/// A file the parser gave up on: one `Unknown` token under the root, with the reason as the error.
pub fn whole_file(src: &str, kinds: Kinds, message: impl Into<String>) -> ParseResult {
    let mut builder = TreeBuilder::new();
    builder.start_node(kinds.root);
    if !src.is_empty() {
        builder.token(kinds.unknown, src);
    }
    builder.token(kinds.eof, "");
    builder.finish_node();
    ParseResult {
        tree: SyntaxNode::new_root(builder.finish()),
        errors: vec![ParseError::syntax(0, src.len() as u32, message)],
    }
}

/// Builds the tree and the errors of the text the tokens do not cover. `scan(rest, pos)` reads
/// the language's own trivia (comments, line continuations) at the start of `rest`, which is the
/// text from `pos` to the next token; whitespace and line breaks are recognized here. `retag` may
/// change a token's kind given the kind of the innermost open node.
pub fn replay(
    src: &str,
    kinds: Kinds,
    tokens: &[Tok],
    nodes: &[Node],
    scan: impl Fn(&str, usize) -> Option<(SyntaxKind, usize)>,
    retag: impl Fn(Tok, Option<SyntaxKind>) -> Tok,
) -> ParseResult {
    let mut lex = Lexer {
        src,
        kinds,
        scan,
        errors: Vec::new(),
    };
    let mut builder = TreeBuilder::new();
    run(&mut lex, &mut builder, tokens, nodes, retag);
    ParseResult {
        tree: SyntaxNode::new_root(builder.finish()),
        errors: lex.errors,
    }
}

/// Replays tokens and nodes: each token takes its leading trivia, itself and its trailing
/// trivia, inside the innermost node that contains it.
fn run<S: Fn(&str, usize) -> Option<(SyntaxKind, usize)>>(
    lex: &mut Lexer<'_, S>,
    builder: &mut TreeBuilder,
    tokens: &[Tok],
    nodes: &[Node],
    retag: impl Fn(Tok, Option<SyntaxKind>) -> Tok,
) {
    let len = lex.src.len() as u32;
    builder.start_node(lex.kinds.root);
    let eof = Tok {
        kind: lex.kinds.eof,
        start: len,
        end: len,
    };
    let mut nodes = nodes.iter().peekable();
    // The end and kind of each open node, innermost last.
    let mut open: Vec<(u32, SyntaxKind)> = Vec::new();
    let mut attach = Attach::new(lex.src);
    attach.begin(tokens.first().map_or(len, |t| t.start), |a, b, out| {
        lex.gap(a, b, out)
    });
    let mut real = tokens.iter().copied().chain(Some(eof)).peekable();
    while let Some(t) = real.next() {
        while open.last().is_some_and(|&(end, _)| end <= t.start) {
            open.pop();
            builder.finish_node();
        }
        while let Some(n) = nodes.next_if(|n| n.start <= t.start) {
            if n.start == n.end {
                builder.start_node(n.kind);
                builder.finish_node();
            } else if n.end > t.start {
                // A node that overlaps its parent ends with it.
                let end = open.last().map_or(n.end, |&(p, _)| p.min(n.end));
                builder.start_node(n.kind);
                open.push((end, n.kind));
            }
        }
        let t = retag(t, open.last().map(|o| o.1));
        let next = real.peek().map(|n| n.start);
        attach.token(builder, t, next, |a, b, out| lex.gap(a, b, out));
    }
    builder.finish_node();
}

struct Lexer<'a, S> {
    src: &'a str,
    kinds: Kinds,
    scan: S,
    errors: Vec<ParseError>,
}

impl<S: Fn(&str, usize) -> Option<(SyntaxKind, usize)>> Lexer<'_, S> {
    /// Splits the text `a..b` between two tokens into trivia tokens.
    fn gap(&mut self, a: u32, b: u32, out: &mut Vec<Tok>) {
        let src = self.src;
        let (mut pos, b) = (a as usize, b as usize);
        let blank = |c: char| c.is_whitespace() || c == '\u{feff}';
        let line_break = |c: char| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
        while pos < b {
            let rest = &src[pos..b];
            let c = rest.chars().next().unwrap_or(' ');
            let Kinds {
                whitespace,
                newline,
                unknown,
                ..
            } = self.kinds;
            // The common gap, spaces up to an ASCII character, needs no scan.
            let spaces = rest.bytes().take_while(|&c| c == b' ').count();
            let (kind, len) = if spaces > 0 && rest.as_bytes().get(spaces).is_none_or(u8::is_ascii)
            {
                (whitespace, spaces)
            } else if let Some(found) = (self.scan)(rest, pos) {
                found
            } else if rest.starts_with("\r\n") {
                (newline, 2)
            } else if line_break(c) {
                (newline, c.len_utf8())
            } else if blank(c) {
                let n = rest.find(|c| !blank(c) || line_break(c));
                (whitespace, n.unwrap_or(rest.len()))
            } else {
                let n = rest.find(char::is_whitespace).unwrap_or(rest.len());
                self.errors
                    .push(ParseError::syntax(pos as u32, n as u32, "unexpected text"));
                (unknown, n)
            };
            out.push(Tok {
                kind,
                start: pos as u32,
                end: (pos + len) as u32,
            });
            pos += len;
        }
    }
}
