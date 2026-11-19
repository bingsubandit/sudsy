//! Declarations: a type and its declarators, functions with their parameters and bodies, classes,
//! enums, namespaces and templates. A line is a declaration when a type (a name path or builtin
//! words, with `*` `&` and `const`) is followed by a name and then by what can follow a declarator.
//!
//! concept: hand-written-parser-kit

use super::expr::is_builtin;
use super::{BODY, CLASS, Cx, ENUM, NONE, SCOPE};
use crate::generated::kind as k;
use core_syntax::SyntaxKind;

type Read = (usize, Option<SyntaxKind>);

/// Words in front of a declaration that are no part of its type.
const SPECIFIERS: &[&str] = &[
    "static",
    "inline",
    "constexpr",
    "consteval",
    "constinit",
    "extern",
    "virtual",
    "explicit",
    "friend",
    "typedef",
    "thread_local",
    "mutable",
    "register",
    "export",
    "_Noreturn",
    "restrict",
    "__inline",
    "__inline__",
    "__extension__",
    "_Thread_local",
];

/// Words that start an attribute whose arguments are a group.
const ATTRIBUTES: &[&str] = &[
    "alignas",
    "_Alignas",
    "__attribute__",
    "__declspec",
    "__asm__",
    "asm",
];

impl Cx<'_> {
    /// A declaration, or none when the tokens are an expression.
    pub(super) fn declaration(&mut self, a: usize, b: usize, scope: u8) -> Option<Read> {
        let s = self.specifiers(a, b);
        if self.word(s, b, "template") {
            return Some(self.template(s, b, scope));
        }
        if (self.word(s, b, "namespace")
            || (self.word(s, b, "extern") && self.is(s + 1, b, k::STRING)))
            && let Some(r) = self.namespace(s, b)
        {
            return Some(r);
        }
        if (self.word(s, b, "class") || self.word(s, b, "struct") || self.word(s, b, "union"))
            && let Some(r) = self.class_specifier(s, b)
        {
            return Some(r);
        }
        if self.word(s, b, "enum")
            && let Some(r) = self.enum_specifier(s, b)
        {
            return Some(r);
        }
        if self.word(s, b, "using") {
            let end = self.simple_end(a, b);
            return Some((
                end,
                (!self.has_block(a, end)).then_some(k::SIMPLE_DECLARATION),
            ));
        }
        if let Some((q, id)) = self.declarators_of(s, b) {
            // `type name (` in a declaration scope is a function.
            if scope != BODY && self.open(id, b, k::LPAR) && self.params_like(id) {
                return Some(self.function(id, b));
            }
            return Some(self.variables(q, b));
        }
        if scope != BODY {
            return self.constructor_like(s, b, scope);
        }
        None
    }

    /// The words in front of a type, and attributes: where the type starts.
    pub(super) fn specifiers(&mut self, a: usize, b: usize) -> usize {
        let mut p = a;
        loop {
            if p >= b {
                return p;
            }
            let t = self.c[p];
            match t.kind {
                k::KEYWORD | k::IDENT => {
                    let w = self.text(p);
                    if t.kind == k::KEYWORD
                        && SPECIFIERS.contains(&w)
                        && !(w == "extern" && self.is(p + 1, b, k::STRING))
                    {
                        p += 1;
                    } else if (ATTRIBUTES.contains(&w)
                        || (t.kind == k::IDENT && w.starts_with("__")))
                        && self.open(p + 1, b, k::LPAR)
                    {
                        p = self.skip(p + 1);
                    } else if t.kind == k::IDENT
                        && macro_word(w)
                        && self.name_like(p + 1, b)
                        && !self.word(p + 1, b, "operator")
                    {
                        p += 1;
                    } else {
                        return p;
                    }
                }
                k::LSQB if self.groups.is_open(p) => p = self.skip(p),
                _ => return p,
            }
        }
    }

    /// If the tokens from `a` are a type and the start of a declarator, where the declarator
    /// starts (after the type and the `*` `&` that belong to it).
    pub(super) fn declarators_of(&mut self, a: usize, b: usize) -> Option<(usize, usize)> {
        self.quiet = true;
        let probe = self.probe_declarators(a, b);
        self.quiet = false;
        // Once it is one, the names of its type are read again, this time with their nodes.
        probe.and_then(|_| self.probe_declarators(a, b))
    }

    fn probe_declarators(&mut self, a: usize, b: usize) -> Option<(usize, usize)> {
        let q = self.type_end(a, b)?;
        let id = self.declarator_id(q, b)?;
        let follows = id >= b
            || matches!(
                self.c[id].kind,
                k::SEMI | k::EQUAL | k::COMMA | k::LPAR | k::LBRACE | k::LSQB | k::COLON | k::RPAR
            )
            || self.word(id, b, "const")
            || self.word(id, b, "override")
            || self.word(id, b, "final");
        follows.then_some((q, id))
    }

    /// The end of a type: `const`, builtin words or a name path with template arguments, then
    /// `*` `&` `&&` and `const`.
    fn type_end(&mut self, a: usize, b: usize) -> Option<usize> {
        let mut q = a;
        while self.word(q, b, "const")
            || self.word(q, b, "volatile")
            || self.word(q, b, "typename")
            || self.word(q, b, "struct")
            || self.word(q, b, "class")
            || self.word(q, b, "union")
            || self.word(q, b, "enum")
        {
            q += 1;
        }
        let mut builtin = false;
        while self.is(q, b, k::KEYWORD)
            && (is_builtin(self.text(q)) || matches!(self.text(q), "const" | "volatile"))
        {
            builtin |= is_builtin(self.text(q));
            q += 1;
        }
        if !builtin {
            if self.word(q, b, "decltype") && self.open(q + 1, b, k::LPAR) {
                q = self.skip(q + 1);
            } else {
                if !(self.is(q, b, k::IDENT) || self.is(q, b, k::COLON_COLON)) {
                    return None;
                }
                let e = self.name_end(q, b, true);
                if e == q {
                    return None;
                }
                q = e;
            }
        }
        loop {
            if self.is(q, b, k::STAR)
                || self.is(q, b, k::AMP)
                || self.is(q, b, k::AMP_AMP)
                || self.word(q, b, "const")
                || self.word(q, b, "volatile")
                || self.word(q, b, "restrict")
                || self.word(q, b, "__restrict")
            {
                q += 1;
            } else {
                return Some(q);
            }
        }
    }

    /// The end of the name a declarator declares, or of `(*name)`, `[a, b]`.
    fn declarator_id(&mut self, q: usize, b: usize) -> Option<usize> {
        if self.open(q, b, k::LPAR) {
            let (lo, hi) = self.inner(q);
            if lo < hi && matches!(self.c[lo].kind, k::STAR | k::AMP | k::CARET) {
                let e = self.skip(q);
                if self.open(e, b, k::LPAR) || self.open(e, b, k::LSQB) {
                    return Some(self.skip(e));
                }
                return None;
            }
            return None;
        }
        if self.open(q, b, k::LSQB) {
            return Some(self.skip(q));
        }
        if !(self.is(q, b, k::IDENT)
            || self.is(q, b, k::COLON_COLON)
            || self.is(q, b, k::TILDE)
            || self.word(q, b, "operator"))
        {
            return None;
        }
        let e = self.name_end(q, b, false);
        (e > q).then_some(e)
    }

    /// The declarators of a declaration of variables, `x = 1, *p, a[3], b{2}`.
    pub(super) fn declarators(&mut self, mut q: usize, b: usize) -> usize {
        loop {
            while self.is(q, b, k::STAR)
                || self.is(q, b, k::AMP)
                || self.is(q, b, k::AMP_AMP)
                || self.word(q, b, "const")
            {
                q += 1;
            }
            let e = self.expression(q, b);
            q = if e > q {
                e
            } else {
                self.skip(q.min(b.saturating_sub(1))).max(q + 1)
            };
            // `Ret (*f)(int){nullptr}`: braces after the declarator.
            if self.open(q, b, k::LBRACE) && self.group_kind(q) != Some(k::BLOCK) {
                self.args(q, k::ARGUMENTS);
                q = self.skip(q);
            }
            if self.is(q, b, k::COLON) {
                q += 1;
                let e = self.expression(q, b);
                q = e.max(q);
            }
            if self.is(q, b, k::COMMA) {
                q += 1;
                continue;
            }
            return q;
        }
    }

    fn variables(&mut self, q: usize, b: usize) -> Read {
        let end = self.simple_end(q, b);
        let semi = end > q && self.c[end - 1].kind == k::SEMI;
        let stop = if semi { end - 1 } else { end };
        self.declarators(q, stop);
        (
            end,
            (!self.has_block(q, stop)).then_some(k::SIMPLE_DECLARATION),
        )
    }

    /// A function whose parameter list is the group at `lp`: parameters, trailing specifiers,
    /// constructor initializers and the body or `;`.
    fn function(&mut self, lp: usize, b: usize) -> Read {
        if self.closed(lp) && !self.crosses_in(lp) {
            self.retag(lp, k::PARAMETER_LIST);
        } else {
            return (self.simple_end(lp, b), None);
        }
        self.parameters(lp);
        let mut q = self.skip(lp);
        loop {
            if self.word(q, b, "noexcept") || self.word(q, b, "throw") {
                q += 1;
                if self.open(q, b, k::LPAR) {
                    q = self.skip(q);
                }
            } else if self.word(q, b, "const")
                || self.word(q, b, "volatile")
                || self.word(q, b, "override")
                || self.word(q, b, "final")
                || self.is(q, b, k::AMP)
                || self.is(q, b, k::AMP_AMP)
            {
                q += 1;
            } else if self.is(q, b, k::LSQB) && self.groups.is_open(q) {
                q = self.skip(q);
            } else if self.is(q, b, k::ARROW) {
                q += 1;
                while q < b
                    && !matches!(self.c[q].kind, k::SEMI | k::EQUAL)
                    && !(self.c[q].kind == k::LBRACE && self.groups.is_open(q))
                {
                    q = self.skip(q);
                }
            } else if self.word(q, b, "requires") {
                q += 1;
                while q < b
                    && !matches!(self.c[q].kind, k::SEMI)
                    && !(self.c[q].kind == k::LBRACE && self.groups.is_open(q))
                {
                    q = self.skip(q);
                }
            } else {
                break;
            }
        }
        if self.is(q, b, k::EQUAL) {
            q = (q + 2).min(b);
        }
        if self.is(q, b, k::COLON) {
            q = self.initializers(q, b);
        }
        if self.is(q, b, k::LBRACE) && self.groups.is_open(q) {
            if self.closed(q) && self.group_kind(q) != Some(k::BLOCK) {
                self.retag(q, k::BLOCK);
            }
            self.own_block(q, BODY);
            q = self.skip(q);
        } else if self.is(q, b, k::SEMI) {
            q += 1;
        }
        (q, Some(k::FUNCTION_DECLARATION))
    }

    /// `: a(1), b{2}` before the body of a constructor.
    fn initializers(&mut self, colon: usize, b: usize) -> usize {
        let mut q = colon + 1;
        loop {
            let e = self.name_end(q, b, false);
            if e == q {
                break;
            }
            q = e;
            if (self.open(q, b, k::LPAR) || self.open(q, b, k::LBRACE)) && !self.block_ahead(q) {
                self.args(q, k::ARGUMENTS);
                q = self.skip(q);
            }
            if self.is(q, b, k::ELLIPSIS) {
                q += 1;
            }
            if self.is(q, b, k::COMMA) {
                q += 1;
            } else {
                break;
            }
        }
        self.emit(colon, q - 1, k::CONSTRUCTOR_INITIALIZERS);
        q
    }

    /// The `{` at `q` is the body (a block that follows a name, not an initializer of a member).
    fn block_ahead(&self, q: usize) -> bool {
        self.c[q].kind == k::LBRACE
            && q > 0
            && matches!(self.c[q - 1].kind, k::RPAR | k::RBRACE)
            && false
    }

    /// `Name(...)`, `~Name()`, `Name::Name(...)` without a return type: a constructor or a
    /// destructor when a body, initializers or `= default` follow (or in a class, a `;`).
    fn constructor_like(&mut self, s: usize, b: usize, scope: u8) -> Option<Read> {
        let e = self.declarator_id(s, b)?;
        if !self.open(e, b, k::LPAR) {
            return None;
        }
        let mut q = self.skip(e);
        while self.word(q, b, "noexcept") || self.word(q, b, "const") || self.word(q, b, "override")
        {
            q += 1;
            if self.open(q, b, k::LPAR) {
                q = self.skip(q);
            }
        }
        if !self.params_like(e) {
            return None;
        }
        let body = self.is(q, b, k::COLON) || self.is(q, b, k::LBRACE) || self.is(q, b, k::EQUAL);
        let tilde = self.is(s, b, k::TILDE);
        if body || tilde || (scope == CLASS && self.is(q, b, k::SEMI)) {
            return Some(self.function(e, b));
        }
        None
    }

    /// The group at `lp` can be a parameter list: empty, or every parameter starts with a name,
    /// a keyword, `::` or `...` and holds no literal before its default value.
    fn params_like(&self, lp: usize) -> bool {
        let (lo, hi) = self.inner(lp);
        let mut p = lo;
        while p < hi {
            if !matches!(
                self.c[p].kind,
                k::IDENT | k::KEYWORD | k::COLON_COLON | k::ELLIPSIS
            ) {
                return false;
            }
            while p < hi && self.c[p].kind != k::COMMA && self.c[p].kind != k::EQUAL {
                if matches!(self.c[p].kind, k::NUMBER | k::STRING | k::CHAR) {
                    return false;
                }
                if self.c[p].kind == k::LESS
                    && let Some(e) = self.template_end(p, hi, false)
                {
                    p = e;
                } else {
                    p = self.skip(p);
                }
            }
            while p < hi && self.c[p].kind != k::COMMA {
                p = self.skip(p);
            }
            p += 1;
        }
        true
    }

    /// `(params)`: one `Parameter` node per parameter, default values read.
    pub(super) fn parameters(&mut self, lp: usize) {
        let (lo, hi) = self.inner(lp);
        let mut p = lo;
        while p < hi {
            let start = p;
            let mut q = p;
            let mut eq = NONE as usize;
            while q < hi && self.c[q].kind != k::COMMA {
                if self.c[q].kind == k::EQUAL && eq == NONE as usize {
                    eq = q;
                }
                if self.c[q].kind == k::LESS
                    && q > 0
                    && self.c[q - 1].kind == k::IDENT
                    && let Some(e) = self.template_end(q, hi, false)
                {
                    q = e;
                    continue;
                }
                q = self.skip(q);
            }
            if q > start {
                if eq != NONE as usize && eq > start {
                    self.expression(eq - 1, q);
                }
                self.emit(start, q - 1, k::PARAMETER);
            }
            p = q + 1;
        }
    }

    /// `template <...> declaration`.
    fn template(&mut self, t: usize, b: usize, scope: u8) -> Read {
        let mut q = t + 1;
        if self.is(q, b, k::LESS)
            && let Some(e) = self.template_end(q, b, true)
        {
            self.emit(q, e - 1, k::TEMPLATE_PARAMETERS);
            q = e;
        }
        let (end, kind) = self.statement(q, b, scope);
        if let Some(kind) = kind {
            self.emit(q, end.max(q + 1) - 1, kind);
        }
        (end.max(q), Some(k::TEMPLATE_DECLARATION))
    }

    /// `namespace a::b { }`, `extern "C" { }`; a namespace alias is a declaration.
    fn namespace(&mut self, s: usize, b: usize) -> Option<Read> {
        let mut q = s + 1;
        while q < b
            && !matches!(self.c[q].kind, k::SEMI | k::EQUAL)
            && !(self.c[q].kind == k::LBRACE)
        {
            q = self.skip(q);
        }
        if !(self.is(q, b, k::LBRACE) && self.groups.is_open(q)) {
            return None;
        }
        if self.closed(q) {
            self.retag(q, k::BLOCK);
        }
        self.own_block(q, SCOPE);
        let end = self.skip(q);
        Some((end, Some(k::NAMESPACE_DECLARATION)))
    }

    /// `class A : public B { }` with the declarators that may follow; none when no body follows.
    fn class_specifier(&mut self, s: usize, b: usize) -> Option<Read> {
        let (body, base) = self.type_body(s, b)?;
        if let Some(c) = base {
            self.emit(c, body - 1, k::BASE_CLAUSE);
        }
        self.own_block(body, CLASS);
        if self.closed(body) {
            self.retag(body, k::BLOCK);
        }
        let mut end = self.skip(body);
        if end < b && self.c[end].kind != k::SEMI {
            self.declarators(end, b);
        }
        end = self.simple_end(end.min(b), b).max(end);
        Some((end, Some(k::CLASS_SPECIFIER)))
    }

    fn enum_specifier(&mut self, s: usize, b: usize) -> Option<Read> {
        let (body, base) = self.type_body(s, b)?;
        if let Some(c) = base {
            self.emit(c, body - 1, k::BASE_CLAUSE);
        }
        self.own_block(body, ENUM);
        if self.closed(body) {
            self.retag(body, k::BLOCK);
        }
        let mut end = self.skip(body);
        if end < b && self.c[end].kind != k::SEMI {
            self.declarators(end, b);
        }
        end = self.simple_end(end.min(b), b).max(end);
        Some((end, Some(k::ENUM_SPECIFIER)))
    }

    /// The `{` of a class or enum that starts at `s`, and its base clause (`:`) if any.
    fn type_body(&mut self, s: usize, b: usize) -> Option<(usize, Option<usize>)> {
        let mut q = s + 1;
        let mut base = None;
        while q < b {
            match self.c[q].kind {
                k::LBRACE => {
                    return (self.groups.is_open(q)).then_some((q, base));
                }
                k::SEMI | k::EQUAL => return None,
                k::COLON if base.is_none() => base = Some(q),
                k::LPAR => {
                    let attr = q > 0 && matches!(self.c[q - 1].kind, k::KEYWORD | k::IDENT) && {
                        let w = self.text(q - 1);
                        ATTRIBUTES.contains(&w) || w.starts_with("__")
                    };
                    if !attr && base.is_none() {
                        return None;
                    }
                }
                _ => {}
            }
            q = self.skip(q);
        }
        None
    }
}

/// `EXPORT_API`, `LLVM_ABI`: a macro that stands where a specifier would.
fn macro_word(s: &str) -> bool {
    s.len() >= 2
        && s.bytes().any(|c| c.is_ascii_uppercase())
        && s.bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}
