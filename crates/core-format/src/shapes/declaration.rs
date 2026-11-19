//! Declarations, read from grammar tags: the annotations in front of one (`#modifiers` holding
//! `#annotation` nodes), a type's header with its heritage clauses (`#heritage`, the body a
//! `#block`), the members of a body (kinds in `blank_compact_kinds` need no blank line between them) and an
//! enum body (`#enum-constant`, `#separator`, the `;` a `#stray`, the members after it an
//! `#enum-tail`). The language prints the elements (a [`Printer`]).
//!
//! concept: declaration-kit
//! what: Annotations, type headers with heritage clauses, member sequences and enum bodies of declarations.
//! tools: google-java-format declarations and annotations, ktlint annotation and class-signature, Prettier class bodies
//! reads: tag:annotation, tag:heritage, option:type_use_annotations, option:member_attribute_line

use super::{BlockStyle, braces, statement_sequence_with};
use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc, GroupId};
use crate::error::FormatError;
use crate::seq::{first, flat, kids, last};
use core_config::OptionValue;
use core_syntax::{SyntaxElement, SyntaxNode, TextRange};

type R = Result<Doc, FormatError>;

/// How a language prints the elements of a declaration.
pub trait Printer {
    /// One element.
    fn el(&self, e: &SyntaxElement) -> R;
    /// The elements one after the other with the language's spacing, `doc` printing each.
    fn seq_with(&self, els: &[SyntaxElement], doc: &dyn Fn(&SyntaxElement) -> R) -> R;
    fn seq(&self, els: &[SyntaxElement]) -> R {
        self.seq_with(els, &|e| self.el(e))
    }
}

/// Where the annotations of a declaration stand.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Annotations {
    /// Each on a line of its own, the other modifiers on the line of the declaration.
    Lines,
    /// On their own line when the declaration does not fit on one, else in front of it.
    Hanging,
    /// With the other modifiers, in front of the declaration.
    Inline,
}

impl Annotations {
    /// A declaration whose annotations have arguments puts each on a line; one that starts with
    /// an annotation lets them hang; any other keeps them inline.
    pub fn of(cx: &FormatCtx<'_>, modifiers: &SyntaxNode) -> Annotations {
        let g = cx.grammar;
        let els = kids(cx, modifiers);
        if els.iter().any(|a| has_arguments(cx, a)) {
            Annotations::Lines
        } else if els
            .first()
            .is_some_and(|a| g.has_tag(a.kind(), "annotation"))
        {
            Annotations::Hanging
        } else {
            Annotations::Inline
        }
    }
}

/// The modifiers of a declaration, the annotations in front laid out as `layout` says.
pub fn modifiers(
    cx: &FormatCtx<'_>,
    modifiers: &SyntaxElement,
    layout: Annotations,
    p: &dyn Printer,
) -> R {
    let Some(node) = modifiers.as_node() else {
        return p.el(modifiers);
    };
    let els = kids(cx, node);
    let mut lead = els
        .iter()
        .take_while(|e| cx.grammar.has_tag(e.kind(), "annotation"))
        .count();
    while lead > 0 && is_type_use(cx, &els[lead - 1], node) {
        lead -= 1;
    }
    let rest = |parts: &mut Vec<Doc>| -> Result<(), FormatError> {
        if lead < els.len() {
            parts.extend([p.seq(&els[lead..])?, doc::lit(" ")]);
        }
        Ok(())
    };
    let mut parts = Vec::new();
    match layout {
        Annotations::Lines => {
            for e in &els[..lead] {
                parts.extend([p.el(e)?, doc::hardline()]);
            }
            rest(&mut parts)?;
        }
        Annotations::Hanging if lead > 0 => {
            parts.extend([p.seq(&els[..lead])?, doc::line()]);
            rest(&mut parts)?;
        }
        _ => parts.extend([p.seq(&els)?, doc::lit(" ")]),
    }
    Ok(doc::concat(parts))
}

/// Does the annotation (without arguments) belong to the type that follows, not to the
/// declaration: the file imports its name as one of the option `type_use_annotations`?
fn is_type_use(cx: &FormatCtx<'_>, annotation: &SyntaxElement, at: &SyntaxNode) -> bool {
    let g = cx.grammar;
    let Some(OptionValue::List(items)) = cx.options.get("type_use_annotations") else {
        return false;
    };
    let Some(a) = annotation.as_node().filter(|a| {
        !a.children()
            .any(|c| g.has_tag(c.kind(), "annotation-arguments"))
    }) else {
        return false;
    };
    let name = a
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| {
            !t.text().is_empty() && t.text().chars().all(|c| c.is_alphanumeric() || c == '_')
        })
        .last()
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    let patterns: Vec<&str> = items
        .iter()
        .filter_map(|i| match i {
            OptionValue::String(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    let Some(root) = at.ancestors().last() else {
        return false;
    };
    patterns.iter().any(|p| p.ends_with(&name))
        && root.descendants().any(|n| {
            g.has_tag(n.kind(), "import")
                && g.slot(&n, "path").is_some_and(|path| {
                    let path: String = path.to_string().split_whitespace().collect();
                    path.rsplit('.').next() == Some(&name)
                        && patterns.iter().any(|p| glob(p, &path))
                })
        })
}

/// `pattern` matches `text`, `*` standing for any text.
fn glob(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, rest)) => text.strip_prefix(head).is_some_and(|t| {
            (0..=t.len())
                .filter(|&i| t.is_char_boundary(i))
                .any(|i| glob(rest, &t[i..]))
        }),
    }
}

