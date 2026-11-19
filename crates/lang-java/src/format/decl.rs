//! Declarations: the file, types and their bodies, members, modifiers, annotations, parameters.
//!
//! concept: declaration-kit, signature-layout

use super::*;
use core_format::shapes::{self, Annotations, ListStyle, Printer};

/// Java's printing of the elements of a declaration.
pub(super) struct Java<'a>(pub(super) &'a FormatCtx<'a>);

impl Printer for Java<'_> {
    fn el(&self, e: &SyntaxElement) -> R {
        el(self.0, e)
    }
    fn seq_with(&self, els: &[SyntaxElement], doc: &dyn Fn(&SyntaxElement) -> R) -> R {
        seq_with(self.0, els, doc)
    }
}

/// The type parameters of a type, the group of which tells where its clauses go.
fn head(cx: &FormatCtx<'_>, e: &SyntaxElement, id: doc::GroupId) -> R {
    match e.as_node() {
        Some(n) if e.kind() == k::TYPE_PARAMETERS => angle_list(cx, n, Some(id)),
        _ => el(cx, e),
    }
}

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
        | k::ANNOTATION_TYPE_DECLARATION => {
            shapes::type_declaration(cx, node, &Java(cx), &|e, id| head(cx, e, id), 2)
        }
        k::CLASS_BODY | k::INTERFACE_BODY | k::RECORD_BODY | k::ANNOTATION_TYPE_BODY => {
            shapes::members(cx, node)
        }
        k::ENUM_BODY => shapes::enum_body(cx, node, &Java(cx)),
        k::METHOD_DECLARATION | k::CONSTRUCTOR_DECLARATION | k::COMPACT_CONSTRUCTOR_DECLARATION => {
            method(cx, node)
        }
        k::FIELD_DECLARATION | k::CONSTANT_DECLARATION => variables(cx, node, true),
        k::LOCAL_VARIABLE_DECLARATION => variables(cx, node, false),
        k::MODIFIERS => modifiers(cx, node),
        k::THROWS | k::SUPER_INTERFACES | k::EXTENDS_INTERFACES | k::PERMITS => {
            shapes::clause_list(cx, node, &Java(cx), 2)
        }
        k::FORMAL_PARAMETER | k::SPREAD_PARAMETER => parameter(cx, node),
        k::RESOURCE_SPECIFICATION => resources(cx, node),
        k::FORMAL_PARAMETERS | k::RECORD_HEADER => hanging_list(cx, node, None),
        k::ARGUMENT_LIST | k::ANNOTATION_ARGUMENT_LIST => paren_list(cx, node),
        k::PACKAGE_DECLARATION => shapes::annotations_above(cx, node, &Java(cx)),
        k::TYPE_PARAMETERS | k::TYPE_ARGUMENTS => angle_list(cx, node, None),
        k::VARIABLE_DECLARATOR => declarator(cx, node),
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

fn modifiers(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    seq(cx, &kids(cx, node))
}

fn method(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = flat(cx, node);
    let mut parts = Vec::new();
    let mut rest = &els[..];
    if let Some(m) = els.first().filter(|e| e.kind() == k::MODIFIERS) {
        parts.push(shapes::modifiers(cx, m, Annotations::Lines, &Java(cx))?);
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
        let layout = m
            .as_node()
            .map_or(Annotations::Inline, |n| Annotations::of(cx, n));
        annotated = layout == Annotations::Hanging;
        parts.push(shapes::modifiers(cx, m, layout, &Java(cx))?);
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
    let call = shapes::CallStyle::of(cx.options);
    shapes::paren_list(cx, node, &Java(cx), &call, 2, extra).or_else(|_| generic(cx, node))
}

#[allow(unused_imports)]
use shapes as _shapes;

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
