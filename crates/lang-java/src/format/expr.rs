//! Expressions: operator chains, method chains, lambdas, assignments and initializers.

use super::*;

pub(super) fn rule(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    kind: core_syntax::SyntaxKind,
) -> Option<R> {
    Some(match kind {
        k::BINARY_EXPRESSION => binary(cx, node),
        k::TERNARY_EXPRESSION => ternary(cx, node),
        k::METHOD_INVOCATION => chain(cx, node),
        k::LAMBDA_EXPRESSION => lambda(cx, node),
        k::ASSIGNMENT_EXPRESSION | k::ELEMENT_VALUE_PAIR => assignment(cx, node),
        k::UNARY_EXPRESSION | k::UPDATE_EXPRESSION => tight(cx, node),
        k::CAST_EXPRESSION => cast(cx, node),
        k::ARRAY_INITIALIZER | k::ELEMENT_VALUE_ARRAY_INITIALIZER => initializer(cx, node),
        _ => return None,
    })
}

fn precedence(op: &str) -> u8 {
    match op {
        "||" => 1,
        "&&" => 2,
        "|" => 3,
        "^" => 4,
        "&" => 5,
        "==" | "!=" => 6,
        "<" | ">" | "<=" | ">=" => 7,
        "<<" | ">>" | ">>>" => 8,
        "+" | "-" => 9,
        _ => 10,
    }
}

/// The operands and operators of a chain of binary operators of one precedence.
fn operands(cx: &FormatCtx<'_>, node: &SyntaxNode, out: &mut Vec<SyntaxElement>) {
    let els = kids(cx, node);
    let prec = els
        .get(1)
        .and_then(|o| first(cx, o))
        .map(|t| precedence(t.text()));
    for (i, e) in els.iter().enumerate() {
        match e {
            SyntaxElement::Node(n)
                if i == 0
                    && n.kind() == k::BINARY_EXPRESSION
                    && kids(cx, n)
                        .get(1)
                        .and_then(|o| first(cx, o))
                        .map(|t| precedence(t.text()))
                        == prec =>
            {
                operands(cx, n, out);
            }
            _ => out.push(e.clone()),
        }
    }
}

fn binary(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut els = Vec::new();
    operands(cx, node, &mut els);
    let mut parts = vec![el(cx, &els[0])?];
    let mut rest = Vec::new();
    let mut prev_operand = &els[0];
    for pair in els[1..].chunks(2) {
        let [op, rhs] = pair else {
            return generic(cx, node);
        };
        rest.push(line_after(cx, prev_operand));
        rest.push(el(cx, op)?);
        rest.push(gap_after(cx, op));
        rest.push(el(cx, rhs)?);
        prev_operand = rhs;
    }
    parts.push(cont(doc::concat(rest)));
    Ok(doc::group(doc::concat(parts)))
}

fn ternary(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    if els.len() != 5 {
        return generic(cx, node);
    }
    Ok(doc::group(doc::concat(vec![
        el(cx, &els[0])?,
        cont(doc::concat(vec![
            line_after(cx, &els[0]),
            el(cx, &els[1])?,
            gap_after(cx, &els[1]),
            el(cx, &els[2])?,
            line_after(cx, &els[2]),
            el(cx, &els[3])?,
            gap_after(cx, &els[3]),
            el(cx, &els[4])?,
        ])),
    ])))
}

fn assign_op(e: &SyntaxElement) -> bool {
    matches!(e, SyntaxElement::Token(t)
        if t.text().ends_with('=') && !matches!(t.text(), "==" | "!=" | "<=" | ">="))
}

