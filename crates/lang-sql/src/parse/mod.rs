//! The SQL parser: one hand-written recursive-descent parser for the ansi, postgres, mysql and
//! tsql dialects (modes of the one parser, `dialect.rs`), lossless and error tolerant.
//!
//! `lexer` tiles the file with tokens; the parser walks the code tokens, names each one (keyword,
//! identifier, function, type, ...), opens the grammar's nodes and puts the zero-width `Indent`,
//! `ImplicitIndent` and `Dedent` the layout reads; `tree` replays that into the lossless tree. A
//! statement that is not understood to its end becomes an `Error` node up to the next `;`; the
//! rest of the file still parses. Nothing panics: every loop consumes a token or ends.
//!
//! concept: recursive-descent-parser

mod bits;
mod clauses;
mod dialect;
mod expr;
mod generic;
mod lexer;
mod merge;
mod mysql;
mod postgres;
mod stmt;
pub(crate) mod tree;
mod tsql;
mod types;

use crate::generated::kind as k;
use core_syntax::SyntaxKind;
use lexer::{Lex, T, lex};
use provider_api::ParseError;
use tree::{Flat, Leaf, Span};

pub use dialect::Dialect;

/// Parses `src` in `dialect` into the flat form `tree::build` replays.
pub(crate) fn parse_flat(src: &str, d: Dialect) -> Flat {
    let mut p = P::new(src, d);
    stmt::file(&mut p);
    p.finish()
}

/// The dialects, as `dialect` option values.
pub const DIALECTS: [&str; 4] = ["ansi", "postgres", "mysql", "tsql"];

/// Parses `src` in the dialect named `dialect` into a tree.
pub fn parse(src: &str, dialect: &str) -> provider_api::ParseResult {
    tree::build(src, parse_flat(src, Dialect::from_name(dialect)))
}

/// A file of the provider: the dialect is the `dialect` option.
pub fn parse_file(file: &provider_api::SourceFile) -> provider_api::ParseResult {
    parse(
        &file.text,
        file.language_version.as_deref().unwrap_or("ansi"),
    )
}

/// A saved state to return to when an attempt does not parse.
#[derive(Clone, Copy)]
pub(crate) struct Cp {
    pos: usize,
    nodes: usize,
    metas: usize,
    errors: usize,
}

struct Meta {
    pos: u32,
    kind: SyntaxKind,
    group: u32,
}

pub(crate) struct P<'a> {
    pub src: &'a str,
    pub d: Dialect,
    lex: Vec<Lex>,
    /// Indices into `lex` of the tokens that are not trivia.
    code: Vec<u32>,
    pub pos: usize,
    kinds: Vec<SyntaxKind>,
    nodes: Vec<(SyntaxKind, u32, u32)>,
    metas: Vec<Meta>,
    group: u32,
    errors: Vec<ParseError>,
    /// Nesting of brackets and expressions being parsed.
    pub depth: u32,
    /// The first argument of the call about to open is a type (`CONVERT(int, x)`).
    pub type_first: bool,
    /// The statement table's row of the statement walked generically.
    pub row: Option<&'static std::collections::HashSet<&'static str>>,
    /// The row says non-keyword words inside brackets are plain words (`@paren=w`).
    pub paren_word: bool,
}

/// The deepest nesting parsed structurally; deeper text is taken as a flat run of tokens.
pub(crate) const MAX_DEPTH: u32 = 200;

