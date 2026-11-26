//! The kit of the hand-written lossless parsers: a lexer base, the token stream with the trivia
//! each real token owns (by the attachment rule of [`crate::trivia`]), and the parser state
//! (cursor, tree builder, depth guard, one diagnostic per error site).
//!
//! A language lexes its text into [`Token`]s (every byte in exactly one token, an `EOF` token
//! last) with [`Lexer`], then parses them with [`Parser`], which puts every token, trivia
//! included, in the tree.
//!
//! concept: hand-written-parser-kit
//! what: Lexer base, token stream with trivia, parser cursor and Pratt climbing for hand-written lossless parsers.
//! tools: none (parser plumbing)
//! reads: grammar:precedence

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
/// A blank byte: space or tab.
#[inline]
pub fn is_blank(c: u8) -> bool {
    matches!(c, b' ' | b'\t')
}

/// A blank byte of a lexer that counts the vertical tab and the form feed as blanks too.
#[inline]
pub fn is_blank_form(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | 0x0b | 0x0c)
}

/// [`is_blank_form`] for a char (an ASCII one).
#[inline]
pub fn is_blank_form_char(c: char) -> bool {
    c.is_ascii() && is_blank_form(c as u8)
}

/// Whitespace as a lexer makes trivia of it: a blank or a line break.
#[inline]
pub fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// [`is_space`] with the vertical tab and the form feed.
#[inline]
pub fn is_space_form(c: char) -> bool {
    is_space(c) || matches!(c, '\x0b' | '\x0c')
}

/// Bytes of the `escape` byte at `at` and the line break after it (`\n`, `\r\n`, `\r`) in `b`; 0
/// when the escape is not at a line break.
pub fn escaped_break_at(b: &[u8], at: usize, escape: u8) -> usize {
    if b.get(at) != Some(&escape) {
        return 0;
    }
    match (b.get(at + 1), b.get(at + 2)) {
        (Some(b'\r'), Some(b'\n')) => 3,
        (Some(b'\n' | b'\r'), _) => 2,
        _ => 0,
    }
}

/// A line break byte: `\n` or `\r`.
#[inline]
pub fn is_eol(c: u8) -> bool {
    matches!(c, b'\n' | b'\r')
}

/// A bracketed group production: its node, brackets, nesting limit and unclosed error.
pub struct Group {
    pub node: SyntaxKind,
    pub open: SyntaxKind,
    pub close: SyntaxKind,
    /// Plural name in the nesting error (`parentheses are nested deeper than ...`).
    pub what: &'static str,
    pub max: usize,
    /// Code and message of the error on an opener whose closer is missing.
    pub unclosed: (&'static str, &'static str),
}

