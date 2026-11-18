//! The kit of the hand-written lossless parsers: a lexer base, the token stream with the trivia
//! each real token owns (by the attachment rule of [`crate::trivia`]), and the parser state
//! (cursor, tree builder, depth guard, one diagnostic per error site).
//!
//! A language lexes its text into [`Token`]s (every byte in exactly one token, an `EOF` token
//! last) with [`Lexer`], then parses them with [`Parser`], which puts every token, trivia
//! included, in the tree.

use crate::trivia;
use crate::{Assoc, Precedence, SyntaxKind, SyntaxNode, TreeBuilder};
use std::ops::Range;

/// Deeper nesting is an error: the rest of the input is kept as one `ERROR` node. This bounds
/// recursion in the parser, the formatter and tree destruction.
pub const MAX_DEPTH: usize = 256;

/// A syntax error: a byte range into the source text, a stable code and a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub start: u32,
    pub len: u32,
    pub code: String,
    pub message: String,
}

impl ParseError {
    pub fn new(start: u32, len: u32, code: impl Into<String>, message: impl Into<String>) -> Self {
        ParseError {
            start,
            len,
            code: code.into(),
            message: message.into(),
        }
    }

    /// A syntax error of a parser that has no codes for them.
    #[inline]
    pub fn syntax(start: u32, len: u32, message: impl Into<String>) -> Self {
        Self::new(start, len, "syntax-error", message)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: SyntaxKind,
    pub start: usize,
    pub end: usize,
}

/// What a parser returns: the tree and its errors, sorted by position.
pub struct Parsed {
    pub tree: SyntaxNode,
    pub errors: Vec<ParseError>,
}

/// The lexer base: the text, the position, and the tokens and errors found so far. A language's
/// lexer wraps it (and derefs to it) and adds its own scanning.
pub struct Lexer<'a> {
    pub src: &'a str,
    pub pos: usize,
    pub tokens: Vec<Token>,
    pub errors: Vec<ParseError>,
}

impl<'a> Lexer<'a> {
    /// `tokens` is the capacity to reserve for the tokens.
    pub fn new(src: &'a str, tokens: usize) -> Self {
        Lexer {
            src,
            pos: 0,
            tokens: Vec::with_capacity(tokens),
            errors: Vec::new(),
        }
    }

    /// The byte `i` ahead of the position, 0 at the end.
    #[inline]
    pub fn at(&self, i: usize) -> u8 {
        self.src.as_bytes().get(self.pos + i).copied().unwrap_or(0)
    }

    #[inline]
    pub fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    #[inline]
    pub fn peek_at(&self, offset: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(offset)
    }

    #[inline]
    pub fn eat_while(&mut self, f: impl Fn(char) -> bool) {
        while let Some(c) = self.peek() {
            if !f(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
    }

    /// A token of `kind` from `start` to the position.
    #[inline]
    pub fn emit(&mut self, kind: SyntaxKind, start: usize) {
        self.tokens.push(Token {
            kind,
            start,
            end: self.pos,
        });
    }

    /// An error on `start..end`.
    pub fn error_to(
        &mut self,
        start: usize,
        end: usize,
        code: &'static str,
        message: impl Into<String>,
    ) {
        self.errors.push(ParseError::new(
            start as u32,
            (end - start) as u32,
            code,
            message,
        ));
    }

    /// An error on `start..` the position.
    #[inline]
    pub fn error(&mut self, start: usize, code: &'static str, message: impl Into<String>) {
        self.error_to(start, self.pos, code, message);
    }

    /// The tokens, ended by the `eof` token, and the errors.
    pub fn finish(mut self, eof: SyntaxKind) -> (Vec<Token>, Vec<ParseError>) {
        let end = self.src.len();
        self.tokens.push(Token {
            kind: eof,
            start: end,
            end,
        });
        (self.tokens, self.errors)
    }
}

/// The parser state over a lexed text.
pub struct Parser<'a> {
    pub src: &'a str,
    tokens: Vec<Token>,
    /// Indices into `tokens` of the non-trivia tokens.
    real: Vec<usize>,
    /// Trivia token ranges owned by each real token.
    leading: Vec<Range<usize>>,
    trailing: Vec<Range<usize>>,
    eof: SyntaxKind,
    /// The index (in the real tokens) of the current token.
    pub pos: usize,
    /// How many levels deep the parser is, as counted by [`Parser::enter`].
    pub depth: usize,
    pub builder: TreeBuilder,
    pub errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    /// `errors` are those of the lexer; they are kept as they are.
    pub fn new(
        src: &'a str,
        (tokens, errors): (Vec<Token>, Vec<ParseError>),
        is_trivia: impl Fn(SyntaxKind) -> bool,
        eof: SyntaxKind,
    ) -> Self {
        let trivia::Attached {
            real,
            leading,
            trailing,
        } = trivia::attach(
            tokens.len(),
            |i| is_trivia(tokens[i].kind),
            |i| &src[tokens[i].start..tokens[i].end],
        );
        Parser {
            src,
            tokens,
            real,
            leading,
            trailing,
            eof,
            pos: 0,
            depth: 0,
            builder: TreeBuilder::new(),
            errors,
        }
    }

    /// Every token, trivia included, as lexed.
    #[inline]
    pub fn all_tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// The `i`th real token.
    #[inline]
    pub fn tok(&self, i: usize) -> Token {
        self.tokens[self.real[i]]
    }

    #[inline]
    pub fn kind_at(&self, i: usize) -> SyntaxKind {
        self.tokens[self.real[i]].kind
    }

    #[inline]
    pub fn text_at(&self, i: usize) -> &'a str {
        let t = self.tok(i);
        &self.src[t.start..t.end]
    }