/// The `#annotation` elements at the start of `els` (decorators, annotations), each followed by a
/// line break, printed by `print`; and how many there are.
pub fn lead_annotations(
    cx: &FormatCtx<'_>,
    els: &[SyntaxElement],
    print: &dyn Fn(&SyntaxElement) -> R,
) -> Result<(Doc, usize), FormatError> {
    let lead = els
        .iter()
        .take_while(|e| cx.grammar.has_tag(e.kind(), "annotation"))
        .count();
    // With the option `member_attribute_line: keep` an annotation the source leaves on the line
    // of what follows it stays there.
    let keep = cx.options.is("member_attribute_line", "keep");
    let mut parts = Vec::new();
    for (i, e) in els[..lead].iter().enumerate() {
        let same_line = keep
            && matches!((last(cx, e), els.get(i + 1).and_then(|n| first(cx, n))), (Some(a), Some(b))
            if !cx.has_line_break(
                &a.parent().unwrap_or_else(|| unreachable!("a token has a parent")),
                TextRange::new(a.text_range().end(), b.text_range().start()),
            ));
        parts.extend([
            print(e)?,
            if same_line {
                doc::lit(" ")
            } else {
                doc::hardline()
            },
        ]);
    }
    Ok((doc::concat(parts), lead))
}

/// Annotations on lines of their own in front of what the node continues with (a package).
pub fn annotations_above(cx: &FormatCtx<'_>, node: &SyntaxNode, p: &dyn Printer) -> R {
    let els = kids(cx, node);
    let (lead, n) = lead_annotations(cx, &els, &|e| p.el(e))?;
    Ok(doc::concat(vec![lead, p.seq(&els[n..])?]))
}

/// An annotation with arguments (`@A()` has none): a node tagged `#annotation-arguments` with
/// something between its brackets.
pub fn has_arguments(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    e.as_node().is_some_and(|n| {
        n.children()
            .any(|l| cx.grammar.has_tag(l.kind(), "annotation-arguments") && kids(cx, &l).len() > 2)
    })
}

/// A type declaration: `modifiers`, the head (name, type parameters, `head_el` prints each of its
/// elements and gets the group that tells whether the type parameters broke), the heritage
/// clauses and the body. The heritage clauses stay on the line while they fit, and otherwise
/// break before their keyword, the first one on a line of its own when the type parameters
/// broke; the lines after the first are `continuation` levels in.
pub fn type_declaration(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    p: &dyn Printer,
    head_el: &dyn Fn(&SyntaxElement, GroupId) -> R,
    continuation: usize,
) -> R {
    let g = cx.grammar;
    let els = flat(cx, node);
    let at = els.iter().position(|e| g.has_tag(e.kind(), "block"));
    let (head, body) = els.split_at(at.unwrap_or(els.len()));
    let mut parts = Vec::new();
    let mut rest = head;
    if let Some(m) = head.first().filter(|e| g.has_tag(e.kind(), "modifiers")) {
        parts.push(modifiers(cx, m, Annotations::Lines, p)?);
        rest = &head[1..];
    }
    let split = rest
        .iter()
        .position(|e| g.has_tag(e.kind(), "heritage"))
        .unwrap_or(rest.len());
    let id = doc::new_group_id();
    let head = p.seq_with(&rest[..split], &|e| head_el(e, id))?;
    let mut clauses = Vec::new();
    for e in &rest[split..] {
        clauses.push((doc::line(), p.el(e)?));
    }
    let style = HeaderStyle {
        grouped: true,
        continuation,
        first_on_own_line: Some(id),
        ..HeaderStyle::default()
    };
    parts.push(header(head, clauses, &style));
    if let Some(b) = body.first() {
        parts.extend([doc::lit(" "), p.el(b)?]);
    }
    Ok(doc::concat(parts))
}

/// A heritage clause: what separates it from the head or the clause before it, and its text.
pub type Clause = (Doc, Doc);

#[derive(Default)]
pub struct HeaderStyle {
    /// Each clause with its separator is a group of its own (it moves to the next line alone).
    pub grouped: bool,
    /// Indentation levels of the clauses when they break.
    pub continuation: usize,
    /// The group of the whole header, for what asks whether it broke.
    pub id: Option<GroupId>,
    /// The first clause stands on a line of its own when this group (the type parameters) broke.
    pub first_on_own_line: Option<GroupId>,
}

