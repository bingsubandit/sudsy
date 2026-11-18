//! The Java parser: a recursive-descent parser over the tokens of `lexer` that builds the lossless
//! tree of `grammars/java.grammar` directly. Never fails: an unexpected token is a syntax error
//! (the file stays as written), parsing goes on at the next statement or member, and every token
//! is in the tree.
//!
//! Left-recursive shapes (binary operators, member chains, array access, scoped names) are loops
//! that wrap what was built with a checkpoint, so a chain of 10,000 calls needs no recursion
//! while parsing; recursion that follows the nesting of the source (statements, unary operands,
//! types) goes through `core_format::deep`.

mod decl;
mod expr;
mod look;
mod stmt;
mod types;

use crate::generated::kind as k;
use crate::lexer::{T, Token, lex};
use core_syntax::grammars;
use core_syntax::{Checkpoint, SyntaxKind, SyntaxNode, TreeBuilder};
use provider_api::adapter::{GrammarScan, Stream, Tok};
use provider_api::{ParseError, ParseResult, SourceFile};

pub fn parse_file(file: &SourceFile) -> ParseResult {
    parse(&file.text)
}

pub fn parse(src: &str) -> ParseResult {
    let lexed = lex(src);
    let first = lexed.tokens.first().map_or(src.len() as u32, |t| t.start);
    let mut p = P {
        src,
        toks: lexed.tokens,
        close: lexed.close,
        pos: 0,
        b: TreeBuilder::new(),
        stream: Stream::of(src, grammars::JAVA.get(), first),
        errors: lexed
            .errors
            .into_iter()
            .map(|(s, l, m)| ParseError::syntax(s, l, m))
            .collect(),
        last_error: usize::MAX,
        no_lambda: false,
    };
    p.run();
    let mut errors = p.errors;
    errors.append(&mut p.stream.gap.errors);
    errors.sort_by_key(|e| e.start);
    ParseResult {
        tree: SyntaxNode::new_root(p.b.finish()),
        errors,
    }
}

pub(crate) struct P<'a> {
    src: &'a str,
    toks: Vec<Token>,
    close: Vec<u32>,
    pos: usize,
    b: TreeBuilder,
    stream: Stream<'a, GrammarScan<'a>>,
    errors: Vec<ParseError>,
    last_error: usize,
    /// `case X ->`: a name followed by `->` is not a lambda.
    no_lambda: bool,
}

impl<'a> P<'a> {
    fn run(&mut self) {
        self.b.start_node(k::PROGRAM);
        self.compilation_unit();
        self.stream.finish(&mut self.b);
        self.b.finish_node();
    }

    // Tokens

    /// The kind of the token `n` places ahead.
    fn nth(&self, n: usize) -> T {
        self.toks.get(self.pos + n).map_or(T::Eof, |t| t.kind)
    }

    fn at_idx(&self, i: usize) -> T {
        self.toks.get(i).map_or(T::Eof, |t| t.kind)
    }

    fn cur(&self) -> T {
        self.nth(0)
    }

    fn at(&self, t: T) -> bool {
        self.cur() == t
    }

    fn at_eof(&self) -> bool {
        self.pos >= self.toks.len()
    }

    fn at_name(&self) -> bool {
        self.cur().is_name()
    }

    /// Whether the tokens `i` and `i + 1` touch.
    fn adjacent(&self, i: usize) -> bool {
        match (self.toks.get(i), self.toks.get(i + 1)) {
            (Some(a), Some(b)) => a.end == b.start,
            _ => false,
        }
    }

