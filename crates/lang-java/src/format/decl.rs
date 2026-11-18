//! Declarations: the file, types and their bodies, members, modifiers, annotations, parameters.

use super::*;
use core_format::shapes::{self, ListStyle};

pub(super) fn rule(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    kind: core_syntax::SyntaxKind,
) -> Option<R> {
    Some(match kind {
        k::CLASS_DECLARATION
        | k::INTERFACE_DECLARATION
        | k::ENUM_DECLARATION
        | k::RECORD_DECLARATION
        | k::ANNOTATION_TYPE_DECLARATION => type_decl(cx, node),
        k::CLASS_BODY | k::INTERFACE_BODY | k::RECORD_BODY | k::ANNOTATION_TYPE_BODY => {
            members(cx, node)
        }
        k::ENUM_BODY => enum_body(cx, node),
        k::METHOD_DECLARATION | k::CONSTRUCTOR_DECLARATION | k::COMPACT_CONSTRUCTOR_DECLARATION => {
            method(cx, node)
        }
        k::FIELD_DECLARATION | k::CONSTANT_DECLARATION => variables(cx, node, true),
        k::LOCAL_VARIABLE_DECLARATION => variables(cx, node, false),
        k::MODIFIERS => modifiers(cx, node),
        k::THROWS => throws(cx, node),
        k::FORMAL_PARAMETER | k::SPREAD_PARAMETER => parameter(cx, node),
        k::RESOURCE_SPECIFICATION => resources(cx, node),
        k::FORMAL_PARAMETERS | k::RECORD_HEADER => hanging_list(cx, node, None),
        k::ARGUMENT_LIST | k::ANNOTATION_ARGUMENT_LIST => paren_list(cx, node),
        k::PACKAGE_DECLARATION => package(cx, node),
        k::TYPE_PARAMETERS | k::TYPE_ARGUMENTS => angle_list(cx, node, None),
        k::VARIABLE_DECLARATOR => declarator(cx, node),
        k::ENUM_CONSTANT => enum_constant(cx, node),
        _ => return None,
    })
}

/// The file: package, imports (static first), then the types, with blank lines between.
pub(super) fn program(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let items = flat(cx, node);
    let eof = items.last().and_then(|e| e.as_token().cloned());
    let items: Vec<_> = items
        .into_iter()
        .filter(|e| e.kind() != k::EOF && !is_tok(e, ";"))
        .collect();
    let mut parts = Vec::new();
    let force = |a: &SyntaxElement, b: &SyntaxElement| {
        let import = |e: &SyntaxElement| e.kind() == k::IMPORT_DECLARATION;
        let is_static = |e: &SyntaxElement| {
            e.as_node().is_some_and(|n| {
                kids(cx, n)
                    .iter()
                    .chain(flat(cx, n).iter())
                    .any(|x| is_tok(x, "static"))
            })
        };
        !(import(a) && import(b) && is_static(a) == is_static(b))
    };
    for (i, e) in items.iter().enumerate() {
        if i > 0 {
            parts.push(doc::hardline());
            let import = |e: &SyntaxElement| e.kind() == k::IMPORT_DECLARATION;
            if force(&items[i - 1], e) || (!import(e) && blank_before(cx, e)) {
                parts.push(doc::hardline());
            }
        }
        parts.push(el(cx, e)?);
    }
    if let Some(eof) = eof {
        let lead = comments::leading_comments(cx.grammar, &eof);
        if !lead.is_empty() {
            if !items.is_empty() {
                parts.push(doc::hardline());
                if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                    parts.push(doc::hardline());
                }
            }
            parts.push(core_format::context::leading_comments_doc(&lead));
        }
    }
    Ok(doc::concat(parts))
}

fn is_body(kind: core_syntax::SyntaxKind) -> bool {
    matches!(
        kind,
        k::CLASS_BODY | k::INTERFACE_BODY | k::ENUM_BODY | k::RECORD_BODY | k::ANNOTATION_TYPE_BODY
    )
}

