//! Whether a space stands between two neighbouring tokens: the part of the layout that needs no
//! structure, only the token texts and the nodes the tokens belong to.

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, SyntaxToken};

fn parent_kind(t: &SyntaxToken) -> Option<SyntaxKind> {
    t.parent().map(|n| n.kind())
}

/// `<` or `>` of a type argument or type parameter list (the others compare).
fn is_angle(t: &SyntaxToken) -> bool {
    matches!(t.kind(), k::LT | k::GT)
        && matches!(parent_kind(t), Some(k::TYPE_ARGUMENTS | k::TYPE_PARAMETERS))
}

pub fn between(p: &SyntaxToken, n: &SyntaxToken) -> bool {
    let (pt, nt) = (p.text(), n.text());
    if nt == "..." && matches!(parent_kind(p), Some(k::MARKER_ANNOTATION | k::ANNOTATION)) {
        return true;
    }
    if matches!(nt, ";" | "," | ")" | "]" | "." | "::" | "...") {
        // `for (; ; )`
        return (nt == ";" || nt == ")") && pt == ";" && parent_kind(n) == Some(k::FOR_SPEC);
    }
    if matches!(pt, "(" | "[" | "." | "::" | "@") {
        return false;
    }
    if pt == ")" && nt == "(" {
        return true;
    }
    if nt == "[" {
        // `char @Nullable [] a`
        return matches!(parent_kind(p), Some(k::MARKER_ANNOTATION | k::ANNOTATION));
    }
    if nt == "<>" {
        return false;
    }
    if is_angle(n) {
        return n.kind() == k::LT
            && parent_kind(n) == Some(k::TYPE_PARAMETERS)
            && p.kind() != k::IDENTIFIER;
    }
    if is_angle(p) {
        if p.kind() == k::LT {
            return false;
        }
        // `Foo.<T>bar()`
        let call = p.parent().and_then(|t| t.parent()).map(|g| g.kind());
        return nt != "("
            && !matches!(
                call,
                Some(k::METHOD_INVOCATION | k::EXPLICIT_CONSTRUCTOR_INVOCATION)
            );
    }
    if pt == "<>" {
        return nt == "{";
    }
    match nt {
        "(" => {
            return !matches!(p.kind(), k::IDENTIFIER) && !matches!(pt, "this" | "super");
        }
        "[" => return false,
        "{" => return !matches!(pt, "{" | "("),
        ":" => {
            return matches!(
                parent_kind(n),
                Some(k::FOR_SPEC | k::TERNARY_EXPRESSION | k::ASSERT_STATEMENT)
            );
        }
        "}" => return false,
        _ => {}
    }
    pt != "{"
}
