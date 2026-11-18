//! The `wrapped` style of a member chain (Kotlin, ktlint's `chain-method-continuation`): the
//! accesses in front of the first call stay with the head, every later dot starts a line when the
//! chain breaks, and a call after a block or argument list that ends on a line of its own stays
//! on that line (`}.toList()`).

use super::{ChainHooks, ChainPolicy, Link, Walk, has_line_comment};
use crate::FormatError;
use crate::comments;
use crate::doc::{self, Doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

/// Does the source have a line break in front of this link's dot?
fn break_before(w: &Walk<'_>, link: &Link) -> bool {
    let Some(SyntaxElement::Token(dot)) = link.elems.first().filter(|_| link.dotted) else {
        return false;
    };
    let mut prev: Option<SyntaxToken> = dot.prev_token();
    while let Some(t) = prev.as_ref().filter(|t| w.cx.grammar.is_trivia(t.kind())) {
        prev = t.prev_token();
    }
    prev.is_some_and(|t| comments::break_after(w.cx.grammar, &t))
}

/// Does the element end in a closing parenthesis, brace or bracket?
fn ends_closed(cx: &crate::FormatCtx<'_>, e: &SyntaxElement) -> bool {
    let tok = match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.last_token(n),
    };
    tok.is_some_and(|t| matches!(t.text(), ")" | "}" | "]"))
}

pub(super) fn wrapped(
    w: &Walk<'_>,
    node: &SyntaxNode,
    head: &SyntaxElement,
    lead: Vec<Doc>,
    links: &[Link],
    policy: &ChainPolicy,
    _hooks: &dyn ChainHooks,
) -> Result<Doc, FormatError> {
    let cx = w.cx;
    let directives = cx.directives_of(node);
    let mut head_parts = lead;
    head_parts.push(w.element(head, false)?);
    // The accesses of a plain name path in front of the first dotted call stay with the head
    // (`kotlin.math.f()`); a head that is a call or has no dotted call has none.
    let first_dot = links.iter().position(|l| l.dotted).unwrap_or(links.len());
    let split = match links.iter().position(|l| l.dotted && l.call) {
        Some(call)
            if links[..call]
                .iter()
                .all(|l| l.dotted && !l.call && !l.index) =>
        {
            call
        }
        _ => first_dot,
    };
    let mut groups: Vec<Vec<Doc>> = Vec::new();
    // Does the group end in a block or list that spans lines in the source?
    let mut open_end: Vec<bool> = Vec::new();
    let mut starts: Vec<usize> = Vec::new();
    for (i, l) in links.iter().enumerate() {
        let d = w.link_doc(l, i + 1 == links.len(), false)?.0;
        if i < split {
            head_parts.push(d);
        } else if l.dotted || groups.is_empty() {
            groups.push(vec![d]);
            starts.push(i);
        } else if let Some(g) = groups.last_mut() {
            g.push(d);
        }
        if i >= split {
            let spans = l.elems.last().is_some_and(|e| ends_closed(cx, e));
            open_end.resize(groups.len(), false);
            open_end[groups.len() - 1] = spans;
        }
    }
    if groups.is_empty() {
        return Ok(doc::concat(head_parts));
    }
    let ops = links.iter().filter(|l| l.dotted).count();
    let calls = links.iter().filter(|l| l.call).count();
    let source_break = starts.iter().any(|&i| break_before(w, &links[i]));
    let force = has_line_comment(cx, links)
        || directives.has(&crate::Directive::BreakAll)
        || (policy.force_links > 0 && ops >= policy.force_links && calls > 0)
        || source_break;
    let keep = directives.has(&crate::Directive::KeepTogether);
    let head_ends = head_parts.last().is_some()
        && links.get(split.wrapping_sub(1)).map_or_else(
            || ends_closed(cx, head),
            |l| l.elems.last().is_some_and(|e| ends_closed(cx, e)),
        );
    let hugging = !has_line_comment(cx, links);
    let mut head = doc::concat(head_parts);
    let mut groups: Vec<Doc> = groups.into_iter().map(doc::concat).collect();
    let inner = !force
        && !keep
        && groups
            .split_last_mut()
            .is_some_and(|(_, init)| init.iter_mut().any(Doc::propagate_breaks));
    if keep || (!force && calls < policy.min_calls && !inner) {
        let mut parts = vec![head];
        parts.extend(groups);
        return Ok(doc::concat(parts));
    }
    // Both layouts share the parts only when the width decides.
    let width_decides = !(force || inner);
    if width_decides {
        head = doc::shared(head);
        groups = groups.into_iter().map(doc::shared).collect();
    }
    let flat = width_decides.then(|| {
        let mut parts = vec![head.clone()];
        parts.extend(groups.iter().cloned());
        doc::concat(parts)
    });
    // A group that starts after a block or list that ends on a line of its own joins that line
    // (a comment between the links breaks the group it follows: no joining then).
    let mut after = head_ends && hugging;
    let mut prev_id = after.then(doc::new_group_id);
    let mut base = Vec::with_capacity(groups.len() + 1);
    base.push(match prev_id {
        Some(id) => doc::group_with_id(id, head),
        None => head,
    });
    for (gi, g) in groups.into_iter().enumerate() {
        let next_after = open_end[gi] && hugging;
        let id = next_after.then(doc::new_group_id);
        let shown = match id {
            Some(id) => doc::group_with_id(id, g),
            None => g,
        };
        let separate = |shown: Doc| doc::indent(doc::concat(vec![doc::hardline(), shown]));
        base.push(match (after, prev_id, gi) {
            (true, Some(prev), 0) => doc::if_break_in(prev, shown.clone(), separate(shown)),
            (true, Some(prev), _) => doc::indent(doc::concat(vec![
                doc::if_break_in(prev, Doc::Nil, doc::hardline()),
                shown,
            ])),
            _ => separate(shown),
        });
        after = next_after;
        prev_id = id;
    }
    let broken = doc::concat(base);
    Ok(match flat {
        Some(flat) => doc::conditional_group(vec![flat, broken]),
        None => broken,
    })
}