impl Group {
    /// A group in `(` `)`: the opener and closer kinds, the unclosed error `code`.
    pub const fn parens(
        node: SyntaxKind,
        (open, close): (SyntaxKind, SyntaxKind),
        what: &'static str,
        max: usize,
        code: &'static str,
    ) -> Group {
        Group {
            node,
            open,
            close,
            what,
            max,
            unclosed: (code, "expected `)` for this `(`"),
        }
    }
}

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

    /// Scans the blanks or the line break at the position as one token: a run of the bytes
    /// `is_blank` accepts is a `ws` token, `\n`, `\r\n` or `\r` a `nl` token. `false` (nothing
    /// scanned) at any other byte.
    pub fn blank(&mut self, ws: SyntaxKind, nl: SyntaxKind, is_blank: impl Fn(u8) -> bool) -> bool {
        let start = self.pos;
        let b = self.src.as_bytes();
        match b.get(start) {
            Some(&c) if is_blank(c) => {
                while self.pos < b.len() && is_blank(b[self.pos]) {
                    self.pos += 1;
                }
                self.emit(ws, start);
            }
            Some(b'\r') if b.get(start + 1) == Some(&b'\n') => {
                self.pos += 2;
                self.emit(nl, start);
            }
            Some(b'\n' | b'\r') => {
                self.pos += 1;
                self.emit(nl, start);
            }
            _ => return false,
        }
        true
    }

    /// Bytes of the `escape` byte at the position and the line break after it (see [`escaped_break_at`]).
    pub fn escaped_break(&self, escape: u8) -> usize {
        escaped_break_at(self.src.as_bytes(), self.pos, escape)
    }

    /// Moves the position to the end of the line, over the line breaks that follow `escape` (when
    /// given) as part of it.
    pub fn eat_line_escaped(&mut self, escape: Option<u8>) {
        let len = self.src.len();
        while self.pos < len && !is_eol(self.src.as_bytes()[self.pos]) {
            self.pos += escape.map_or(0, |e| self.escaped_break(e)).max(1);
        }
    }

    /// Moves the position back over the blanks before it, down to `start`.
    pub fn trim_blanks_back(&mut self, start: usize) {
        let b = self.src.as_bytes();
        while self.pos > start && is_blank(b[self.pos - 1]) {
            self.pos -= 1;
        }
    }

    /// Moves the position to the end of the line (before its line break).
    #[inline]
    pub fn eat_line(&mut self) {
        let rest = &self.src.as_bytes()[self.pos..];
        self.pos += rest
            .iter()
            .position(|&c| c == b'\n' || c == b'\r')
            .unwrap_or(rest.len());
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
    /// `errors` are those of the lexer; they are kept as they are. `trivia`: the language's
    /// `kind::TRIVIA_MASK`.
    pub fn new(
        src: &'a str,
        (tokens, errors): (Vec<Token>, Vec<ParseError>),
        trivia: u64,
        eof: SyntaxKind,
    ) -> Self {
        let trivia::Attached {
            real,
            leading,
            trailing,
        } = trivia::attach(
            tokens.len(),
            |i| crate::kind::token_in_mask(trivia, tokens[i].kind),
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

    /// The current real token follows the one before it with nothing in between.
    pub fn touches_prev(&self) -> bool {
        self.pos > 0 && self.tok(self.pos).start == self.tok(self.pos - 1).end
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

    /// [`Parser::bump`] with the token entered in the tree as `kind`: a token that only the
    /// parse can tell (a command name from an argument of the same spelling).
    pub fn bump_as(&mut self, kind: SyntaxKind) {
        let at = self.real[self.pos];
        self.tokens[at].kind = kind;
        self.bump();
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

    /// The `nesting-too-deep` error on the current token: `what` are nested deeper than `max` levels.
    pub fn too_deep(&mut self, what: &str, max: usize) {
        self.error(
            "nesting-too-deep",
            format!("{what} are nested deeper than {max} levels"),
        );
    }

    /// Starts the group at the current opener: its node and the opener, returning the opener's
    /// index; too deep (past `g.max`), the error and the balanced group up to `limit` as one `ERROR`
    /// node, and `None`. The caller parses the items, then [`Parser::group_end`].
    pub fn group_start(&mut self, g: &Group, limit: usize) -> Option<usize> {
        if !self.enter(g.max) {
            self.too_deep(g.what, g.max);
            self.error_group(g.open, g.close, limit);
            return None;
        }
        self.start(g.node);
        let open = self.pos;
        self.bump();
        Some(open)
    }

    /// Ends the group of [`Parser::group_start`]: the closer, or the unclosed error on the opener.
    pub fn group_end(&mut self, g: &Group, open: usize) {
        self.close_group(g, open);
        self.finish();
        self.leave();
    }

    /// The closer of `g` opened at real token `open`, or the unclosed error on the opener.
    pub fn close_group(&mut self, g: &Group, open: usize) {
        if self.cur() == g.close {
            self.bump();
        } else {
            self.error_at(open, g.unclosed.0, g.unclosed.1);
        }
    }

    /// An `ERROR` node of the current token.
    pub fn bump_error(&mut self) {
        self.start(SyntaxKind::ERROR);
        self.bump();
        self.finish();
    }

    /// An `ERROR` node of the rest of the input.
    #[inline]
    pub fn error_rest(&mut self) {
        self.error_until(self.eof_index());
    }

    /// For each real token of kind `open`: the index of the `close` token that pairs with it
    /// (stack matching in one pass), `u32::MAX` when there is none; other tokens hold `u32::MAX`.
    pub fn pairs(&self, open: SyntaxKind, close: SyntaxKind) -> Vec<u32> {
        let mut pairs = vec![u32::MAX; self.eof_index()];
        let mut stack = Vec::new();
        for i in 0..self.eof_index() {
            let kind = self.kind_at(i);
            if kind == open {
                stack.push(i);
            } else if kind == close
                && let Some(o) = stack.pop()
            {
                pairs[o] = i as u32;
            }
        }
        pairs
    }

    /// Whether real token `i` starts at column 0: the byte before it is a line break or there is none.
    pub fn at_line_start(&self, i: usize) -> bool {
        let at = self.tok(i).start;
        at == 0 || matches!(self.src.as_bytes()[at - 1], b'\n' | b'\r')
    }

    /// An `ERROR` node of the balanced group that starts at the current `open` token: up to its
    /// `close` token, or `limit` (a real token index) when it is not closed before.
    pub fn error_group(&mut self, open: SyntaxKind, close: SyntaxKind, limit: usize) {
        self.builder.start_node(SyntaxKind::ERROR);
        let mut depth = 0usize;
        while self.pos < limit && !self.at_eof() {
            let kind = self.cur();
            depth += usize::from(kind == open);
            depth = depth.saturating_sub(usize::from(kind == close));
            self.bump();
            if depth == 0 {
                break;
            }
        }
        self.builder.finish_node();
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
/// of the grammar, how many tokens it takes and the kind the host gives its token.
#[derive(Clone, Copy, Debug)]
pub struct Operator {
    pub prec: Precedence,
    pub tokens: usize,
    pub kind: SyntaxKind,
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

/// Raw text whose closer is derived from its opener (a bracket text `[=[ ... ]=]`, a raw string
/// `R"d( ... )d"`, a dollar quote `$tag$ ... $tag$`): the end (after `closer`) of the first
/// `closer` at or after `from`, `None` when the text has no end.
pub fn raw_end(src: &[u8], from: usize, closer: &[u8]) -> Option<usize> {
    let (first, rest) = closer.split_first()?;
    let mut i = from;
    while let Some(p) = src.get(i..)?.iter().position(|c| c == first) {
        let at = i + p;
        if src[at + 1..].starts_with(rest) {
            return Some(at + closer.len());
        }
        i = at + 1;
    }
    None
}

/// The start of the first line after the one holding `from` whose text (up to the end of the
/// input) satisfies `starts`; the end of the input when there is none. Where an unterminated
/// construct stops at the next line that starts a new statement.
pub fn next_line_where(src: &str, from: usize, starts: impl Fn(&str) -> bool) -> usize {
    let mut at = from;
    while let Some(nl) = src[at..].find('\n') {
        at += nl + 1;
        if starts(&src[at..]) {
            return at;
        }
    }
    src.len()
}

/// Is the sign token ending at `sign_end` the sign of the number starting at `number_start`? The
/// number touches the sign, and the sign starts an operand: a blank comes before it, or an
/// opening token (`(`, `,`: `f(-1)`, `(1, -2)`).
pub fn is_signed_number(
    sign_end: u32,
    number_start: u32,
    blank_before: bool,
    after_opener: bool,
) -> bool {
    sign_end == number_start && (blank_before || after_opener)
}

/// Does only a block comment (`open` .. `close`) stand before `offset` on its line?
pub fn only_block_comment_before(src: &str, offset: usize, open: &str, close: &str) -> bool {
    let line = src[..offset].rsplit('\n').next().unwrap_or("").trim();
    line.starts_with(open) && line.ends_with(close)
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn an_escape_before_a_line_break_takes_the_break() {
        assert_eq!(escaped_break_at(b"a\\\r\nb", 1, b'\\'), 3);
        assert_eq!(escaped_break_at(b"\\\n", 0, b'\\'), 2);
        assert_eq!(escaped_break_at(b"\\\r", 0, b'\\'), 2);
        assert_eq!(escaped_break_at(b"\\x", 0, b'\\'), 0);
        assert_eq!(escaped_break_at(b"\\", 0, b'\\'), 0);
        assert!(is_blank_form(0x0c) && !is_blank(0x0c));
        assert!(
            is_blank_form_char('\u{b}') && !is_blank_form_char('\n') && !is_blank_form_char('é')
        );
        assert!(is_space('\r') && is_space('\t') && !is_space('\u{c}') && is_space_form('\u{c}'));
    }

    #[test]
    fn a_sign_touches_its_number_after_a_blank_or_an_opener() {
        assert!(is_signed_number(5, 5, true, false));
        assert!(is_signed_number(5, 5, false, true));
        assert!(!is_signed_number(5, 5, false, false));
        assert!(!is_signed_number(5, 6, true, true));
    }

    #[test]
    fn only_a_block_comment_may_precede_on_the_line() {
        let src = "x\n/* c */ delimiter\n a /* c */ delimiter";
        let (a, b) = (
            src.find("delimiter").unwrap(),
            src.rfind("delimiter").unwrap(),
        );
        assert!(only_block_comment_before(src, a, "/*", "*/"));
        assert!(!only_block_comment_before(src, b, "/*", "*/"));
        assert!(!only_block_comment_before("x delimiter", 2, "/*", "*/"));
    }

    #[test]
    fn raw_text_ends_at_its_own_closer() {
        let src = b"[==[ a ]] ]=] ]==] ]==]";
        assert_eq!(raw_end(src, 4, b"]==]"), Some(18));
        assert_eq!(raw_end(src, 4, b"]]"), Some(9));
        assert_eq!(raw_end(src, 4, b"]===]"), None);
        assert_eq!(raw_end(src, 19, b"]==]"), Some(23));
        assert_eq!(raw_end(src, 99, b"]"), None);
        assert_eq!(raw_end(b"$a$ x $b$ y $a$", 3, b"$a$"), Some(15));
        assert_eq!(raw_end(b"abc", 0, b""), None);
        let starts = |l: &str| l.starts_with("x(");
        assert_eq!(next_line_where("a\nb\nx(1)\nx(2)", 0, starts), 4);
        assert_eq!(next_line_where("a\nx(1)", 2, starts), 6);
    }

    #[test]
    fn blanks_and_line_breaks_are_tokens_of_their_own() {
        let (ws, nl) = (SyntaxKind(1), SyntaxKind(2));
        let mut lx = Lexer::new("  \t\r\n\n\rx \u{e9}", 0);
        let blank = |lx: &mut Lexer| lx.blank(ws, nl, |c| c == b' ' || c == b'\t');
        while blank(&mut lx) {}
        assert_eq!(lx.pos, 7);
        let kinds: Vec<_> = lx
            .tokens
            .iter()
            .map(|t| (t.kind, t.end - t.start))
            .collect();
        assert_eq!(kinds, [(ws, 3), (nl, 2), (nl, 1), (nl, 1)]);
        lx.pos += 1;
        assert!(blank(&mut lx) && !blank(&mut lx));
        lx.eat_line();
        assert_eq!(lx.pos, lx.src.len());
        let mut lx = Lexer::new("ab\r\ncd", 0);
        lx.eat_line();
        assert_eq!(lx.pos, 2);
    }
}