    #[inline]
    pub fn cur(&self) -> SyntaxKind {
        self.kind_at(self.pos)
    }

    #[inline]
    pub fn at_eof(&self) -> bool {
        self.cur() == self.eof
    }

    /// The index of the `EOF` token.
    #[inline]
    pub fn eof_index(&self) -> usize {
        self.real.len() - 1
    }

    /// The trivia tokens the `i`th real token owns before and after itself.
    #[inline]
    pub fn leading(&self, i: usize) -> impl Iterator<Item = Token> + '_ {
        self.leading[i].clone().map(|j| self.tokens[j])
    }

    #[inline]
    pub fn trailing(&self, i: usize) -> impl Iterator<Item = Token> + '_ {
        self.trailing[i].clone().map(|j| self.tokens[j])
    }

    #[inline]
    pub fn start(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind);
    }

    pub fn finish(&mut self) {
        self.builder.finish_node();
    }

    #[inline]
    fn put(&mut self, i: usize) {
        let t = self.tokens[i];
        self.builder.token(t.kind, &self.src[t.start..t.end]);
    }

    /// Consumes the current token with its leading and trailing trivia.
    #[inline]
    pub fn bump(&mut self) {
        let (lead, trail) = (
            self.leading[self.pos].clone(),
            self.trailing[self.pos].clone(),
        );
        for i in lead {
            self.put(i);
        }
        self.put(self.real[self.pos]);
        for i in trail {
            self.put(i);
        }
        if !self.at_eof() {
            self.pos += 1;
        }
    }

    /// An error on the `i`th real token; see [`Parser::error_range`].
    #[inline]
    pub fn error_at(&mut self, i: usize, code: &str, message: impl Into<String>) {
        let t = self.tok(i);
        self.error_range(t.start..t.end, code, message);
    }

    /// An error on a byte range. One diagnostic per error site: a report that overlaps an earlier
    /// one (a lexer error on the same token, or a second complaint about the same position; an
    /// empty range counts as one byte) is dropped.
    pub fn error_range(&mut self, r: Range<usize>, code: &str, message: impl Into<String>) {
        let (start, end) = (r.start as u32, (r.end as u32).max(r.start as u32 + 1));
        if self
            .errors
            .iter()
            .any(|e| e.start < end && start < e.start + e.len.max(1))
        {
            return;
        }
        self.errors.push(ParseError {
            start,
            len: (r.end - r.start) as u32,
            code: code.into(),
            message: message.into(),
        });
    }

    /// [`Parser::error_at`] on the current token.
    #[inline]
    pub fn error(&mut self, code: &str, message: impl Into<String>) {
        self.error_at(self.pos, code, message);
    }

    /// An `ERROR` node of the tokens up to (not including) real token `end`.
    pub fn error_until(&mut self, end: usize) {
        self.builder.start_node(SyntaxKind::ERROR);
        while self.pos < end {
            self.bump();
        }
        self.builder.finish_node();
    }

    /// An `ERROR` node of the rest of the input.
    #[inline]
    pub fn error_rest(&mut self) {
        self.error_until(self.eof_index());
    }

    /// Goes one level deeper; `false` (and no change) at `max`.
    #[inline]
    pub fn enter(&mut self, max: usize) -> bool {
        let ok = self.depth < max;
        self.depth += usize::from(ok);
        ok
    }

    #[inline]
    pub fn leave(&mut self) {
        self.depth -= 1;
    }

    /// The tree and the sorted errors.
    pub fn done(mut self) -> Parsed {
        self.errors.sort_by_key(|e| e.start);
        Parsed {
            tree: SyntaxNode::new_root(self.builder.finish()),
            errors: self.errors,
        }
    }
}

/// The binary operator at the current token, as a [`Pratt`] host reads it: its `precedence` line
/// of the grammar and how many tokens it takes.
#[derive(Clone, Copy, Debug)]
pub struct Operator {
    pub prec: Precedence,
    pub tokens: usize,
}

/// A parser with binary operators ([`climb`]): the host knows its tokens and its tree, the
/// `precedence` lines of its grammar give the levels.
pub trait Pratt {
    /// Where an operand starts (a tree builder checkpoint).
    type Mark: Copy;

    fn mark(&mut self) -> Self::Mark;

    /// Parses the operand that binds tighter than every binary operator.
    fn operand(&mut self);

    /// The binary operator at the current token.
    fn operator(&self) -> Option<Operator>;

    /// Wraps what was parsed since `at` and the operator: the host consumes the operator, then
    /// calls [`climb`] with [`Operator::rhs_min`] for the right operand.
    fn infix(&mut self, at: Self::Mark, op: Operator);
}

impl Operator {
    /// The loosest level the right operand may contain.
    pub fn rhs_min(&self) -> u8 {
        self.prec.level + u8::from(self.prec.assoc != Assoc::Right)
    }
}

/// Parses operands joined by the operators whose level is at least `min`.
pub fn climb<P: Pratt>(p: &mut P, min: u8) {
    let at = p.mark();
    p.operand();
    while let Some(op) = p.operator() {
        if op.prec.level < min {
            break;
        }
        p.infix(at, op);
    }
}