fn type_decl(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = flat(cx, node);
    let at = els.iter().position(|e| is_body(e.kind()));
    let (head, body) = els.split_at(at.unwrap_or(els.len()));
    let mut parts = Vec::new();
    let mut rest = head;
    if let Some(m) = head.first().filter(|e| e.kind() == k::MODIFIERS) {
        parts.push(modifiers_then(cx, m, true, false)?);
        rest = &head[1..];
    }
    // Everything up to the name and type parameters stays on a line; supertypes break before
    // their keyword when the header does not fit.
    let split = rest
        .iter()
        .position(|e| {
            matches!(
                e.kind(),
                k::SUPERCLASS | k::SUPER_INTERFACES | k::EXTENDS_INTERFACES | k::PERMITS
            )
        })
        .unwrap_or(rest.len());
    let id = doc::new_group_id();
    let mut header = vec![seq_with(cx, &rest[..split], |e| match e.as_node() {
        Some(n) if e.kind() == k::TYPE_PARAMETERS => angle_list(cx, n, Some(id)),
        _ => el(cx, e),
    })?];
    let mut tail = Vec::new();
    for (i, e) in rest[split..].iter().enumerate() {
        let clause = doc::shared(el(cx, e)?);
        let flat = doc::group(doc::concat(vec![doc::line(), clause.clone()]));
        tail.push(if i == 0 {
            doc::if_break_in(id, doc::concat(vec![doc::hardline(), clause]), flat)
        } else {
            flat
        });
    }
    if !tail.is_empty() {
        header.push(cont(doc::concat(tail)));
    }
    parts.push(doc::group(doc::concat(header)));
    if let Some(b) = body.first() {
        parts.push(doc::lit(" "));
        parts.push(el(cx, b)?);
    }
    Ok(doc::concat(parts))
}

/// The members of a body, one per line: a blank line before everything but a field without
/// javadoc, and where the source has one.
fn members(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    shapes::block(cx, node, &member_style())
}

fn member_style() -> shapes::BlockStyle {
    shapes::BlockStyle {
        force_blank: Some(member_blank),
        keep_leading_blank: true,
        ..shapes::BlockStyle::default()
    }
}

/// A blank line goes between two members unless both are plain fields.
fn member_blank(cx: &FormatCtx<'_>, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
    let spaced = |n: &SyntaxNode| {
        !matches!(n.kind(), k::FIELD_DECLARATION | k::CONSTANT_DECLARATION)
            || has_javadoc(cx, n)
            || has_annotation_args(cx, n)
    };
    spaced(prev) || spaced(next)
}

fn has_javadoc(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    cx.first_token(n).is_some_and(|t| {
        comments::leading_trivia(cx.grammar, &t)
            .iter()
            .any(|x| x.kind() == k::COMMENT && x.to_string().starts_with("/**"))
    })
}

fn has_annotation_args(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    kids(cx, n).iter().any(|m| {
        m.as_node().is_some_and(|m| {
            m.kind() == k::MODIFIERS && kids(cx, m).iter().any(|a| has_args(cx, a))
        })
    })
}

fn enum_body(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (els.first().cloned(), els.last().cloned())
    else {
        return generic(cx, node);
    };
    let inner = &els[1..els.len() - 1];
    let consts: Vec<_> = inner
        .iter()
        .filter(|e| e.kind() == k::ENUM_CONSTANT)
        .collect();
    let decls = inner.iter().find(|e| e.kind() == k::ENUM_BODY_DECLARATIONS);
    let commas: Vec<_> = inner.iter().filter(|e| is_tok(e, ",")).collect();
    let mut parts = Vec::new();
    for (i, c) in consts.iter().enumerate() {
        if i > 0 {
            parts.push(el(cx, commas[i - 1])?);
            parts.push(doc::hardline());
            if blank_before(cx, c) {
                parts.push(doc::hardline());
            }
        }
        parts.push(el(cx, c)?);
    }
    let comma_last = inner
        .iter()
        .rev()
        .find(|e| !matches!(e.kind(), k::ENUM_BODY_DECLARATIONS | k::SEMI) && !is_tok(e, ";"))
        .is_some_and(|e| is_tok(e, ","));
    if comma_last {
        parts.push(el(cx, commas[consts.len() - 1])?);
    }
    let semi_el = inner.iter().find(|e| is_tok(e, ";")).cloned().or_else(|| {
        decls
            .and_then(|d| d.as_node())
            .and_then(|d| kids(cx, d).into_iter().find(|e| is_tok(e, ";")))
    });
    let semi = decls.is_some() || semi_el.is_some();
    let semi_doc = |cx: &FormatCtx<'_>| match &semi_el {
        Some(e) => el(cx, e),
        None => Ok(doc::text(";")),
    };
    if semi && !decls.is_some_and(|d| d.as_node().is_some_and(|n| kids(cx, n).len() > 1)) {
        if comma_last {
            parts.push(doc::hardline());
        }
        parts.push(semi_doc(cx)?);
    }
    if let Some(d) = decls.and_then(|d| d.as_node()) {
        let members: Vec<SyntaxNode> = d.children().collect();
        if !members.is_empty() {
            if comma_last {
                parts.push(doc::hardline());
            }
            parts.push(semi_doc(cx)?);
        }
        if !members.is_empty() {
            parts.push(doc::hardline());
            parts.push(doc::hardline());
            parts.push(shapes::statement_sequence_with(
                cx,
                &members,
                1,
                Some(member_blank),
                false,
            )?);
        }
    }
    let style = shapes::BlockStyle::default();
    if parts.is_empty() {
        return Ok(shapes::braces(cx, &open, &close, None, false, &style));
    }
    let lead_blank = consts
        .first()
        .and_then(|c| c.as_node())
        .is_some_and(|c| has_javadoc(cx, c) && shapes::blank_before(cx, c));
    Ok(shapes::braces(
        cx,
        &open,
        &close,
        Some(doc::concat(parts)),
        lead_blank,
        &style,
    ))
}

