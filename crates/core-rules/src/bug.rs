//! The `bug` family: bug rules that read the roles and role fields of the grammar (`#binary` with
//! `left`, `operator`, `right`; `#conditional` and `#loop` with `condition`; `#key-value`;
//! `#case`; `#block`, `#catch`), so one check serves every language whose grammar has them and
//! each tool's rule id is a row (a rule file with `family: bug`, `params: { check: <name> }`).
//!
//! Checks: `self-compare`, `constant-condition`, `duplicate-keys`, `duplicate-cases`,
//! `empty-block`, `empty-catch`, `cond-assign`, and `compare-literal` (params `values` and
//! `operators`, space separated texts: an operand that is one of the values compared with one of
//! the operators). `{name}` in the message is the repeated name or the literal.

use crate::rule::{Rule, RuleContext, RuleMeta};
use crate::tree::significant_range;
use crate::unique::Repeats;
use crate::{Family, Params};
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, TextRange};

pub const FAMILY: Family = Family { name: "bug", build };

#[derive(Clone, Copy)]
enum Check {
    SelfCompare,
    ConstantCondition,
    DuplicateKeys,
    DuplicateCases,
    EmptyBlock,
    EmptyCatch,
    CondAssign,
    CompareLiteral,
}

const CHECKS: &[(&str, Check, &[&str])] = &[
    ("self-compare", Check::SelfCompare, &["binary"]),
    (
        "constant-condition",
        Check::ConstantCondition,
        &["conditional", "loop"],
    ),
    ("duplicate-keys", Check::DuplicateKeys, &["key-value"]),
    ("duplicate-cases", Check::DuplicateCases, &["case"]),
    ("empty-block", Check::EmptyBlock, &["block", "switch"]),
    ("empty-catch", Check::EmptyCatch, &["catch"]),
    ("cond-assign", Check::CondAssign, &["conditional", "loop"]),
    ("compare-literal", Check::CompareLiteral, &["binary"]),
];

struct Bug {
    meta: &'static RuleMeta,
    message: String,
    check: Check,
    tags: &'static [&'static str],
    values: Vec<String>,
    operators: Vec<String>,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let name = p.text("check")?;
    let Some((_, check, tags)) = CHECKS.iter().find(|(n, ..)| *n == name) else {
        let names: Vec<&str> = CHECKS.iter().map(|(n, ..)| *n).collect();
        return Err(format!(
            "param `check`: `{name}` is not one of {}",
            names.join(", ")
        ));
    };
    let words = |text: String| text.split_whitespace().map(str::to_string).collect();
    let (mut values, mut operators) = (Vec::new(), Vec::new());
    if matches!(check, Check::CompareLiteral) {
        values = words(p.text("values")?);
        operators = words(p.text("operators")?);
    }
    let rule = Bug {
        meta,
        message: p.text("message")?,
        check: *check,
        tags,
        values,
        operators,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Bug {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn tags(&self) -> &[&'static str] {
        self.tags
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        match self.check {
            Check::SelfCompare => {
                if let Some((l, op, r)) = operands(node, g)
                    && matches!(g.operator_kind(&op), Some("equality" | "comparison"))
                {
                    let (l, r) = (text(&peel(l, g).0, g), text(&peel(r, g).0, g));
                    if l == r {
                        self.report(ctx, significant_range(node, g), &l);
                    }
                }
            }
            Check::ConstantCondition => self.constant_condition(node, ctx),
            Check::DuplicateKeys => self.duplicate_keys(node, ctx),
            Check::DuplicateCases => self.duplicate_cases(node, ctx),
            Check::EmptyBlock => self.empty_block(node, ctx),
            Check::EmptyCatch => {
                if let Some(SyntaxElement::Node(body)) = g.slot(node, "body")
                    && braces_empty(&body, ctx)
                    && let Some(first) = first_code_token(node, g)
                {
                    self.report(ctx, first, "");
                }
            }
            Check::CondAssign => self.cond_assign(node, ctx),
            Check::CompareLiteral => self.compare_literal(node, ctx),
        }
    }
}

impl Bug {
    fn report(&self, ctx: &mut RuleContext<'_>, range: TextRange, name: &str) {
        ctx.report(range, self.message.replace("{name}", name));
    }

    fn constant_condition(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let Some(cond) = g.slot(node, "condition") else {
            return;
        };
        let cond = peel(cond, g).0;
        if !g.has_tag(cond.kind(), "literal") {
            return;
        }
        let name = text(&cond, g);
        // `while (true)`, `while True`, `while (1)` are how an endless loop is written
        if g.has_tag(node.kind(), "loop") && matches!(name.as_str(), "true" | "True" | "1") {
            return;
        }
        self.report(ctx, range_of(&cond, g), &name);
    }

    fn cond_assign(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let Some(cond) = g.slot(node, "condition") else {
            return;
        };
        let start = cond.text_range().start();
        let (inner, layers) = peel(cond, g);
        let SyntaxElement::Node(assign) = inner else {
            return;
        };
        if !g.has_tag(assign.kind(), "assignment") {
            return;
        }
        // the statement's own `(` before the condition counts as a pair: `if ((a = b))` is the
        // way to say it is meant
        let own = node.children_with_tokens().any(|c| {
            matches!(&c, SyntaxElement::Token(t) if t.text() == "(" && t.text_range().end() <= start)
        });
        if layers + usize::from(own) <= 1 {
            self.report(ctx, significant_range(&assign, g), "");
        }
    }

    fn compare_literal(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let Some((l, op, r)) = operands(node, g) else {
            return;
        };
        if !self.operators.contains(&op) {
            return;
        }
        let hit = [l, r]
            .map(|e| text(&e, g))
            .into_iter()
            .find(|t| self.values.contains(t));
        if let Some(name) = hit {
            self.report(ctx, significant_range(node, g), &name);
        }
    }

    fn empty_block(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        if g.has_tag(node.kind(), "switch") {
            if g.slots(node, "cases")
                .is_some_and(|mut c| c.next().is_none())
            {
                let brace = node
                    .children_with_tokens()
                    .find(|e| e.as_token().is_some_and(|t| t.text() == "{"));
                let range = brace.map_or_else(|| significant_range(node, g), |b| b.text_range());
                self.report(ctx, range, "switch");
            }
            return;
        }
        let in_statement = node.parent().is_some_and(|p| {
            ["conditional", "loop", "try", "catch"]
                .iter()
                .any(|t| g.has_tag(p.kind(), t))
        });
        if in_statement && braces_empty(node, ctx) {
            self.report(ctx, significant_range(node, g), "block");
        }
    }

    fn duplicate_keys(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        if !starts_run(node, "key-value", g) {
            return;
        }
        let mut seen = Repeats::default();
        for entry in run(node, "key-value", g) {
            let Some(key) = g.slot(&entry, "key") else {
                continue;
            };
            let literal = g.has_tag(key.kind(), "literal");
            let before =
                |t: &core_syntax::SyntaxToken| t.text_range().end() <= key.text_range().start();
            let own = |s: &str| {
                entry
                    .children_with_tokens()
                    .any(|c| matches!(&c, SyntaxElement::Token(t) if before(t) && t.text() == s))
            };
            // accessors pair up, and a computed key that is not a literal is not known
            if own("get") || own("set") || (!literal && own("[")) {
                continue;
            }
            let name = text(&key, g);
            let name = unquote(&name).to_string();
            let range = range_of(&key, g);
            seen.check(
                ctx,
                name.clone(),
                range,
                || self.message.replace("{name}", &name),
                "first defined here",
            );
        }
    }

    fn duplicate_cases(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        if !starts_run(node, "case", g) {
            return;
        }
        let mut seen = Repeats::default();
        for case in run(node, "case", g) {
            let range = significant_range(&case, g);
            for test in g.slots(&case, "test").into_iter().flatten() {
                let key = text(&test, g);
                seen.check(ctx, key, range, || self.message.clone(), "first case");
            }
        }
    }
}

/// The `left`, `operator` text and `right` of a `#binary`.
fn operands(node: &SyntaxNode, g: &Grammar) -> Option<(SyntaxElement, String, SyntaxElement)> {
    let op = text(&g.slot(node, "operator")?, g);
    Some((g.slot(node, "left")?, op, g.slot(node, "right")?))
}

/// The source of `e` without trivia.
fn text(e: &SyntaxElement, g: &Grammar) -> String {
    match e {
        SyntaxElement::Token(t) => t.text().to_string(),
        SyntaxElement::Node(n) => n
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !g.is_trivia(t.kind()))
            .map(|t| t.text().to_string())
            .collect(),
    }
}

