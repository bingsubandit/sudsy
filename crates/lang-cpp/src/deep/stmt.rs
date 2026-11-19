//! Statements: what one line is. A control statement is one line with its body (a block, or the
//! statement that follows, up to its `;`), so its parts are read from the tokens of the line.
//!
//! concept: hand-written-parser-kit

use super::{BODY, CLASS, Cx, ENUM, INIT, SCOPE, UNSET};
use crate::generated::kind as k;
use core_syntax::SyntaxKind;

type Read = (usize, Option<SyntaxKind>);

impl Cx<'_> {
    /// Reads the line `i`, which is in a block of `scope`; true for a `case` label.
    pub(super) fn line(&mut self, i: usize, scope: u8) -> bool {
        let (start, end) = (self.nodes[i].start, self.nodes[i].end);
        let a = self.c.partition_point(|t| t.start < start);
        let b = self.c.partition_point(|t| t.end <= end);
        if a >= b {
            return false;
        }
        if scope == INIT {
            // The values of an initializer list: its elements, no line.
            self.nodes[i].start = super::GONE;
            self.list(a, b);
            return false;
        }
        if scope == ENUM {
            self.list(a, b);
            return false;
        }
        // A block on its own is a node of its own, the line around it is not.
        if self.c[a].kind == k::LBRACE && self.group_kind(a) == Some(k::BLOCK) && self.skip(a) >= b
        {
            self.own_block(a, scope);
            self.nodes[i].start = super::GONE;
            return false;
        }
        let (end, kind) = self.statement(a, b, scope);
        let Some(kind) = kind else {
            return false;
        };
        if self.crosses(a, end.min(b) - 1) {
            return false;
        }
        if end >= b {
            self.nodes[i].kind = kind;
        } else {
            self.emit(a, end - 1, kind);
        }
        kind == k::CASE_CLAUSE
    }

    /// One statement from `a`: where it ends, and its kind (none: the tokens stay a line).
    pub(super) fn statement(&mut self, a: usize, b: usize, scope: u8) -> Read {
        let t = self.c[a];
        if t.kind == k::SEMI {
            return (a + 1, Some(k::EMPTY_STATEMENT));
        }
        if t.kind == k::KEYWORD {
            let r = match self.text(a) {
                "if" => self.if_statement(a, b, scope),
                "while" | "for" => self.control(a, b, scope, k::WHILE_STATEMENT),
                "switch" => self.switch(a, b),
                "do" => self.do_statement(a, b, scope),
                "try" => self.try_statement(a, b),
                "return" | "co_return" => self.jump(a, b, k::RETURN_STATEMENT, true),
                "break" => self.jump(a, b, k::BREAK_STATEMENT, false),
                "continue" => self.jump(a, b, k::CONTINUE_STATEMENT, false),
                "goto" => self.jump(a, b, k::GOTO_STATEMENT, false),
                "case" | "default" => self.label(a, b),
                "public" | "private" | "protected" if self.is(a + 1, b, k::COLON) => {
                    (a + 2, Some(k::ACCESS_SPECIFIER))
                }
                _ => (a, None),
            };
            if r.1.is_some() {
                return r;
            }
        }
        if t.kind == k::IDENT && crate::structure::macro_name(self.text(a)) {
            // `FOO(x);` and `FOO`: a macro call, which may expand to anything
            let mut e = a + 1;
            if self.open(e, b, k::LPAR) {
                e = self.skip(e);
            }
            if self.is(e, b, k::SEMI) {
                e += 1;
            }
            if e >= b || self.c[e - 1].kind == k::SEMI {
                return (e, Some(k::MACRO_STATEMENT));
            }
        }
        if t.kind == k::IDENT && self.is(a + 1, b, k::COLON) && scope != CLASS {
            return (a + 2, Some(k::LABEL_STATEMENT));
        }
        self.declaration_or_expression(a, b, scope)
    }

    /// The end of the statement that starts at `a` and ends at its `;` (or at `b`).
    pub(super) fn simple_end(&self, a: usize, b: usize) -> usize {
        let mut p = a;
        while p < b {
            if self.c[p].kind == k::SEMI {
                return p + 1;
            }
            p = self.skip(p);
        }
        b
    }

    /// A statement used as the body of another: a block (as it is) or a statement of its own.
    fn body(&mut self, p: usize, b: usize, scope: u8) -> usize {
        if p >= b {
            return p;
        }
        if self.c[p].kind == k::LBRACE && self.groups.is_open(p) {
            self.own_block(p, BODY);
            if self.closed(p) && self.group_kind(p) != Some(k::BLOCK) {
                self.retag(p, k::BLOCK);
            }
            return self.skip(p);
        }
        let (e, kind) = self.statement(p, b, scope);
        let e = e.max(p + 1);
        self.emit(p, e - 1, kind.unwrap_or(k::LINE));
        e
    }

    /// The `( )` group of a control statement: its node is a `Condition`, its parts are read.
    fn condition(&mut self, p: usize) {
        self.retag(p, k::CONDITION);
        let (lo, hi) = self.inner(p);
        let mut q = lo;
        while q < hi {
            let mut e = q;
            while e < hi && self.c[e].kind != k::SEMI {
                e = self.skip(e);
            }
            self.condition_part(q, e);
            q = e + 1;
        }
    }

    /// An expression, or the declaration of a variable (`T x = v`, `T x : range`).
    fn condition_part(&mut self, a: usize, b: usize) {
        if a >= b {
            return;
        }
        match self.declarators_of(a, b) {
            Some((q, _)) => {
                self.declarators(q, b);
            }
            None => self.list(a, b),
        }
    }

    fn if_statement(&mut self, a: usize, b: usize, scope: u8) -> Read {
        let mut p = a + 1;
        if self.word(p, b, "constexpr") || self.word(p, b, "consteval") {
            p += 1;
        }
        if self.is(p, b, k::BANG) && self.word(p + 1, b, "consteval") {
            p += 2;
        }
        if self.open(p, b, k::LPAR) {
            self.condition(p);
            p = self.skip(p);
        }
        if p >= b {
            return (a, None);
        }
        let mut end = self.body(p, b, scope);
        if self.word(end, b, "else") {
            let q = end + 1;
            if self.word(q, b, "if") {
                let (e, _) = self.if_statement(q, b, scope);
                self.emit(q, e - 1, k::IF_STATEMENT);
                end = e;
            } else {
                end = self.body(q, b, scope);
            }
        }
        (end, Some(k::IF_STATEMENT))
    }

    /// `while (c) body`, `for (head) body`.
    fn control(&mut self, a: usize, b: usize, scope: u8, kind: SyntaxKind) -> Read {
        let mut p = a + 1;
        if self.open(p, b, k::LPAR) {
            self.condition(p);
            p = self.skip(p);
        }
        let kind = if self.word(a, b, "for") {
            k::FOR_STATEMENT
        } else {
            kind
        };
        (self.body(p, b, scope), Some(kind))
    }

    fn switch(&mut self, a: usize, b: usize) -> Read {
        let mut p = a + 1;
        if self.open(p, b, k::LPAR) {
            self.condition(p);
            p = self.skip(p);
        }
        (self.body(p, b, BODY), Some(k::SWITCH_STATEMENT))
    }

    fn do_statement(&mut self, a: usize, b: usize, scope: u8) -> Read {
        let mut p = self.body(a + 1, b, scope);
        if self.word(p, b, "while") && self.open(p + 1, b, k::LPAR) {
            self.condition(p + 1);
            p = self.skip(p + 1);
            if self.is(p, b, k::SEMI) {
                p += 1;
            }
            return (p, Some(k::DO_STATEMENT));
        }
        (p, None)
    }

    fn try_statement(&mut self, a: usize, b: usize) -> Read {
        let mut p = a + 1;
        if self.is(p, b, k::LBRACE) {
            p = self.body(p, b, BODY);
        }
        while self.word(p, b, "catch") {
            let start = p;
            p += 1;
            if self.open(p, b, k::LPAR) {
                self.retag(p, k::CONDITION);
                p = self.skip(p);
            }
            p = self.body(p, b, BODY);
            self.emit(start, p - 1, k::CATCH_CLAUSE);
        }
        (p, Some(k::TRY_STATEMENT))
    }

    /// `return v;`, `break;`, `goto l;`.
    fn jump(&mut self, a: usize, b: usize, kind: SyntaxKind, value: bool) -> Read {
        let end = self.simple_end(a, b);
        if self.has_block(a, end) {
            return (end, None);
        }
        if value {
            let semi = end > a && self.c[end - 1].kind == k::SEMI;
            self.list(a + 1, if semi { end - 1 } else { end });
        }
        (end, Some(kind))
    }

    /// A `{ }` block among the tokens `a..b` (a lambda-like macro): the statement is no plain one.
    pub(super) fn has_block(&self, a: usize, b: usize) -> bool {
        let mut p = a;
        while p < b {
            if self.c[p].kind == k::LBRACE && self.group_kind(p) == Some(k::BLOCK) {
                return true;
            }
            p = self.skip(p);
        }
        false
    }

    /// `case v:` and `default:`; the label ends its line.
    fn label(&mut self, a: usize, b: usize) -> Read {
        let mut colon = a + 1;
        while colon < b && self.c[colon].kind != k::COLON {
            colon = self.skip(colon);
        }
        if colon >= b {
            return (a, None);
        }
        self.list(a + 1, colon);
        // `case 1 ... 3:` is a range.
        if (a + 1..colon).any(|p| self.c[p].kind == k::ELLIPSIS) {
            self.emit(a + 1, colon - 1, k::COMPOUND_EXPRESSION);
        }
        (colon + 1, Some(k::CASE_CLAUSE))
    }

    /// An expression statement, or a declaration.
    pub(super) fn declaration_or_expression(&mut self, a: usize, b: usize, scope: u8) -> Read {
        if let Some(r) = self.declaration(a, b, scope) {
            return r;
        }
        let end = self.simple_end(a, b);
        let semi = end > a && self.c[end - 1].kind == k::SEMI;
        let stop = if semi { end - 1 } else { end };
        let mut p = a;
        loop {
            let e = self.expression(p, stop);
            p = e;
            if p < stop && self.c[p].kind == k::COMMA {
                p += 1;
                continue;
            }
            break;
        }
        (
            end,
            if p >= stop && stop > a {
                Some(k::EXPRESSION_STATEMENT)
            } else {
                None
            },
        )
    }
}

#[allow(dead_code)]
const _: (u8, u8, u8) = (CLASS, SCOPE, UNSET);
