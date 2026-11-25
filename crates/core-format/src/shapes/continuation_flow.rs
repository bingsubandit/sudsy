//! Operator chains of a command language (`a && b || c`, `a | b`) whose source line breaks are
//! kept: a break next to an operator stays there, the lines after it stand one level in, and under
//! `binary_operator_line: start` the break goes before the operator. In the code of an embedded
//! instruction ([`EmbedMode::Continued`]) the first statement's chain names the indentation of
//! its first line, so the lines of the instruction stand level with it.
//!
//! concept: line-continuation-layout
//! what: Operator chains whose line breaks are kept, before or after the operators.
//! tools: shfmt, dockerfmt

use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::{EmbedMode, FormatCtx};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// A significant child; `cont` when a line continuation precedes it.
pub struct Item {
    pub el: SyntaxElement,
    pub cont: bool,
}

/// The facts of a language's chains: node kinds, operator tokens and the trivia kind that holds
/// a continuation (`\` and a line break).
pub struct OperatorFlow<'a> {
    /// The kinds a statement's chain is made of; `leaf` is the one that holds no flow below it.
    pub flows: &'a [SyntaxKind],
    pub leaf: SyntaxKind,
    /// Kinds that chain into one flow when the first child is of the kind of the other
    /// (`a && b >> f`: a list in a redirected).
    pub pair: &'a [SyntaxKind],
    /// Token kinds that hold an operator, and the operators that chain.
    pub operator_kinds: &'a [SyntaxKind],
    pub operators: &'a [&'a str],
    /// A flow of this kind after one of `lead_operators` starts level with that operator.
    pub pipe: SyntaxKind,
    pub lead_operators: &'a [&'a str],
    /// Children that are never glued to the one before them (redirects).
    pub unglued: &'a [SyntaxKind],
    /// The kinds of blocks: a chain that starts with one has its lines one level in already.
    pub blocks: &'a [SyntaxKind],
    /// The whitespace kind that holds a continuation.
    pub whitespace: SyntaxKind,
}