fn enum_constant(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    generic(cx, node)
}

/// The modifiers and what separates them from the rest: annotations in front of a declaration
/// go on lines of their own when `vertical`, the keywords and other annotations stay on the line.
fn modifiers_then(cx: &FormatCtx<'_>, m: &SyntaxElement, vertical: bool, soft: bool) -> R {
    let Some(node) = m.as_node() else {
        return el(cx, m);
    };
    let els = kids(cx, node);
    let mut lead = els
        .iter()
        .take_while(|e| matches!(e.kind(), k::ANNOTATION | k::MARKER_ANNOTATION))
        .count();
    while lead > 0 && is_type_use(&els[lead - 1], node) {
        lead -= 1;
    }
    let mut parts = Vec::new();
    if vertical {
        for e in &els[..lead] {
            parts.push(el(cx, e)?);
            parts.push(doc::hardline());
        }
        if lead < els.len() {
            parts.push(seq(cx, &els[lead..])?);
            parts.push(doc::lit(" "));
        }
    } else if soft && lead > 0 {
        parts.push(seq(cx, &els[..lead])?);
        parts.push(doc::line());
        if lead < els.len() {
            parts.push(seq(cx, &els[lead..])?);
            parts.push(doc::lit(" "));
        }
    } else {
        parts.push(seq(cx, &els)?);
        parts.push(doc::lit(" "));
    }
    Ok(doc::concat(parts))
}

fn modifiers(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    seq(cx, &kids(cx, node))
}

fn method(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = flat(cx, node);
    let mut parts = Vec::new();
    let mut rest = &els[..];
    if let Some(m) = els.first().filter(|e| e.kind() == k::MODIFIERS) {
        parts.push(modifiers_then(cx, m, true, false)?);
        rest = &els[1..];
    }
    let block = rest
        .iter()
        .position(|e| matches!(e.kind(), k::BLOCK | k::CONSTRUCTOR_BODY));
    let (head, tail) = rest.split_at(block.unwrap_or(rest.len()));
    let (head, semi) = match head.split_last() {
        Some((s, h)) if is_tok(s, ";") => (h, Some(s)),
        _ => (head, None),
    };
    let throws = head.iter().position(|e| e.kind() == k::THROWS);
    let (sig, thr) = head.split_at(throws.unwrap_or(head.len()));
    let params = sig.iter().position(|e| e.kind() == k::FORMAL_PARAMETERS);
    let (pre, post) = sig.split_at(params.unwrap_or(sig.len()));
    // A long return type moves below the type parameters; the throws clause moves to its own
    // line when the parameters are one per line, or when it does not fit after them.
    let id = doc::new_group_id();
    let (type_params, head) = match pre {
        [tp, rest @ ..] if !rest.is_empty() && tp.kind() == k::TYPE_PARAMETERS => {
            (Some(el(cx, tp)?), Some(seq(cx, rest)?))
        }
        _ => (None, Some(seq(cx, pre)?)),
    };
    let mut params_doc = Vec::new();
    for e in post {
        params_doc.push(match e.as_node() {
            Some(n) if e.kind() == k::FORMAL_PARAMETERS => hanging_list(cx, n, Some(id))?,
            _ => el(cx, e)?,
        });
    }
    let clauses = thr
        .iter()
        .map(|e| el(cx, e))
        .collect::<Result<Vec<_>, _>>()?;
    if !pre.is_empty() || !post.is_empty() || !clauses.is_empty() {
        parts.push(shapes::signature(shapes::Signature {
            type_params,
            head,
            params: doc::concat(params_doc),
            params_id: Some(id),
            tail: clauses,
            continuation: 2,
        }));
    }
    if let Some(s) = semi {
        parts.push(el(cx, s)?);
    }
    if !tail.is_empty() {
        let end = tail[0].clone();
        if !is_tok(&end, ";") {
            parts.push(doc::lit(" "));
        }
        parts.push(seq(cx, tail)?);
    }
    Ok(doc::concat(parts))
}