    /// The tree kind of the current token when nothing else is said.
    fn default_kind(t: T) -> SyntaxKind {
        match t {
            T::Ident => k::IDENTIFIER,
            T::NumberLit => k::NUMBER,
            T::StringLit => k::STRING,
            T::CharLit => k::CHAR,
            T::True | T::False => k::BOOLEAN,
            T::Null => k::NULL,
            T::LBrace => k::L_BRACE,
            T::RBrace => k::R_BRACE,
            T::LParen => k::L_PAREN,
            T::RParen => k::R_PAREN,
            T::LBracket => k::L_BRACKET,
            T::RBracket => k::R_BRACKET,
            T::Semi => k::SEMI,
            T::Comma => k::COMMA,
            T::Dot => k::DOT,
            T::Ellipsis => k::ELLIPSIS,
            T::At => k::AT,
            T::Colon => k::COLON,
            T::ColonColon => k::COLON_COLON,
            T::Arrow => k::ARROW,
            T::Question => k::QUESTION,
            T::Eq => k::ASSIGN,
            T::Lt => k::LT,
            T::Gt => k::GT,
            T::Unknown => k::UNKNOWN,
            T::Eof => k::EOF,
            t if t.is_primitive() => k::PRIMITIVE_TYPE,
            t if t.is_contextual() => k::IDENTIFIER,
            T::Le
            | T::Ge
            | T::EqEq
            | T::Ne
            | T::AndAnd
            | T::OrOr
            | T::Not
            | T::Tilde
            | T::Plus
            | T::Minus
            | T::Star
            | T::Slash
            | T::Percent
            | T::Amp
            | T::Pipe
            | T::Caret
            | T::Shl
            | T::PlusPlus
            | T::MinusMinus
            | T::AssignOp => k::OPERATOR,
            _ => k::KEYWORD,
        }
    }

    /// Emits the tokens `pos..pos + count` as one token of `kind`.
    fn emit(&mut self, kind: SyntaxKind, count: usize) {
        let Some(first) = self.toks.get(self.pos) else {
            return;
        };
        let last = (self.pos + count).min(self.toks.len()) - 1;
        let tok = Tok {
            kind,
            start: first.start,
            end: self.toks[last].end,
        };
        let next = self
            .toks
            .get(last + 1)
            .map_or(self.src.len() as u32, |t| t.start);
        self.stream.token(&mut self.b, tok, next);
        self.pos = last + 1;
    }

    fn bump(&mut self) {
        self.emit(Self::default_kind(self.cur()), 1);
    }

    fn bump_as(&mut self, kind: SyntaxKind) {
        self.emit(kind, 1);
    }

    /// The current name or reserved word as a keyword token.
    fn bump_kw(&mut self) {
        self.emit(k::KEYWORD, 1);
    }

    fn eat(&mut self, t: T) -> bool {
        let found = self.at(t);
        if found {
            self.bump();
        }
        found
    }

    /// Consumes `t` or records the error.
    fn expect(&mut self, t: T, what: &str) -> bool {
        if self.eat(t) {
            return true;
        }
        self.error(&format!("expected {what}"));
        false
    }

    fn error(&mut self, message: &str) {
        if self.last_error == self.pos {
            return;
        }
        self.last_error = self.pos;
        let (start, len) = match self.toks.get(self.pos) {
            Some(t) => (t.start, t.end - t.start),
            None => (self.src.len() as u32, 0),
        };
        self.errors.push(ParseError::syntax(start, len, message));
    }

    /// Skips the current token as an error, so a loop always moves on.
    fn bump_error(&mut self, message: &str) {
        self.error(message);
        if !self.at_eof() {
            self.bump();
        }
    }

    // Nodes

    fn start(&mut self, kind: SyntaxKind) {
        self.b.start_node(kind);
    }

    fn finish(&mut self) {
        self.b.finish_node();
    }

    fn cp(&self) -> Checkpoint {
        self.b.checkpoint()
    }

    /// A node around what `f` parses.
    fn node(&mut self, kind: SyntaxKind, f: impl FnOnce(&mut Self)) {
        self.b.start_node(kind);
        f(self);
        self.b.finish_node();
    }

    /// A node around everything built since `cp`, and what `f` parses after it.
    fn wrap(&mut self, cp: Checkpoint, kind: SyntaxKind, f: impl FnOnce(&mut Self)) {
        self.b.start_node_at(cp, kind);
        f(self);
        self.b.finish_node();
    }
}
