//! What the arguments of a format string are, read from the grammar's tags and fields: a tuple
//! (`#tuple`), a dict (`#dict`), a list, set or comprehension (`#display`), a value that is
//! never a tuple (`#literal`, an operation on such values), or something unknown.

use core_syntax::roles::{code_text, un};
use core_syntax::{FormatVia, Grammar, SyntaxElement, SyntaxNode};

/// The reading of one grammar.
pub(super) struct Reader<'g> {
    pub g: &'g Grammar,
}

/// The right side of an operator format.
pub(super) enum Rhs {
    Tuple(Vec<SyntaxNode>),
    /// The string keys, when every key is one.
    Dict(Option<Vec<String>>),
    Sequence,
    Single,
    Unknown,
}

impl Reader<'_> {
    pub fn tag(&self, n: &SyntaxNode, tag: &str) -> bool {
        self.g.has_tag(n.kind(), tag)
    }

    /// The node in field `label`, without parentheses around it.
    pub fn field(&self, n: &SyntaxNode, label: &str) -> Option<SyntaxNode> {
        self.g.slot(n, label)?.into_node().map(|n| un(&n))
    }

    /// The text of the operator token of `n`.
    pub fn operator(&self, n: &SyntaxNode) -> Option<String> {
        match self.g.slot(n, "operator")? {
            SyntaxElement::Token(t) => Some(t.text().to_owned()),
            SyntaxElement::Node(n) => Some(code_text(&n)),
        }
    }

    /// The kind of the operator of `n` (`logical`, `arithmetic`, ...).
    fn operator_kind(&self, n: &SyntaxNode) -> Option<&str> {
        self.operator(n).and_then(|o| self.g.operator_kind(&o))
    }

    /// The value of a string literal: its pieces joined, escapes resolved.
    pub fn string_value(&self, n: &SyntaxNode) -> String {
        n.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| self.g.has_tag(t.kind(), "string"))
            .map(|t| {
                let value = self.g.string_value(t.kind(), t.text());
                value.map(|v| v.into_owned()).unwrap_or_default()
            })
            .collect()
    }

    pub fn is_string(&self, n: &SyntaxNode) -> bool {
        self.tag(n, "string-literal")
    }

    /// An expression that is one value, never a tuple.
    fn single(&self, e: &SyntaxNode) -> bool {
        let e = un(e);
        let parts: Vec<SyntaxNode> = e.children().collect();
        let all = || parts.iter().all(|p| self.single(p));
        if self.tag(&e, "literal") || self.tag(&e, "display") {
            true
        } else if self.tag(&e, "unary") {
            self.operator_kind(&e) == Some("logical")
                || parts.first().is_some_and(|p| self.single(p))
        } else if self.tag(&e, "binary") {
            // A format with an operator (`"%s" % x`) is a string.
            let formats = self.operator(&e).is_some_and(|o| {
                (self.g.formats.iter()).any(|f| f.via == FormatVia::Operator && f.name == o)
            });
            if formats && self.field(&e, "left").is_some_and(|l| self.is_string(&l)) {
                return true;
            }
            match self.operator_kind(&e) {
                Some("comparison" | "equality" | "membership") => false,
                _ => all(),
            }
        } else if self.tag(&e, "conditional") {
            [self.field(&e, "then"), self.field(&e, "else")]
                .iter()
                .all(|p| p.as_ref().is_some_and(|p| self.single(p)))
        } else if self.tag(&e, "named-expr") {
            self.field(&e, "right").is_some_and(|p| self.single(&p))
        } else {
            false
        }
    }

    /// The string keys of a dict display, when each entry has one (a `**mapping` or a
    /// comprehension has none).
    fn dict_keys(&self, d: &SyntaxNode) -> Option<Vec<String>> {
        let g = self.g;
        let (mut keys, mut last, mut spread) = (Vec::new(), None, false);
        for e in d.children_with_tokens() {
            match e {
                SyntaxElement::Node(n) if g.has_tag(n.kind(), "comprehension") => return None,
                SyntaxElement::Node(n) => last = Some(n),
                SyntaxElement::Token(t) if g.has_tag(t.kind(), "rest") => spread = true,
                SyntaxElement::Token(t) if g.has_tag(t.kind(), "key-separator") => {
                    keys.push(last.take()?);
                }
                SyntaxElement::Token(_) => {}
            }
        }
        let keys: Option<Vec<String>> = keys
            .iter()
            .map(|k| {
                let k = un(k);
                self.is_string(&k).then(|| self.string_value(&k))
            })
            .collect();
        keys.filter(|_| !spread)
    }

    pub fn classify(&self, e: &SyntaxNode) -> Rhs {
        let mut e = un(e);
        while self.tag(&e, "named-expr") {
            match self.field(&e, "right") {
                Some(v) => e = v,
                None => break,
            }
        }
        if self.tag(&e, "tuple") {
            Rhs::Tuple(e.children().collect())
        } else if self.tag(&e, "dict") {
            Rhs::Dict(self.dict_keys(&e))
        } else if self.tag(&e, "display") {
            Rhs::Sequence
        } else if self.single(&e) {
            Rhs::Single
        } else {
            Rhs::Unknown
        }
    }
}