/// A type's head and its heritage clauses, one group: the clauses are indented, so a head that
/// breaks (type parameters) keeps its own indentation.
pub fn header(head: Doc, clauses: Vec<Clause>, style: &HeaderStyle) -> Doc {
    let tail = doc::concat(
        clauses
            .into_iter()
            .enumerate()
            .map(|(i, (sep, clause))| {
                let clause = doc::shared(clause);
                let d = doc::concat(vec![sep, clause.clone()]);
                let d = if style.grouped { doc::group(d) } else { d };
                match style.first_on_own_line.filter(|_| i == 0) {
                    Some(id) => doc::if_break_in(id, doc::concat(vec![doc::hardline(), clause]), d),
                    None => d,
                }
            })
            .collect(),
    );
    let tail = (0..style.continuation).fold(tail, |d, _| doc::indent(d));
    let d = doc::concat(vec![head, tail]);
    match style.id {
        Some(id) => doc::group_with_id(id, d),
        None => doc::group(d),
    }
}

/// The members of a body, a blank line between two where the `[blank]` rows say so and where
/// the source has one.
pub fn members(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    super::block(cx, node, &member_style())
}

fn member_style() -> BlockStyle {
    BlockStyle {
        force_blank: Some(|cx, p, n| cx.blank_between(p, n)),
        keep_leading_blank: true,
        ..BlockStyle::default()
    }
}

/// Does a `/**` comment stand in front of the node?
pub fn has_doc_comment(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    cx.first_token(n).is_some_and(|t| {
        comments::leading_trivia(cx.grammar, &t)
            .iter()
            .any(|x| cx.grammar.has_tag(x.kind(), "comment") && x.to_string().starts_with("/**"))
    })
}

/// Does the declaration have an annotation (with arguments: `args`) among its modifiers?
pub fn annotated(cx: &FormatCtx<'_>, n: &SyntaxNode, args: bool) -> bool {
    kids(cx, n).iter().any(|m| {
        m.as_node().is_some_and(|m| {
            cx.grammar.has_tag(m.kind(), "modifiers")
                && kids(cx, m).iter().any(|a| {
                    if args {
                        has_arguments(cx, a)
                    } else {
                        cx.grammar.has_tag(a.kind(), "annotation")
                    }
                })
        })
    })
}

/// An enum body: the constants one per line (a blank where the source has one), the `;` that
/// ends them, then the members after a blank line.
pub fn enum_body(cx: &FormatCtx<'_>, node: &SyntaxNode, p: &dyn Printer) -> R {
    let g = cx.grammar;
    let els = kids(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (els.first().cloned(), els.last().cloned())
    else {
        return p.seq(&els);
    };
    let inner = &els[1..els.len() - 1];
    let is = |e: &SyntaxElement, tag: &str| g.has_tag(e.kind(), tag);
    let consts: Vec<_> = inner.iter().filter(|e| is(e, "enum-constant")).collect();
    let decls = inner.iter().find(|e| is(e, "enum-tail"));
    let commas: Vec<_> = inner.iter().filter(|e| is(e, "separator")).collect();
    let mut parts = Vec::new();
    for (i, c) in consts.iter().enumerate() {
        if i > 0 {
            parts.extend([p.el(commas[i - 1])?, doc::hardline()]);
            if c.as_node().is_some_and(|n| super::blank_before(cx, n)) {
                parts.push(doc::hardline());
            }
        }
        parts.push(p.el(c)?);
    }
    let comma_last = inner
        .iter()
        .rev()
        .find(|e| !is(e, "enum-tail") && !is(e, "stray"))
        .is_some_and(|e| is(e, "separator"));
    if comma_last {
        parts.push(p.el(commas[consts.len() - 1])?);
    }
    let semi_el = inner.iter().find(|e| is(e, "stray")).cloned().or_else(|| {
        decls
            .and_then(|d| d.as_node())
            .and_then(|d| kids(cx, d).into_iter().find(|e| is(e, "stray")))
    });
    let semi_doc = || match &semi_el {
        Some(e) => p.el(e),
        None => Ok(doc::text(";")),
    };
    let members: Vec<SyntaxNode> = decls
        .and_then(|d| d.as_node())
        .map(|d| d.children().collect())
        .unwrap_or_default();
    let tail_kids = decls
        .and_then(|d| d.as_node())
        .map_or(0, |d| kids(cx, d).len());
    if (decls.is_some() || semi_el.is_some()) && tail_kids <= 1 {
        if comma_last {
            parts.push(doc::hardline());
        }
        parts.push(semi_doc()?);
    }
    if !members.is_empty() {
        if comma_last {
            parts.push(doc::hardline());
        }
        parts.extend([
            semi_doc()?,
            doc::hardline(),
            doc::hardline(),
            statement_sequence_with(
                cx,
                &members,
                1,
                Some(|cx, p, n| cx.blank_between(p, n)),
                false,
            )?,
        ]);
    }
    let style = BlockStyle::default();
    if parts.is_empty() {
        return Ok(braces(cx, &open, &close, None, false, &style));
    }
    let lead_blank = consts
        .first()
        .and_then(|c| c.as_node())
        .is_some_and(|c| has_doc_comment(cx, c) && super::blank_before(cx, c));
    Ok(braces(
        cx,
        &open,
        &close,
        Some(doc::concat(parts)),
        lead_blank,
        &style,
    ))
}