impl<'a> P<'a> {
    fn new(src: &'a str, d: Dialect) -> P<'a> {
        let (lexed, open) = lex(src, d);
        let code: Vec<u32> = (0..lexed.len() as u32)
            .filter(|&i| !lexed[i as usize].kind.is_trivia())
            .collect();
        let kinds = code
            .iter()
            .map(|&i| default_kind(lexed[i as usize].kind))
            .collect();
        let mut p = P {
            src,
            d,
            lex: lexed,
            code,
            pos: 0,
            kinds,
            nodes: Vec::new(),
            metas: Vec::new(),
            group: 0,
            errors: Vec::new(),
            depth: 0,
            type_first: false,
            row: None,
            paren_word: false,
        };
        for i in open {
            let l = p.lex[i as usize];
            let what = if l.kind == T::BlockComment {
                "unterminated comment"
            } else {
                "unterminated string or name"
            };
            p.errors.push(ParseError::syntax(
                l.start,
                l.end - l.start,
                what.to_string(),
            ));
        }
        p
    }

    // ---- tokens

    pub fn eof(&self) -> bool {
        self.pos >= self.code.len()
    }

    pub fn tok(&self, n: usize) -> Option<&Lex> {
        self.code.get(self.pos + n).map(|&i| &self.lex[i as usize])
    }

    /// The kind of the lexer token `n` ahead.
    pub fn t(&self, n: usize) -> Option<T> {
        self.tok(n).map(|l| l.kind)
    }

    pub fn is_t(&self, n: usize, kind: T) -> bool {
        self.t(n) == Some(kind)
    }

    pub fn text(&self, n: usize) -> &'a str {
        match self.tok(n) {
            Some(l) => &self.src[l.start as usize..l.end as usize],
            None => "",
        }
    }

    /// Is the token `n` ahead the word `w` (case-insensitive)?
    pub fn is(&self, n: usize, w: &str) -> bool {
        self.is_t(n, T::Word) && self.text(n).eq_ignore_ascii_case(w)
    }

    pub fn at(&self, w: &str) -> bool {
        self.is(0, w)
    }

    /// Is the next token one of `ws`?
    pub fn at_any_at(&self, n: usize, words: &[&str]) -> bool {
        words.iter().any(|w| self.is(n, w))
    }

    pub fn at_any(&self, ws: &[&str]) -> bool {
        self.is_t(0, T::Word) && ws.iter().any(|w| self.text(0).eq_ignore_ascii_case(w))
    }

    /// Are the next tokens the words `ws`?
    pub fn at_seq(&self, ws: &[&str]) -> bool {
        ws.iter().enumerate().all(|(i, w)| self.is(i, w))
    }

    pub fn is_op(&self, n: usize, op: &str) -> bool {
        self.is_t(n, T::Op) && self.text(n) == op
    }

    pub fn at_op(&self, op: &str) -> bool {
        self.is_op(0, op)
    }

    /// Gives the next token `kind` and moves on.
    pub fn bump_as(&mut self, kind: SyntaxKind) {
        if self.pos < self.code.len() {
            self.kinds[self.pos] = kind;
            self.pos += 1;
        }
    }

    /// Moves on with the token's default kind.
    pub fn bump(&mut self) {
        if self.pos < self.code.len() {
            self.pos += 1;
        }
    }

    /// Consumes the word `w` as a keyword.
    pub fn eat(&mut self, w: &str) -> bool {
        let hit = self.at(w);
        if hit {
            self.bump_as(k::KEYWORD);
        }
        hit
    }

    /// Consumes the words `ws` as keywords if they all follow.
    pub fn eat_seq(&mut self, ws: &[&str]) -> bool {
        let hit = self.at_seq(ws);
        if hit {
            for _ in ws {
                self.bump_as(k::KEYWORD);
            }
        }
        hit
    }

    /// Consumes one of `ws` as a keyword.
    pub fn eat_any(&mut self, ws: &[&str]) -> bool {
        let hit = self.at_any(ws);
        if hit {
            self.bump_as(k::KEYWORD);
        }
        hit
    }

    pub fn eat_t(&mut self, kind: T) -> bool {
        let hit = self.is_t(0, kind);
        if hit {
            self.bump();
        }
        hit
    }

    /// Is the next token a word that no clause can claim (an identifier, not a reserved word)?
    pub fn at_name(&self) -> bool {
        match self.t(0) {
            Some(T::Quoted) => true,
            Some(T::Word) => !self.reserved(0),
            Some(T::Var) => self.text(0).starts_with('#'),
            _ => false,
        }
    }

    pub fn reserved(&self, n: usize) -> bool {
        let mut buf = [0u8; 40];
        let w = dialect::upper(self.text(n), &mut buf);
        !w.is_empty() && self.d.is_reserved(w)
    }