/// `lhs = rhs`: the right side moves to the next line when it does not fit.
pub(super) fn assign_split(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> Option<R> {
    let eq = els.iter().position(assign_op)?;
    let lhs = match seq(cx, &els[..=eq]) {
        Ok(d) => d,
        Err(e) => return Some(Err(e)),
    };
    let rhs = &els[eq + 1..];
    if rhs
        .first()
        .is_some_and(|e| matches!(e.kind(), k::ARRAY_INITIALIZER | k::LAMBDA_EXPRESSION))
    {
        return Some(seq(cx, rhs).map(|r| doc::concat(vec![lhs, doc::lit(" "), r])));
    }
    Some(seq(cx, rhs).map(|r| {
        doc::group(doc::concat(vec![
            lhs,
            cont(doc::group(doc::concat(vec![doc::line(), r]))),
        ]))
    }))
}

fn assignment(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    assign_split(cx, &kids(cx, node)).unwrap_or_else(|| generic(cx, node))
}

fn lambda(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let Some(arrow) = els.iter().position(|e| e.kind() == k::ARROW) else {
        return generic(cx, node);
    };
    let head = seq(cx, &els[..=arrow])?;
    let body = &els[arrow + 1..];
    if body.first().is_some_and(|b| b.kind() == k::BLOCK) {
        return Ok(doc::concat(vec![head, doc::lit(" "), seq(cx, body)?]));
    }
    let sep = if matches!(gap_after(cx, &els[arrow]), Doc::Nil | Doc::Text(_)) {
        doc::line()
    } else {
        doc::hardline()
    };
    Ok(doc::group(doc::concat(vec![
        head,
        cont(doc::group(doc::concat(vec![sep, seq(cx, body)?]))),
    ])))
}

/// Prefix and postfix operators stick to their operand.
fn tight(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let mut parts = Vec::new();
    let mut prev: Option<SyntaxToken> = None;
    for e in &els {
        if let (Some(a), Some(b)) = (&prev, first(cx, e)) {
            let (x, y) = (a.text(), b.text());
            if (x.ends_with('-') && y.starts_with('-')) || (x.ends_with('+') && y.starts_with('+'))
            {
                parts.push(doc::lit(" "));
            }
        }
        parts.push(el(cx, e)?);
        prev = last(cx, e);
    }
    Ok(doc::concat(parts))
}

/// `(Type) expr`: a long operand moves below the cast.
fn cast(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let Some(close) = els.iter().position(|e| is_tok(e, ")")) else {
        return generic(cx, node);
    };
    if close + 2 != els.len() {
        return generic(cx, node);
    }
    Ok(doc::group(doc::concat(vec![
        seq(cx, &els[..=close])?,
        cont(doc::concat(vec![doc::line(), el(cx, &els[close + 1])?])),
    ])))
}

/// `{a, b}`: on one line, else after the `{` one per line, or filled when the elements are
/// literals or arrays. A trailing comma of the source stays and breaks the braces.
fn initializer(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let (Some(open), Some(close)) = (els.first(), els.last()) else {
        return generic(cx, node);
    };
    let inner = &els[1..els.len() - 1];
    let items: Vec<_> = inner.iter().filter(|e| !is_tok(e, ",")).collect();
    if items.is_empty() {
        return seq(cx, &els);
    }
    let trailing = inner.last().is_some_and(|e| is_tok(e, ","));
    let filled = items
        .iter()
        .all(|e| matches!(e.kind(), k::CHAR | k::NUMBER | k::ARRAY_INITIALIZER));
    let commas: Vec<_> = inner.iter().filter(|e| is_tok(e, ",")).collect();
    let mut parts = Vec::new();
    for (i, e) in items.iter().enumerate() {
        let comma = match commas.get(i) {
            Some(c) => el(cx, c)?,
            None => doc::nil(),
        };
        parts.push(doc::concat(vec![el(cx, e)?, comma]));
        if i + 1 < items.len() {
            let broken = commas
                .get(i)
                .and_then(|c| c.as_token())
                .is_some_and(|c| line_comment_after(cx, c));
            parts.push(if broken { doc::hardline() } else { doc::line() });
        }
    }
    let body = if filled {
        doc::fill(parts)
    } else {
        doc::concat(parts)
    };
    let mut out = vec![
        el(cx, open)?,
        doc::indent(doc::concat(vec![doc::softline(), body])),
        doc::softline(),
        el(cx, close)?,
    ];
    if trailing {
        out.push(doc::break_parent());
    }
    Ok(doc::group(doc::concat(out)))
}

/// One link of a method chain: `.name(args)` or `.field`.
struct Link {
    els: Vec<SyntaxElement>,
    call: bool,
}

fn links(cx: &FormatCtx<'_>, node: &SyntaxNode, out: &mut Vec<Link>) -> Vec<SyntaxElement> {
    let els = flat(cx, node);
    let mut base = Vec::new();
    let mut cur: Vec<SyntaxElement> = Vec::new();
    let mut seen_dot = false;
    for e in els {
        if is_tok(&e, ".") && !seen_dot {
            seen_dot = true;
            cur.push(e);
            continue;
        }
        if !seen_dot {
            match &e {
                SyntaxElement::Node(n)
                    if matches!(n.kind(), k::METHOD_INVOCATION | k::FIELD_ACCESS) =>
                {
                    base = links(cx, n, out);
                }
                _ => base.push(e),
            }
        } else {
            cur.push(e);
        }
    }
    if seen_dot {
        let call = cur.iter().any(|e| e.kind() == k::ARGUMENT_LIST);
        out.push(Link { els: cur, call });
    }
    base
}

fn chain(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut out = Vec::new();
    let base = links(cx, node, &mut out);
    let simple = matches!(base.as_slice(), [SyntaxElement::Token(_)])
        || matches!(base.as_slice(), [SyntaxElement::Node(n)] if matches!(n.kind(), k::TYPE_NAME | k::THIS_EXPRESSION | k::SUPER_EXPRESSION));
    let call_base = !simple
        && base.last().is_some_and(|e| match e.as_node() {
            Some(n) if n.kind() == k::OBJECT_CREATION_EXPRESSION => {
                !n.children().any(|c| c.kind() == k::CLASS_BODY)
            }
            _ => true,
        });
    let calls = out.iter().filter(|l| l.call).count() + usize::from(call_base);
    if calls < 2 {
        let mut all = base;
        for l in out {
            all.extend(l.els);
        }
        return seq(cx, &all);
    }
    let keep = prefix_len(&base, &out);
    let mut head = base;
    for l in &out[..keep] {
        head.extend(l.els.iter().cloned());
    }
    let mut parts = vec![seq_with(cx, &head, |e| match e.as_node() {
        Some(n) if e.kind() == k::ARGUMENT_LIST => decl::paren_list_extra(cx, n),
        _ => el(cx, e),
    })?];
    let mut rest = Vec::new();
    for l in &out[keep..] {
        rest.push(doc::softline());
        rest.push(seq(cx, &l.els)?);
    }
    parts.push(cont(doc::concat(rest)));
    Ok(doc::group(doc::concat(parts)))
}

/// The name of a link (`.name(...)`, `.name`).
fn link_name(l: &Link) -> String {
    l.els
        .iter()
        .find(|e| matches!(e.kind(), k::METHOD_NAME | k::IDENTIFIER))
        .map(|e| match e {
            SyntaxElement::Node(n) => n.text().to_string(),
            SyntaxElement::Token(t) => t.text().to_string(),
        })
        .unwrap_or_default()
}

/// How many links stay on the line of the base: a type name (`Foo.bar()`, `a.b.Foo.bar()`) and
/// `this.field` keep the names in front of the first call and that call, `x.stream()` keeps the
/// `stream()`.
fn prefix_len(base: &[SyntaxElement], out: &[Link]) -> usize {
    let upper = |s: &str| s.chars().next().is_some_and(char::is_uppercase);
    let first_call = out.iter().position(|l| l.call).unwrap_or(out.len());
    let name = match base {
        [SyntaxElement::Token(t)] => t.text().to_string(),
        [SyntaxElement::Node(n)] if matches!(n.kind(), k::TYPE_NAME | k::THIS_EXPRESSION) => {
            n.text().to_string()
        }
        _ => String::new(),
    };
    let mut keep = 0;
    if name.is_empty() {
        return out
            .iter()
            .take(2)
            .position(|l| link_name(l) == "stream")
            .map_or(0, |i| i + 1);
    }
    if name == "this" {
        keep = first_call.min(1);
    } else if upper(&name) || name.len() <= 4 {
        keep = (first_call + 1).min(out.len());
    } else if let Some(i) = out[..first_call].iter().position(|l| upper(&link_name(l))) {
        keep = (first_call + 1).min(out.len()).max(i + 1);
    }
    if let Some(i) = out.iter().take(2).position(|l| link_name(l) == "stream") {
        keep = keep.max(i + 1);
    }
    keep
}