impl OperatorFlow<'_> {
    fn is_op(&self, el: &SyntaxElement) -> bool {
        self.op_text(el)
            .is_some_and(|t| self.operators.contains(&t))
    }

    fn op_text<'e>(&self, el: &'e SyntaxElement) -> Option<&'e str> {
        el.as_token()
            .filter(|t| self.operator_kinds.contains(&t.kind()))
            .map(|t| t.text())
    }

    /// Does a continuation (whitespace trivia with the escape) stand right before `t`?
    pub fn continued_before(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
        let mut cur = t.prev_token();
        while let Some(p) = cur.filter(|p| cx.grammar.is_trivia(p.kind())) {
            if p.kind() == self.whitespace && p.text().contains('\\') {
                return true;
            }
            cur = p.prev_token();
        }
        false
    }

    /// Is there a line break (not a continuation) between `t` and the next significant token?
    pub fn breaks(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
        let mut cur = t.next_token();
        while let Some(n) = cur.filter(|n| cx.grammar.is_trivia(n.kind())) {
            if n.kind() != self.whitespace {
                return true;
            }
            cur = n.next_token();
        }
        false
    }

    /// The significant children of `node`.
    pub fn items(&self, cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<Item> {
        self.items_with(cx, node, |el, cont| Item { el, cont })
    }

    /// [`OperatorFlow::items`] built as the caller's own type (no second vector).
    pub fn items_with<T>(
        &self,
        cx: &FormatCtx<'_>,
        node: &SyntaxNode,
        make: impl Fn(SyntaxElement, bool) -> T,
    ) -> Vec<T> {
        node.children_with_tokens()
            .filter(|el| !matches!(el, SyntaxElement::Token(t) if cx.grammar.is_trivia(t.kind())))
            .map(|el| {
                let first = match &el {
                    SyntaxElement::Token(t) => Some(t.clone()),
                    SyntaxElement::Node(n) => cx.first_token(n),
                };
                let cont = first.is_some_and(|t| self.continued_before(cx, &t));
                make(el, cont)
            })
            .collect()
    }

    fn broken_op(&self, cx: &FormatCtx<'_>, es: &[Item], i: usize) -> bool {
        let e = &es[i];
        e.cont
            || es.get(i + 1).is_some_and(|n| n.cont)
            || e.el.as_token().is_some_and(|t| self.breaks(cx, t))
    }

    /// Does a break stand next to an operator of the children of a flow?
    fn has_operator_break(&self, cx: &FormatCtx<'_>, es: &[Item]) -> bool {
        (0..es.len()).any(|i| self.is_op(&es[i].el) && self.broken_op(cx, es, i))
    }

    /// Is `node` a chain of flows that does not start with a block that `is_block` accepts
    /// (whose closing line stands one level in already)?
    pub fn chain_led(&self, node: &SyntaxNode, is_block: impl Fn(&SyntaxNode) -> bool) -> bool {
        let mut lead = node.clone();
        while self.flows.contains(&lead.kind()) && lead.kind() != self.leaf {
            match lead.first_child() {
                Some(c) => lead = c,
                None => break,
            }
        }
        self.flows.contains(&node.kind())
            && !(self.blocks.contains(&lead.kind()) && is_block(&lead))
    }

    /// Is `node`, a flow of embedded code, the one of the first statement that has the first
    /// operator break? A chain of flows is one flow, and so is a `pair` flow that another follows.
    fn is_first_break(
        &self,
        cx: &FormatCtx<'_>,
        node: &SyntaxNode,
        es: &[Item],
        is_block: &dyn Fn(&SyntaxNode) -> bool,
    ) -> bool {
        if EmbedMode::of(cx.options) != Some(EmbedMode::Continued)
            || !self.has_operator_break(cx, es)
        {
            return false;
        }
        let mut top = node.clone();
        while let Some(p) = top.parent().filter(|p| self.flows.contains(&p.kind())) {
            let same = p.kind() == top.kind()
                || (self.pair.contains(&p.kind()) && self.pair.contains(&top.kind()));
            if !same || p.first_child().as_ref() != Some(&top) {
                return false;
            }
            top = p;
        }
        let in_first = top
            .ancestors()
            .last()
            .is_some_and(|root| cx.first_token(&root) == cx.first_token(node));
        let mut below = node.first_child();
        while let Some(c) = below {
            if self.flows.contains(&c.kind())
                && c.kind() != self.leaf
                && self.has_operator_break(cx, &self.items(cx, &c))
            {
                return false;
            }
            below = c.first_child();
        }
        in_first && self.chain_led(&top, is_block)
    }

    /// The children of a flow joined by a space, with the layout of `a &&` + line break + `b`
    /// kept (the rest indented one level) and line continuations kept. `continuation` ends a
    /// line that a source continuation keeps; `doc_of` lays one child out.
    pub fn layout(
        &self,
        cx: &FormatCtx<'_>,
        node: &SyntaxNode,
        continuation: &Doc,
        is_block: &dyn Fn(&SyntaxNode) -> bool,
        doc_of: &dyn Fn(&SyntaxElement) -> Result<Doc, FormatError>,
    ) -> Result<Doc, FormatError> {
        let es = self.items(cx, node);
        let mut head = Vec::new();
        let mut tail: Option<Vec<Doc>> = None;
        let op_first = cx
            .options
            .is(core_config::opt::BINARY_OPERATOR_LINE, "start");
        let first_of = |e: &Item| match &e.el {
            SyntaxElement::Token(t) => Some(t.clone()),
            SyntaxElement::Node(n) => cx.first_token(n),
        };
        let last_of = |e: &Item| match &e.el {
            SyntaxElement::Token(t) => Some(t.clone()),
            SyntaxElement::Node(n) => cx.last_token(n),
        };
        // (a pipe after a list operator: its first broken `|` stands level with that operator,
        // the others one level in)
        let mut flat_first = op_first
            && node.kind() == self.pipe
            && cx.first_token(node).is_some_and(|t| {
                let mut prev = t.prev_token();
                while let Some(p) = prev.clone().filter(|p| cx.grammar.is_trivia(p.kind())) {
                    prev = p.prev_token();
                }
                prev.is_some_and(|p| self.lead_operators.contains(&p.text()))
            });
        for (i, e) in es.iter().enumerate() {
            let d = doc_of(&e.el)?;
            if i == 0 {
                head.push(d);
                continue;
            }
            let prev = &es[i - 1];
            let (e_op, prev_op) = (self.is_op(&e.el), self.is_op(&prev.el));
            if op_first && (e_op || prev_op) {
                // A break next to an operator goes before it (`a \` + line break + `&& b`).
                let sep = if e_op {
                    let broken = self.broken_op(cx, &es, i);
                    if broken && std::mem::take(&mut flat_first) && tail.is_none() {
                        head.extend([continuation.clone(), doc::hardline()]);
                        head.push(d);
                        continue;
                    } else if broken {
                        tail.get_or_insert_with(Vec::new);
                        vec![continuation.clone(), doc::hardline()]
                    } else {
                        vec![doc::lit(" ")]
                    }
                } else {
                    vec![doc::lit(" ")]
                };
                let t = tail.as_mut().unwrap_or(&mut head);
                t.extend(sep);
                t.push(d);
                continue;
            }
            let broken_after_op =
                prev_op && (prev.cont || prev.el.as_token().is_some_and(|t| self.breaks(cx, t)));
            let glued = !e.cont
                && !broken_after_op
                && !prev_op
                && !e_op
                && !self.unglued.contains(&e.el.kind())
                && last_of(prev).and_then(|t| t.next_token()) == first_of(e);
            let sep = if glued {
                vec![]
            } else if e.cont && e_op {
                vec![doc::lit(" ")]
            } else if e.cont {
                vec![continuation.clone(), doc::hardline()]
            } else if broken_after_op {
                vec![doc::hardline()]
            } else {
                vec![doc::lit(" ")]
            };
            if ((e.cont && !e_op) || broken_after_op) && tail.is_none() {
                tail = Some(Vec::new());
            }
            let target = tail.as_mut().unwrap_or(&mut head);
            target.extend(sep);
            target.push(d);
        }
        if let Some(tail) = tail {
            // (the first statement's flow with the first operator break: what stands before it
            // goes one level in from the instruction, the lines from the break on stand where the
            // flow puts them)
            if self.is_first_break(cx, node, &es, is_block) {
                head = vec![doc::indent(doc::concat(head))];
            }
            head.push(doc::indent(doc::concat(tail)));
        }
        Ok(doc::concat(head))
    }
}

/// The statements of embedded code on lines of their own: every line after the first one level
/// in from the instruction, and the first too unless it is a chain that names its own
/// indentation (`natural`, see [`OperatorFlow::chain_led`]).
pub fn embedded_statement(chunk: Doc, embedded: bool, natural: bool) -> Doc {
    if embedded && !natural {
        doc::indent(chunk)
    } else {
        chunk
    }
}
