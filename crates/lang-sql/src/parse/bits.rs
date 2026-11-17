//! Small shared pieces: references, aliases, brackets, comma lists.

use super::lexer::T;
use super::{MAX_DEPTH, P};
use crate::generated::kind as k;
use core_syntax::SyntaxKind;

impl P<'_> {
    /// `a`, `a.b.c`, `t.*`: an object reference (a column, a table, a schema). The node closes
    /// over the whole chain.
    pub fn obj_ref(&mut self) -> bool {
        let m = self.mark();
        if !self.eat_name() {
            return false;
        }
        while self.is_t(0, T::Dot) && self.starts_name(1) {
            self.bump();
            if self.is_op(0, "*") {
                self.bump();
                break;
            }
            self.eat_name();
        }
        self.close(k::OBJECT_REFERENCE, m);
        true
    }

    /// Does a name (or `*`) start `n` tokens ahead?
    pub fn starts_name(&self, n: usize) -> bool {
        match self.t(n) {
            Some(T::Quoted) => true,
            Some(T::Word) => !self.reserved(n) || self.is_t(n + 1, T::Dot) || self.is_t(n + 1, T::LParen),
            Some(T::Op) => self.is_op(n, "*"),
            _ => false,
        }
    }

    /// An `AS`-alias or a bare alias of an element of a list or a table.
    pub fn alias(&mut self) -> bool {
        let m = self.mark();
        let has_as = self.at("as");
        if !has_as && !(self.at_name() && !self.at_any(&["window", "limit"])) {
            return false;
        }
        if has_as && !(self.starts_alias_name(1)) {
            return false;
        }
        self.indent();
        if has_as {
            self.eat("as");
        }
        self.alias_name();
        // `AS t (a, b)`: the column aliases.
        self.dedent();
        self.close(k::ALIAS_EXPRESSION, m);
        true
    }

    fn starts_alias_name(&self, n: usize) -> bool {
        matches!(self.t(n), Some(T::Quoted | T::Str)) || (self.is_t(n, T::Word) && !self.reserved(n))
    }

    fn alias_name(&mut self) {
        if self.is_t(0, T::Str) {
            self.bump();
        } else {
            self.eat_name();
        }
    }

    /// `( ... )` with the indent of a bracket; `f` parses the contents, anything it leaves is an
    /// error up to the closing bracket. Returns whether the bracket was closed.
    pub fn bracketed(&mut self, f: impl FnOnce(&mut Self)) -> bool {
        self.bracketed_as(k::BRACKETED, f)
    }

    pub fn bracketed_as(&mut self, kind: SyntaxKind, f: impl FnOnce(&mut Self)) -> bool {
        let m = self.mark();
        if !self.is_t(0, T::LParen) {
            return false;
        }
        self.bump();
        self.indent();
        self.split();
        self.depth += 1;
        if self.depth < MAX_DEPTH {
            f(self);
        }
        self.depth -= 1;
        self.skip_to_close();
        self.dedent();
        let closed = self.eat_t(T::RParen);
        if !closed {
            self.error(self.pos, "missing )");
        }
        self.close(kind, m);
        closed
    }

    /// Takes what is left of a bracket, up to its closing bracket, as an error.
    fn skip_to_close(&mut self) {
        if self.is_t(0, T::RParen) || self.eof() {
            return;
        }
        let m = self.mark();
        let at = self.pos;
        let mut depth = 0usize;
        while let Some(t) = self.t(0) {
            match t {
                T::LParen | T::LSquare => depth += 1,
                T::RParen | T::RSquare if depth > 0 => depth -= 1,
                T::RParen => break,
                T::Semi => break,
                _ => {}
            }
            self.bump();
        }
        self.error(at, "syntax error");
        self.close(SyntaxKind::ERROR, m);
    }

    /// `item (, item)*`: the commas keep their kind; stops when `item` parses nothing.
    pub fn list(&mut self, mut item: impl FnMut(&mut Self) -> bool) -> bool {
        if !item(self) {
            return false;
        }
        while self.is_t(0, T::Comma) {
            let cp = self.cp();
            self.bump();
            if !item(self) {
                // A trailing comma before `)` stays in the list.
                if !self.is_t(0, T::RParen) {
                    self.rewind(cp);
                }
                break;
            }
        }
        true
    }

    /// Takes tokens up to the next `;` (outside brackets) as an error: the end of an
    /// unparsed statement.
    pub fn skip_statement(&mut self) {
        let m = self.mark();
        let at = self.pos;
        let mut depth = 0usize;
        while let Some(t) = self.t(0) {
            match t {
                T::LParen | T::LSquare => depth += 1,
                T::RParen | T::RSquare => depth = depth.saturating_sub(1),
                T::Semi if depth == 0 => break,
                _ => {}
            }
            self.bump();
        }
        if self.pos > m {
            self.error(at, "syntax error");
            self.close(SyntaxKind::ERROR, m);
        }
    }
}
