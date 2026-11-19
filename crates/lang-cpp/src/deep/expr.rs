//! Expressions: precedence climbing for the binary operators (the `precedence` lines of the
//! grammar), the postfix forms (call, index, member access), prefix operators, casts, lambdas and
//! names with template arguments. A name or a literal alone stays a bare token.
//!
//! concept: hand-written-parser-kit

use super::{BODY, Cx, INIT, UNSET};
use crate::generated::kind as k;
use core_syntax::SyntaxKind;
use core_syntax::grammars;
use core_syntax::parse::{Operator, Pratt, climb};

/// Words that name a type or a value: operands, and callees of a functional cast.
const OPERAND_WORDS: &[&str] = &[
    "true",
    "false",
    "nullptr",
    "this",
    "int",
    "char",
    "short",
    "long",
    "float",
    "double",
    "bool",
    "void",
    "signed",
    "unsigned",
    "wchar_t",
    "char8_t",
    "char16_t",
    "char32_t",
    "auto",
    "_Bool",
    "static_cast",
    "dynamic_cast",
    "const_cast",
    "reinterpret_cast",
    "typeid",
    "alignof",
    "decltype",
    "noexcept",
    "static_assert",
    "_Static_assert",
    "_Alignof",
    "_Generic",
    "typename",
    "asm",
    "typeof",
    "typeof_unqual",
    "__builtin_offsetof",
    "NULL",
];

/// Words in front of an operand: `sizeof x`, `co_await x`, `delete p`, `throw e`.
const PREFIX_WORDS: &[&str] = &[
    "sizeof",
    "co_await",
    "co_yield",
    "delete",
    "throw",
    "co_return",
];

/// Words that may follow a type in a cast and are no operand (`(T) const`).
const NOT_OPERAND: &[&str] = &[
    "const", "volatile", "noexcept", "override", "final", "requires",
];