    /// Is the next token attached to this one, with no space between?
    pub fn touches_next(&self) -> bool {
        let at = |n: usize| self.code.get(self.pos + n).map(|&c| self.lex[c as usize]);
        matches!((at(0), at(1)), (Some(a), Some(b)) if a.end == b.start)
    }

    /// Is the previous token a word that is no clause word (a name, an alias)?
    pub fn prev_is_plain_word(&self) -> bool {
        let Some(l) = self
            .pos
            .checked_sub(1)
            .and_then(|i| self.code.get(i))
            .map(|&c| self.lex[c as usize])
        else {
            return false;
        };
        let mut buf = [0u8; 40];
        l.kind == T::Word
            && !self.d.is_reserved(dialect::upper(
                &self.src[l.start as usize..l.end as usize],
                &mut buf,
            ))
    }

    pub fn keyword_like(&self, n: usize) -> bool {
        let mut buf = [0u8; 40];
        let w = dialect::upper(self.text(n), &mut buf);
        !w.is_empty() && self.d.is_keyword(w)
    }

    /// Consumes a name (an unreserved word or a quoted one) as an identifier.
    pub fn eat_name(&mut self) -> bool {
        let hit = self.at_name();
        if hit {
            if self.is_t(0, T::Var) {
                self.bump(); // `#temp`
                return true;
            }
            let kind = if self.is_t(0, T::Quoted) {
                k::QUOTED_IDENTIFIER
            } else {
                k::IDENTIFIER
            };
            self.bump_as(kind);
        }
        hit
    }

    // ---- nodes and indentation metas

    /// Does a line break lie between the previous code token and the cursor?
    pub fn newline_before(&self) -> bool {
        let (Some(&a), Some(&b)) = (
            self.pos.checked_sub(1).and_then(|i| self.code.get(i)),
            self.code.get(self.pos),
        ) else {
            return false;
        };
        self.src[self.lex[a as usize].end as usize..self.lex[b as usize].start as usize]
            .contains('\n')
    }

    pub fn mark(&self) -> usize {
        self.pos
    }

    /// Closes a node of `kind` over the tokens since `mark`.
    pub fn close(&mut self, kind: SyntaxKind, mark: usize) {
        if self.pos > mark {
            self.nodes.push((kind, mark as u32, self.pos as u32));
        }
    }

    pub fn cp(&self) -> Cp {
        Cp {
            pos: self.pos,
            nodes: self.nodes.len(),
            metas: self.metas.len(),
            errors: self.errors.len(),
        }
    }

    pub fn rewind(&mut self, cp: Cp) {
        self.pos = cp.pos;
        self.nodes.truncate(cp.nodes);
        self.metas.truncate(cp.metas);
        self.errors.truncate(cp.errors);
    }

    fn meta(&mut self, kind: SyntaxKind) {
        let pos = self.pos as u32;
        if self.metas.last().is_some_and(|m| m.pos != pos) {
            self.group += 1;
        }
        self.metas.push(Meta {
            pos,
            kind,
            group: self.group,
        });
    }

    pub fn indent(&mut self) {
        self.meta(k::INDENT);
    }

    pub fn implicit(&mut self) {
        self.meta(k::IMPLICIT_INDENT);
    }

    pub fn dedent(&mut self) {
        self.meta(k::DEDENT);
    }

    /// Indents what `f` reads (an implicit indent for a condition); nothing is placed when it reads nothing.
    pub fn indented(&mut self, implicit: bool, f: impl FnOnce(&mut Self)) {
        let (pos, metas) = (self.pos, self.metas.len());
        if implicit {
            self.implicit();
        } else {
            self.indent();
        }
        f(self);
        if self.pos == pos {
            self.metas.truncate(metas);
        } else {
            self.dedent();
        }
    }

    /// Ends the group of metas at this position: the next ones are placed on their own.
    pub fn split(&mut self) {
        self.group += 1;
    }