/// `throws A, B`: the continuation lines are indented under the first type.
fn throws(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    let els = kids(cx, node);
    for (i, e) in els.iter().enumerate() {
        if i == 1 {
            parts.push(doc::lit(" "));
        }
        parts.push(el(cx, e)?);
        if is_tok(e, ",") {
            parts.push(cont(doc::line()));
        }
    }
    Ok(doc::group(doc::concat(parts)))
}

/// `Type name`: a long type moves the name to the next line.
fn parameter(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = flat(cx, node);
    let Some((name, head)) = els.split_last().filter(|(_, h)| !h.is_empty()) else {
        return generic(cx, node);
    };
    if !matches!(name.kind(), k::IDENTIFIER) {
        return generic(cx, node);
    }
    Ok(doc::group(doc::concat(vec![
        seq(cx, head)?,
        cont(doc::concat(vec![doc::line(), el(cx, name)?])),
    ])))
}

/// `try (A a = x; B b = y)`: the resources one below the other when they do not fit.
fn resources(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let mut parts = Vec::new();
    for e in &els {
        parts.push(el(cx, e)?);
        if is_tok(e, ";") {
            parts.push(cont(doc::line()));
        }
    }
    Ok(doc::group(doc::concat(parts)))
}

/// A field or a local variable: `modifiers Type name = init, name2 = init2;`.
fn variables(cx: &FormatCtx<'_>, node: &SyntaxNode, _field: bool) -> R {
    let els = flat(cx, node);
    let mut parts = Vec::new();
    let mut rest = &els[..];
    let mut annotated = false;
    if let Some(m) = els.first().filter(|e| e.kind() == k::MODIFIERS) {
        let node = m.as_node().unwrap();
        let has = |kind| kids(cx, node).iter().any(|a| a.kind() == kind);
        let vertical = kids(cx, node).iter().any(|a| has_args(cx, a));
        let _ = has;
        annotated = !vertical
            && kids(cx, node)
                .first()
                .is_some_and(|a| matches!(a.kind(), k::MARKER_ANNOTATION | k::ANNOTATION));
        parts.push(modifiers_then(cx, m, vertical, annotated)?);
        rest = &els[1..];
    }
    parts.push(seq(cx, rest)?);
    let d = doc::concat(parts);
    Ok(if annotated { doc::group(d) } else { d })
}

/// `name = init`: break after the `=` when the initializer does not fit on the line.
fn declarator(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = flat(cx, node);
    match els.iter().position(|e| e.kind() == k::ASSIGN) {
        Some(eq) => expr::assign_at(cx, node, &els, eq),
        None => seq(cx, &els),
    }
}

/// `( a, b )`: all on one line, else after the opening parenthesis on one continuation line,
/// else one per line.
pub(super) fn paren_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    paren_list_in(cx, node, false)
}

/// [`paren_list`] inside a method chain: one more continuation level.
pub(super) fn paren_list_extra(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    paren_list_in(cx, node, true)
}