impl Cx<'_> {
    /// Parses a comma-free expression from `a`; returns the token after it (`a` when none).
    pub(super) fn expression(&mut self, a: usize, b: usize) -> usize {
        let (pos, lim) = (self.pos, self.lim);
        (self.pos, self.lim) = (a, b);
        self.assign();
        let end = self.pos;
        (self.pos, self.lim) = (pos, lim);
        end
    }

    /// Reads the expressions between `a` and `b`, separated by commas (arguments, initializers,
    /// the parts of a `for` head); what is no expression is skipped.
    pub(super) fn list(&mut self, a: usize, b: usize) {
        let mut p = a;
        while p < b {
            let e = self.expression(p, b);
            p = if e > p { e } else { self.skip(p) };
            if p < b && self.c[p].kind == k::COMMA {
                p += 1;
            } else if p < b && e > a {
                p = self.skip(p);
            }
        }
    }

    /// The group at `i` holds a list of expressions: its node becomes `kind`.
    pub(super) fn args(&mut self, i: usize, kind: SyntaxKind) {
        if self.c[i].kind == k::LBRACE {
            self.mark_init(i);
            if self.group_kind(i) != Some(k::BLOCK) {
                self.retag(i, kind);
            }
        } else {
            self.retag(i, kind);
            let (lo, hi) = self.inner(i);
            self.list(lo, hi);
        }
    }

    /// The lines of a `{ }` group of values are read as a list when the driver reaches them.
    pub(super) fn mark_init(&mut self, i: usize) {
        let Some(g) = self.groups.node(i) else {
            return;
        };
        // A group that holds statements (`MACRO({ a; b; })`) is a block.
        let (lo, hi) = self.inner(i);
        let mut p = lo;
        while p < hi && self.c[p].kind != k::SEMI {
            p = self.skip(p);
        }
        let statement = lo < hi
            && self.c[lo].kind == k::KEYWORD
            && matches!(
                self.text(lo),
                "switch" | "if" | "for" | "while" | "do" | "return" | "try"
            );
        if (p < hi || statement) && self.closed(i) {
            self.nodes[g].kind = k::BLOCK;
            self.scope[g] = BODY;
        } else {
            self.scope[g] = INIT;
        }
    }

    /// `a, b, c` as the middle of a conditional.
    fn comma_expression(&mut self) {
        let at = self.pos;
        self.assign();
        let mut commas = false;
        while self.pos > at && self.is(self.pos, self.lim, k::COMMA) {
            self.pos += 1;
            self.assign();
            commas = true;
        }
        if commas {
            self.emit(at, self.pos - 1, k::COMPOUND_EXPRESSION);
        }
    }

    fn assign(&mut self) {
        let at = self.pos;
        self.ternary();
        if self.pos > at && self.pos < self.lim && self.is_assign(self.pos) {
            self.pos += 1;
            let rhs = self.pos;
            self.assign();
            // `.field = v` in an initializer names a member: no assignment
            let designator = at > 0 && self.c[at - 1].kind == k::DOT;
            if self.pos > rhs && !designator {
                self.emit(at, self.pos - 1, k::ASSIGNMENT);
            }
        }
    }

    fn is_assign(&self, i: usize) -> bool {
        matches!(
            self.c[i].kind,
            k::EQUAL
                | k::PLUS_EQ
                | k::MINUS_EQ
                | k::STAR_EQ
                | k::SLASH_EQ
                | k::PERCENT_EQ
                | k::CARET_EQ
                | k::AMP_EQ
                | k::PIPE_EQ
                | k::LESS_LESS_EQ
                | k::GREATER_GREATER_EQ
        )
    }

    fn ternary(&mut self) {
        let at = self.pos;
        climb(self, 1);
        if self.pos > at && self.pos < self.lim && self.c[self.pos].kind == k::QUESTION {
            self.pos += 1;
            let then = self.pos;
            self.comma_expression();
            // What the expression reader cannot place goes with the middle part.
            let mut p = self.pos;
            while p < self.lim && self.c[p].kind != k::COLON && self.c[p].kind != k::SEMI {
                p = self.skip(p);
            }
            if p > self.pos && p < self.lim && self.c[p].kind == k::COLON && p > then {
                self.emit(then, p - 1, k::COMPOUND_EXPRESSION);
                self.pos = p;
            }
            if self.pos < self.lim && self.c[self.pos].kind == k::COLON {
                self.pos += 1;
                self.assign();
            }
            self.emit(at, self.pos - 1, k::TERNARY);
        }
    }

    fn unary(&mut self) {
        let at = self.pos;
        if at >= self.lim {
            return;
        }
        let t = self.c[at];
        let prefix = match t.kind {
            k::PLUS
            | k::MINUS
            | k::BANG
            | k::TILDE
            | k::STAR
            | k::AMP
            | k::PLUS_PLUS
            | k::MINUS_MINUS
            | k::AMP_AMP => true,
            k::KEYWORD => PREFIX_WORDS.contains(&self.text(at)),
            _ => false,
        };
        if prefix {
            self.pos += 1;
            // `delete[] p`.
            if t.kind == k::KEYWORD
                && self.text(at) == "delete"
                && self.open(self.pos, self.lim, k::LSQB)
            {
                self.pos = self.skip(self.pos);
            }
            self.unary();
            if self.pos > at + 1 {
                self.emit(at, self.pos - 1, k::UNARY);
            }
            return;
        }
        // `::new`, `::delete`.
        if t.kind == k::COLON_COLON
            && matches!(
                self.c
                    .get(at + 1)
                    .map(|n| (n.kind, &self.src[n.start as usize..n.end as usize])),
                Some((k::KEYWORD, "new" | "delete"))
            )
            && at + 1 < self.lim
        {
            self.pos += 1;
            self.unary();
            return;
        }
        if t.kind == k::KEYWORD && self.text(at) == "new" {
            self.new_expression();
            return;
        }
        if t.kind == k::LPAR && self.groups.is_open(at) && self.cast(at) {
            self.pos = self.skip(at);
            self.unary();
            self.emit(at, self.pos - 1, k::CAST);
            return;
        }
        self.postfix();
    }

    /// Whether the `( )` group at `i` is a cast: it holds a type and an operand follows.
    fn cast(&self, i: usize) -> bool {
        let (lo, hi) = self.inner(i);
        let after = self.skip(i);
        // `(T){ ... }` is a compound literal.
        let literal = self.is(after, self.lim, k::LBRACE);
        let prefix = matches!(
            self.c.get(after).map(|t| t.kind),
            Some(k::PLUS | k::MINUS | k::PLUS_PLUS | k::MINUS_MINUS | k::STAR | k::AMP)
        );
        if lo >= hi || after >= self.lim || !(literal || prefix || self.operand_start(after)) {
            return false;
        }
        let mut builtin = false;
        let mut decorated = false;
        let mut name = false;
        let mut p = lo;
        while p < hi {
            match self.c[p].kind {
                k::STAR | k::AMP | k::AMP_AMP => decorated = true,
                k::COLON_COLON => {}
                k::IDENT => name = true,
                k::KEYWORD => {
                    let w = self.text(p);
                    if w == "decltype" && self.open(p + 1, hi, k::LPAR) {
                        builtin = true;
                        p = self.skip(p + 1);
                        continue;
                    }
                    if matches!(w, "struct" | "class" | "enum" | "union") || is_builtin(w) {
                        builtin = true;
                    } else if !matches!(
                        w,
                        "const" | "volatile" | "typename" | "unsigned" | "signed"
                    ) {
                        return false;
                    }
                }
                k::LESS => match self.template_end(p, hi, false) {
                    Some(e) => {
                        decorated = true;
                        p = e;
                        continue;
                    }
                    None => return false,
                },
                _ => return false,
            }
            p += 1;
        }
        // `(T) x` with a lone name is a cast only before something that cannot continue a binary
        // expression: a name, a literal, `(` is a call (`(f)(x)`).
        // A prefix operator after a lone name is a binary operator: `(a) - b`.
        let chained =
            self.is(after, self.lim, k::LPAR) && self.groups.is_open(after) && self.cast(after);
        builtin || decorated || chained || (name && !prefix && self.c[after].kind != k::LPAR)
    }

    /// Whether the token at `i` starts an operand (not an operator that could be binary).
    fn operand_start(&self, i: usize) -> bool {
        let t = self.c[i];
        match t.kind {
            k::IDENT | k::NUMBER | k::STRING | k::CHAR | k::LPAR | k::BANG | k::TILDE => true,
            k::KEYWORD => {
                let w = self.text(i);
                !NOT_OPERAND.contains(&w)
                    && (OPERAND_WORDS.contains(&w)
                        || PREFIX_WORDS.contains(&w)
                        || w == "new"
                        || w == "sizeof")
            }
            _ => false,
        }
    }

    fn postfix(&mut self) {
        let at = self.pos;
        self.primary();
        if self.pos == at {
            return;
        }
        loop {
            let p = self.pos;
            if p >= self.lim {
                return;
            }
            match self.c[p].kind {
                k::LPAR if self.groups.is_open(p) => {
                    self.args(p, k::ARGUMENTS);
                    self.pos = self.skip(p);
                    self.emit(at, self.pos - 1, k::CALL);
                }
                k::LBRACE
                    if self.groups.is_open(p)
                        && self.group_kind(p) != Some(k::BLOCK)
                        && self.callee_before(p) =>
                {
                    self.args(p, k::ARGUMENTS);
                    if self.group_kind(p) == Some(k::BLOCK) {
                        return;
                    }
                    self.pos = self.skip(p);
                    self.emit(at, self.pos - 1, k::CALL);
                }
                k::LSQB if self.groups.is_open(p) => {
                    self.args(p, k::ARGUMENTS);
                    self.pos = self.skip(p);
                    self.emit(at, self.pos - 1, k::INDEX);
                }
                k::DOT | k::ARROW => {
                    let mut q = p + 1;
                    if self.word(q, self.lim, "template") {
                        q += 1;
                    }
                    let e = self.name_end(q, self.lim, false);
                    if e == q {
                        return;
                    }
                    self.pos = e;
                    self.emit(at, e - 1, k::MEMBER_ACCESS);
                }
                k::PLUS_PLUS | k::MINUS_MINUS => {
                    self.pos = p + 1;
                    self.emit(at, p, k::UNARY);
                }
                _ => return,
            }
        }
    }

    /// The operand just before the `{` at `p` is a name or a type that braces construct.
    fn callee_before(&self, p: usize) -> bool {
        p > 0
            && matches!(
                self.c[p - 1].kind,
                k::IDENT | k::GREATER | k::GREATER_GREATER
            )
    }

    fn primary(&mut self) {
        let at = self.pos;
        let t = self.c[at];
        match t.kind {
            k::NUMBER | k::CHAR => self.pos += 1,
            k::STRING => self.strings(),
            k::LPAR if self.groups.is_open(at) => {
                let (lo, hi) = self.inner(at);
                // `({ statements })` and what a directive splits are no plain parentheses.
                if self.closed(at) && !self.has_block(lo, hi) && !self.crosses_in(at) {
                    self.retag(at, k::PARENTHESIZED);
                }
                self.list(lo, hi);
                self.pos = self.skip(at);
            }
            k::LBRACE if self.groups.is_open(at) => {
                self.mark_init(at);
                self.retag(at, k::INIT_LIST);
                self.pos = self.skip(at);
            }
            k::LSQB if self.groups.is_open(at) => self.lambda(),
            // A macro in front of a string (`PRI "x"`) goes with it.
            k::IDENT if self.is(at + 1, self.lim, k::STRING) => self.strings(),
            k::IDENT | k::COLON_COLON => self.name(),
            k::KEYWORD => {
                let w = self.text(at);
                if OPERAND_WORDS.contains(&w) || is_builtin(w) {
                    self.name();
                }
            }
            _ => {}
        }
    }

    /// String literals one after the other, with the macros between them (`"a" PRIu64 "b"`).
    fn strings(&mut self) {
        let at = self.pos;
        let lim = self.lim;
        while self.pos < lim {
            let t = self.c[self.pos].kind;
            if t == k::STRING || (t == k::IDENT && self.is(self.pos + 1, lim, k::STRING)) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos > at + 1 {
            self.emit(at, self.pos - 1, k::STRING_CONCAT);
        }
    }

    /// A name, qualified or with template arguments, as one `QualifiedName` when it is more than
    /// a token.
    fn name(&mut self) {
        let at = self.pos;
        let e = self.name_end(at, self.lim, false);
        self.pos = e.max(at + 1);
    }

    /// The end of the name that starts at `p`: `::`-separated parts, each with its template
    /// arguments (`decl`: a name may be followed by another name, so `a < b > c` is a template).
    pub(super) fn name_end(&mut self, p: usize, b: usize, decl: bool) -> usize {
        let e = self.name_parts(p, b, decl);
        if e > p + 1 {
            self.emit(p, e - 1, k::QUALIFIED_NAME);
        }
        e
    }

    fn name_parts(&mut self, p: usize, b: usize, decl: bool) -> usize {
        let mut q = p;
        if self.is(q, b, k::COLON_COLON) {
            q += 1;
        }
        loop {
            if self.is(q, b, k::TILDE) {
                q += 1;
            }
            if self.word(q, b, "operator") {
                return self.operator_name_end(q, b);
            }
            if !self.name_like(q, b) {
                return p;
            }
            q += 1;
            if self.is(q, b, k::LESS)
                && matches!(self.c[q - 1].kind, k::IDENT | k::KEYWORD)
                && let Some(e) = self.template_end(q, b, false)
                && self.after_template(e, b, decl)
            {
                // Template arguments are tokens of the name; they get a node of their own.
                self.emit(q, e - 1, k::TEMPLATE_ARGUMENTS);
                q = e;
            }
            if self.is(q, b, k::COLON_COLON) && self.name_like(q + 1, b) {
                q += 1;
                if self.word(q, b, "template") && self.name_like(q + 1, b) {
                    q += 1;
                }
                continue;
            }
            return q;
        }
    }

    /// `operator` and the tokens that name the operator, up to its parameter list.
    pub(super) fn operator_name_end(&self, p: usize, b: usize) -> usize {
        let mut q = p + 1;
        if (self.is(q, b, k::LPAR) || self.is(q, b, k::LSQB))
            && self.groups.is_open(q)
            && self.inner(q).0 >= self.inner(q).1
        {
            return self.skip(q);
        }
        while q < b && !(self.c[q].kind == k::LPAR && self.groups.is_open(q)) {
            q += 1;
        }
        q
    }

    /// Whether what follows the template arguments that end at `e` shows they are some.
    fn after_template(&self, e: usize, b: usize, decl: bool) -> bool {
        if e >= b {
            return true;
        }
        match self.c[e].kind {
            k::LPAR
            | k::LBRACE
            | k::COLON_COLON
            | k::RPAR
            | k::COMMA
            | k::SEMI
            | k::RSQB
            | k::QUESTION
            | k::PIPE_PIPE
            | k::AMP_AMP
            | k::EQ_EQ
            | k::BANG_EQ
            | k::GREATER
            | k::GREATER_GREATER
            | k::DOT
            | k::ARROW
            | k::AMP
            | k::STAR
            | k::ELLIPSIS
            | k::EQUAL => true,
            k::IDENT | k::KEYWORD => decl,
            _ => false,
        }
    }

    /// The end of the template arguments that open at `p` (`<`), if what lies between can be
    /// some: names, literals, `::`, `,`, `*`, `&`, groups; `lax` (template parameters) also
    /// `=` and `.`.
    pub(super) fn template_end(&self, p: usize, b: usize, lax: bool) -> Option<usize> {
        let mut depth = 0i32;
        let mut q = p;
        let limit = (p + 192).min(b);
        while q < limit {
            match self.c[q].kind {
                k::LESS => depth += 1,
                k::GREATER => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(q + 1);
                    }
                }
                k::GREATER_GREATER => {
                    if depth < 2 {
                        return None;
                    }
                    depth -= 2;
                    if depth == 0 {
                        return Some(q + 1);
                    }
                }
                k::IDENT
                | k::NUMBER
                | k::STRING
                | k::CHAR
                | k::COLON_COLON
                | k::COMMA
                | k::STAR
                | k::AMP
                | k::ELLIPSIS
                | k::MINUS
                | k::PLUS
                | k::TILDE
                | k::BANG => {}
                k::AMP_AMP => {
                    if !matches!(
                        self.c.get(q + 1).map(|t| t.kind),
                        Some(k::GREATER | k::GREATER_GREATER | k::COMMA | k::ELLIPSIS)
                    ) {
                        return None;
                    }
                }
                k::EQUAL | k::DOT | k::ARROW if lax => {}
                k::KEYWORD => {
                    if matches!(
                        self.text(q),
                        "if" | "else"
                            | "return"
                            | "for"
                            | "while"
                            | "do"
                            | "switch"
                            | "case"
                            | "break"
                            | "continue"
                            | "goto"
                            | "new"
                            | "delete"
                            | "throw"
                    ) {
                        return None;
                    }
                }
                k::LPAR | k::LSQB | k::LBRACE => {
                    if !self.groups.is_open(q) {
                        return None;
                    }
                    q = self.skip(q);
                    continue;
                }
                _ => return None,
            }
            q += 1;
        }
        None
    }

    /// `[captures]<T>(parameters) specifiers -> type { body }`.
    fn lambda(&mut self) {
        let at = self.pos;
        let after = self.skip(at);
        let b = self.lim;
        let mut q = after;
        if self.is(q, b, k::LESS)
            && let Some(e) = self.template_end(q, b, true)
        {
            q = e;
        }
        let params = self.open(q, b, k::LPAR);
        if !params && !self.is(q, b, k::LBRACE) {
            // Not a lambda: `[x]` alone (an attribute, a designator).
            self.pos = after;
            return;
        }
        self.retag(at, k::CAPTURES);
        let (lo, hi) = self.inner(at);
        self.captures(lo, hi);
        if params {
            if self.closed(q) && !self.crosses_in(q) {
                self.retag(q, k::PARAMETER_LIST);
            }
            self.parameters(q);
            q = self.skip(q);
        }
        // Specifiers and a trailing return type, up to the body.
        while q < b && !(self.c[q].kind == k::LBRACE && self.groups.is_open(q)) {
            if self.c[q].kind == k::SEMI || self.c[q].kind == k::COMMA || self.c[q].kind == k::RPAR
            {
                break;
            }
            q = self.skip(q);
        }
        if self.is(q, b, k::LBRACE) && self.groups.is_open(q) {
            if self.closed(q) && self.group_kind(q) != Some(k::BLOCK) {
                self.retag(q, k::BLOCK);
            }
            self.own_block(q, BODY);
            q = self.skip(q);
        }
        self.pos = q;
        self.emit(at, q - 1, k::LAMBDA);
    }

    /// The captures between `lo` and `hi`: an init-capture `name = value` is no assignment, only its
    /// value is read.
    fn captures(&mut self, lo: usize, hi: usize) {
        let mut p = lo;
        while p < hi {
            let mut e = p;
            while e < hi && self.c[e].kind != k::COMMA {
                e = self.skip(e);
            }
            let named = self.is(p, e, k::IDENT) && self.is(p + 1, e, k::EQUAL);
            self.list(if named { p + 2 } else { p }, e);
            p = e + 1;
        }
    }

    /// `new T(args)`, `new T[n]`, `new (place) T{..}`.
    fn new_expression(&mut self) {
        let at = self.pos;
        let b = self.lim;
        let mut q = at + 1;
        if self.open(q, b, k::LPAR) {
            self.args(q, k::ARGUMENTS);
            q = self.skip(q);
        }
        loop {
            let e = self.name_end(q, b, true);
            if e > q {
                q = e;
                continue;
            }
            match self.c.get(q).map(|t| t.kind) {
                Some(k::STAR | k::AMP) if q < b => q += 1,
                Some(k::KEYWORD)
                    if q < b && (is_builtin(self.text(q)) || self.text(q) == "const") =>
                {
                    q += 1
                }
                _ => break,
            }
        }
        while self.open(q, b, k::LSQB) {
            self.args(q, k::ARGUMENTS);
            q = self.skip(q);
        }
        if self.open(q, b, k::LPAR) || self.open(q, b, k::LBRACE) {
            self.args(q, k::ARGUMENTS);
            q = self.skip(q);
        }
        self.pos = q.max(at + 1);
        self.emit(at, self.pos - 1, k::NEW);
    }
}