    pub fn error(&mut self, at: usize, message: &str) {
        let (start, end) = match self.code.get(at).or(self.code.last()) {
            Some(&i) => (self.lex[i as usize].start, self.lex[i as usize].end),
            None => (0, 0),
        };
        self.errors
            .push(ParseError::syntax(start, end - start, message.to_string()));
    }

    // ---- the flat result

    fn finish(self) -> Flat {
        let P {
            lex,
            code,
            kinds,
            nodes,
            metas,
            errors,
            ..
        } = self;
        let mut leaves: Vec<Leaf> = Vec::with_capacity(lex.len() + metas.len());
        let mut leaf_of: Vec<u32> = Vec::with_capacity(code.len() + 1);
        let mut mi = 0;
        let mut next_lex = 0;
        #[allow(clippy::needless_range_loop)]
        for c in 0..=code.len() {
            let upto = code.get(c).map_or(lex.len(), |&i| i as usize);
            let at_prev = if next_lex > 0 {
                lex[next_lex - 1].end
            } else {
                0
            };
            let at_tok = code
                .get(c)
                .map_or(lex.last().map_or(0, |l| l.end), |&i| lex[i as usize].start);
            // The metas at this gap, in groups: an all-indent group goes in front of the trivia, the
            // others behind.
            let end = metas[mi..]
                .iter()
                .position(|m| m.pos as usize != c)
                .map_or(metas.len(), |p| mi + p);
            let gap = &metas[mi..end];
            mi = end;
            let mut behind: Vec<&Meta> = Vec::new();
            let mut g = 0;
            while g < gap.len() {
                let e = gap[g..]
                    .iter()
                    .position(|m| m.group != gap[g].group)
                    .map_or(gap.len(), |p| g + p);
                let group = &gap[g..e];
                if group.iter().all(|m| m.kind != k::DEDENT) {
                    for m in group {
                        leaves.push(Leaf {
                            kind: m.kind,
                            start: at_prev,
                            end: at_prev,
                        });
                    }
                } else {
                    behind.extend(group);
                }
                g = e;
            }
            for (j, l) in lex[next_lex..upto].iter().enumerate() {
                let _ = j;
                leaves.push(Leaf {
                    kind: trivia_kind(l.kind),
                    start: l.start,
                    end: l.end,
                });
            }
            for m in behind {
                leaves.push(Leaf {
                    kind: m.kind,
                    start: at_tok,
                    end: at_tok,
                });
            }
            leaf_of.push(leaves.len() as u32);
            if let Some(&i) = code.get(c) {
                let l = &lex[i as usize];
                leaves.push(Leaf {
                    kind: kinds[c],
                    start: l.start,
                    end: l.end,
                });
                next_lex = i as usize + 1;
            }
        }
        let nodes = nodes
            .into_iter()
            .map(|(kind, first, end)| Span {
                kind,
                first: leaf_of[first as usize] as usize,
                end: leaf_of[end as usize - 1] as usize + 1,
            })
            .collect();
        Flat {
            leaves,
            nodes,
            errors,
        }
    }
}

fn trivia_kind(kind: T) -> SyntaxKind {
    match kind {
        T::Ws => k::WHITESPACE,
        T::Nl => k::NEWLINE,
        T::LineComment => k::LINE_COMMENT,
        _ => k::BLOCK_COMMENT,
    }
}

/// The kind of a token no rule named.
fn default_kind(kind: T) -> SyntaxKind {
    match kind {
        T::Word => k::WORD,
        T::Number => k::NUMBER,
        T::Str => k::STRING,
        T::Quoted => k::QUOTED_IDENTIFIER,
        T::LParen => k::L_PAREN,
        T::RParen => k::R_PAREN,
        T::LSquare => k::L_SQUARE,
        T::RSquare => k::R_SQUARE,
        T::Comma => k::COMMA,
        T::Semi => k::SEMICOLON,
        T::Dot => k::DOT,
        T::Cast => k::CAST_OP,
        T::Colon => k::COLON,
        T::Unknown => k::UNKNOWN,
        T::Data => k::DATA,
        _ => k::OPERATOR,
    }
}