fn paren_list_in(cx: &FormatCtx<'_>, node: &SyntaxNode, extra: bool) -> R {
    let els = kids(cx, node);
    let (Some(open), Some(close)) = (els.first(), els.last()) else {
        return generic(cx, node);
    };
    if els.len() == 2 {
        return seq(cx, &els);
    }
    let items = &els[1..els.len() - 1];
    let args = node.kind() == k::ARGUMENT_LIST;
    let format = args
        && items.len() > 2
        && items.first().is_some_and(|e| {
            first(cx, e).is_some_and(|t| t.kind() == k::STRING)
                && matches!(e.kind(), k::STRING | k::BINARY_EXPRESSION)
                && match e {
                    SyntaxElement::Token(t) => t.text().contains('%'),
                    SyntaxElement::Node(n) => n.text().to_string().contains('%'),
                }
        });
    let mut parts = Vec::new();
    let mut rest = Vec::new();
    for (i, e) in items.iter().enumerate() {
        let sink = if format && i >= 2 {
            &mut rest
        } else {
            &mut parts
        };
        sink.push(el(cx, e)?);
        if is_tok(e, ",") {
            sink.push(doc::line());
        }
    }
    if format {
        parts.push(doc::group(doc::concat(rest)));
    }
    let hug = items.len() == 1
        && items[0].as_node().is_some_and(|n| match n.kind() {
            k::LAMBDA_EXPRESSION => n.children().any(|c| c.kind() == k::BLOCK),
            k::ELEMENT_VALUE_ARRAY_INITIALIZER | k::ARRAY_INITIALIZER => {
                node.kind() == k::ANNOTATION_ARGUMENT_LIST
            }
            _ => false,
        });
    if hug {
        return Ok(doc::group(doc::concat(vec![
            el(cx, open)?,
            doc::concat(parts),
            el(cx, close)?,
        ])));
    }
    let body = doc::concat(parts);
    // Annotation arguments break all or none.
    let body = if node.kind() == k::ANNOTATION_ARGUMENT_LIST {
        body
    } else {
        doc::group(body)
    };
    let inner = cont(doc::concat(vec![doc::softline(), body]));
    Ok(doc::group(doc::concat(vec![
        el(cx, open)?,
        if extra { cont(inner) } else { inner },
        el(cx, close)?,
    ])))
}

#[allow(unused_imports)]
use shapes as _shapes;

/// Annotations of a package go on lines of their own.
fn package(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let lead = els
        .iter()
        .take_while(|e| matches!(e.kind(), k::ANNOTATION | k::MARKER_ANNOTATION))
        .count();
    let mut parts = Vec::new();
    for e in &els[..lead] {
        parts.push(el(cx, e)?);
        parts.push(doc::hardline());
    }
    parts.push(seq(cx, &els[lead..])?);
    Ok(doc::concat(parts))
}

/// `<A, B>`: on one line, else after the `<` two continuation levels in. `id`: the group of the
/// whole list.
fn angle_list(cx: &FormatCtx<'_>, node: &SyntaxNode, id: Option<doc::GroupId>) -> R {
    let els = kids(cx, node);
    if els.len() < 3 {
        return generic(cx, node);
    }
    let mut parts = Vec::new();
    for e in &els[1..els.len() - 1] {
        parts.push(el(cx, e)?);
        if is_tok(e, ",") {
            parts.push(doc::line());
        }
    }
    let d = doc::concat(vec![
        el(cx, &els[0])?,
        cont(cont(doc::concat(vec![
            doc::softline(),
            doc::group(doc::concat(parts)),
        ]))),
        el(cx, &els[els.len() - 1])?,
    ]);
    Ok(match id {
        Some(id) => doc::group_with_id(id, d),
        None => doc::group(d),
    })
}

/// `(A a, B b)`: on one line, else on the line after the `(`, else one per line. `id`: the
/// group of the items (to ask whether they broke).
fn hanging_list(cx: &FormatCtx<'_>, node: &SyntaxNode, id: Option<doc::GroupId>) -> R {
    let style = ListStyle {
        hug_close: true,
        indent_levels: 2,
        inner_id: id,
        hang: true,
        ..ListStyle::default()
    };
    shapes::delimited_list(cx, node, &style).or_else(|_| generic(cx, node))
}

/// Is the annotation one the file imports from a package of type-use annotations? Those belong
/// to the type, not to the declaration.
fn is_type_use(annotation: &SyntaxElement, at: &SyntaxNode) -> bool {
    let Some(a) = annotation
        .as_node()
        .filter(|a| a.kind() == k::MARKER_ANNOTATION)
    else {
        return false;
    };
    let text = a
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| {
            t.text().chars().all(|c| c.is_alphanumeric() || c == '_') && !t.text().is_empty()
        })
        .last()
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    let name = text.as_str();
    if !matches!(name, "Nullable" | "NonNull") {
        return false;
    }
    let Some(root) = at.ancestors().last() else {
        return false;
    };
    root.descendants().any(|n| {
        n.kind() == k::IMPORT_DECLARATION && {
            let t = n.text().to_string();
            let t = t
                .trim_start_matches("import")
                .trim()
                .trim_end_matches(';')
                .trim();
            (t.starts_with("org.jspecify.annotations.") || t.starts_with("org.checkerframework."))
                && t.rsplit('.').next() == Some(name)
        }
    })
}

/// An annotation with arguments (`@A()` has none).
fn has_args(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    e.as_node().is_some_and(|n| {
        n.kind() == k::ANNOTATION
            && kids(cx, n)
                .iter()
                .any(|a| a.as_node().is_some_and(|l| kids(cx, l).len() > 2))
    })
}