pub(super) fn is_builtin(w: &str) -> bool {
    matches!(
        w,
        "int"
            | "char"
            | "short"
            | "long"
            | "float"
            | "double"
            | "bool"
            | "void"
            | "signed"
            | "unsigned"
            | "wchar_t"
            | "char8_t"
            | "char16_t"
            | "char32_t"
            | "auto"
            | "_Bool"
            | "_Complex"
            | "__int128"
    )
}

impl Pratt for Cx<'_> {
    type Mark = usize;

    fn mark(&mut self) -> usize {
        self.pos
    }

    fn operand(&mut self) {
        self.unary();
    }

    fn operator(&self) -> Option<Operator> {
        let i = self.pos;
        if i >= self.lim {
            return None;
        }
        let t = self.c[i];
        if t.kind == k::KEYWORD || t.kind == k::IDENT {
            return None;
        }
        let prec = grammars::CPP.get().precedence(self.text(i))?;
        Some(Operator {
            prec,
            tokens: 1,
            kind: t.kind,
        })
    }

    fn infix(&mut self, at: usize, op: Operator) {
        self.pos += op.tokens;
        let rhs = self.pos;
        climb(self, op.rhs_min());
        if self.pos > rhs {
            self.emit(at, self.pos - 1, k::BINARY);
        }
    }
}

#[allow(dead_code)]
const _: u8 = UNSET;
