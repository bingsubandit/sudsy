//! Quoted annotations (`x: "Foo"`, `List["Foo"]`, `cast("Foo", x)`): the string is Python
//! code read later, parsed with the Python grammar, and the names in it are references from the
//! scope the string is written in.

use crate::adapter::parse_snippet;
use crate::generated::kind as k;
use crate::semantic::{Cx, last_name};
use crate::text::parts;
use core_semantics::scopes::ScopeId;
use core_syntax::{SyntaxNode, TextRange, TextSize};

/// Is the string a value, not a type: an item of `Literal[...]`, or a note of `Annotated[T, ...]`?
fn is_value(string: &SyntaxNode) -> bool {
    let mut child = string.clone();
    for anc in string.ancestors().skip(1) {
        match anc.kind() {
            k::PARENTHESIZED => child = anc,
            k::EXPR_TUPLE => {
                if anc.parent().is_some_and(|p| p.kind() == k::EXPR_SUBSCRIPT) {
                    child = anc;
                    continue;
                }
                return false;
            }
            k::EXPR_SUBSCRIPT => {
                let object = anc.first_child();
                let name = object.and_then(|o| last_name(&o));
                return match name.as_deref() {
                    Some("Literal") => true,
                    Some("Annotated") => {
                        let slice = anc.children().nth(1);
                        match slice {
                            Some(s) if s.kind() == k::EXPR_TUPLE => {
                                s.children().next().is_none_or(|first| first != child)
                            }
                            _ => false,
                        }
                    }
                    _ => false,
                };
            }
            _ => return false,
        }
    }
    false
}

/// Adds the references of the names in the quoted annotation `string`, or notes why it is no
/// annotation.
pub fn references(cx: &mut Cx, string: &SyntaxNode, scope: ScopeId) {
    if is_value(string) {
        return;
    }
    let pieces = parts(string);
    let range = cx.range(string);
    let [piece] = pieces.as_slice() else {
        if pieces.len() > 1 {
            let text = crate::text::code_text(string);
            cx.forward_errors.push((range, text));
        }
        return;
    };
    if piece.bytes || piece.inner.contains('\\') {
        return;
    }
    let parsed = parse_snippet(&piece.inner);
    let body: Vec<SyntaxNode> = parsed.tree.children().collect();
    let [stmt] = body.as_slice() else {
        return fail(cx, range, &piece.inner);
    };
    if !parsed.errors.is_empty() || stmt.kind() != k::STMT_EXPR {
        return fail(cx, range, &piece.inner);
    }
    for name in parsed
        .tree
        .descendants()
        .filter(|n| n.kind() == k::EXPR_NAME)
    {
        if in_literal(&name) {
            continue;
        }
        let Some(token) = name
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == k::NAME)
        else {
            continue;
        };
        let at = token.text_range();
        let shift = |o: TextSize| o + piece.inner_start;
        let range = TextRange::new(shift(at.start()), shift(at.end()));
        cx.add_reference(token.text(), range, string, scope);
    }
}

fn fail(cx: &mut Cx, range: TextRange, text: &str) {
    cx.forward_errors.push((range, text.to_owned()));
}

/// Is the name inside the slice of a `Literal[...]` of the quoted annotation?
fn in_literal(name: &SyntaxNode) -> bool {
    name.ancestors().skip(1).any(|a| {
        a.kind() == k::EXPR_SUBSCRIPT
            && a.first_child()
                .is_some_and(|o| o != *name && last_name(&o).as_deref() == Some("Literal"))
            && a.first_child()
                .is_some_and(|o| !o.text_range().contains_range(name.text_range()))
    })
}