fn range_of(e: &SyntaxElement, g: &Grammar) -> TextRange {
    match e {
        SyntaxElement::Token(t) => t.text_range(),
        SyntaxElement::Node(n) => significant_range(n, g),
    }
}

fn first_code_token(node: &SyntaxNode, g: &Grammar) -> Option<TextRange> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !g.is_trivia(t.kind()))
        .map(|t| t.text_range())
}

/// `e` inside its parentheses (a node of `(`, one element, `)`), and how many pairs there were.
fn peel(mut e: SyntaxElement, g: &Grammar) -> (SyntaxElement, usize) {
    let mut layers = 0;
    while let SyntaxElement::Node(n) = &e {
        let mut parts = n.children_with_tokens().filter(|c| !g.is_trivia(c.kind()));
        let inner = match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(SyntaxElement::Token(a)), Some(inner), Some(SyntaxElement::Token(b)), None)
                if a.text() == "(" && b.text() == ")" =>
            {
                inner
            }
            _ => break,
        };
        e = inner;
        layers += 1;
    }
    (e, layers)
}

/// `{`, then only blank space, then `}`: no statement and no comment.
fn braces_empty(node: &SyntaxNode, ctx: &RuleContext<'_>) -> bool {
    let range = significant_range(node, ctx.file.grammar);
    let src = &ctx.file.text[usize::from(range.start())..usize::from(range.end())];
    src.strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .is_some_and(|inner| inner.trim().is_empty())
}

fn unquote(name: &str) -> &str {
    let b = name.as_bytes();
    match (b.first(), b.last()) {
        (Some(q @ (b'"' | b'\'' | b'`')), Some(e)) if q == e && b.len() >= 2 => {
            &name[1..name.len() - 1]
        }
        _ => name,
    }
}

/// Is `node` the first of its siblings with `tag`? The check of a run of entries runs there.
fn starts_run(node: &SyntaxNode, tag: &str, g: &Grammar) -> bool {
    !std::iter::successors(node.prev_sibling(), |p| p.prev_sibling())
        .any(|p| g.has_tag(p.kind(), tag))
}

/// `node` and its later siblings with `tag`.
fn run<'a>(
    node: &SyntaxNode,
    tag: &'a str,
    g: &'a Grammar,
) -> impl Iterator<Item = SyntaxNode> + 'a {
    std::iter::successors(Some(node.clone()), |n| n.next_sibling())
        .filter(move |n| g.has_tag(n.kind(), tag))
}
