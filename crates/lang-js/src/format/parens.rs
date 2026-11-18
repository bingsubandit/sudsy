//! Parentheses: which a node needs (Prettier's `needsParens`), on top of the shared parentheses
//! shape (`core_format::shapes::parens`). The tree keeps `ParenthesizedExpression` (`#transparent`
//! for the safety check): a kept pair is printed as written, an unneeded pair is left out, and a
//! node that needs a pair the source lacks gets one around its printed document.
//!
//! concept: parenthesized-expression

use super::ast::{is_member, operator_text, outer, parent, starts_with, un};
use crate::generated::kind as k;
use core_format::doc::Doc;
use core_format::shapes::{
    self, ParenHooks, ParenLayout, ParenPolicy, Unneeded, binary_needs, is_dropped_close as dropped,
};
use core_format::{FormatCtx, FormatError};
use core_syntax::roles::has;
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

pub(crate) use shapes::Need;

const POLICY: ParenPolicy = ParenPolicy {
    layout: ParenLayout::Flat,
    unneeded: Unneeded::Drop,
};

struct Hooks;

impl ParenHooks for Hooks {
    fn needs(&self, cx: &FormatCtx<'_>, node: &SyntaxNode) -> Need {
        needs_parens(cx, node)
    }

    /// A type cast (`/** @type {T} */ (x)`) ties the pair to its expression.
    fn tied(&self, cx: &FormatCtx<'_>, open: &SyntaxToken) -> bool {
        core_format::comments::leading_comments(cx.grammar, open)
            .last()
            .is_some_and(|c| {
                let t = c.text();
                t.starts_with("/**") && (t.contains("@type") || t.contains("@satisfies"))
            })
    }

    fn addable(&self, cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
        match node.kind() {
            k::IDENTIFIER_REFERENCE => is_let(cx, node),
            kind => addable(kind),
        }
    }
}

/// The kinds that get parentheses when they need them and the source has none.
fn addable(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::BINARY_EXPRESSION
            | k::LOGICAL_EXPRESSION
            | k::UNARY_EXPRESSION
            | k::UPDATE_EXPRESSION
            | k::AWAIT_EXPRESSION
            | k::YIELD_EXPRESSION
            | k::SEQUENCE_EXPRESSION
            | k::ASSIGNMENT_EXPRESSION
            | k::CONDITIONAL_EXPRESSION
            | k::ARROW_FUNCTION_EXPRESSION
            | k::FUNCTION
            | k::CLASS
            | k::OBJECT_EXPRESSION
            | k::NUMERIC_LITERAL
            | k::STRING_LITERAL
            | k::CHAIN_EXPRESSION
            | k::CALL_EXPRESSION
            | k::STATIC_MEMBER_EXPRESSION
            | k::COMPUTED_MEMBER_EXPRESSION
            | k::TAGGED_TEMPLATE_EXPRESSION
            | k::TS_NON_NULL_EXPRESSION
            | k::JSX_ELEMENT
            | k::JSX_FRAGMENT
            | k::TS_AS_EXPRESSION
            | k::TS_SATISFIES_EXPRESSION
            | k::TS_TYPE_ASSERTION
    )
}

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if node.kind() != k::PARENTHESIZED_EXPRESSION {
        return Ok(None);
    }
    shapes::parenthesized(cx, node, &POLICY, &Hooks).map(Some)
}

/// Is `close` the `)` of a pair that is left out of the output?
pub(crate) fn is_dropped_close(cx: &FormatCtx<'_>, close: &SyntaxToken) -> bool {
    dropped(cx, close, &POLICY, &Hooks)
}

/// `doc` of `node` in the parentheses it needs when the source has none.
pub(crate) fn wrap(node: &SyntaxNode, cx: &FormatCtx<'_>, doc: Doc) -> Doc {
    shapes::wrap(cx, node, doc, &POLICY, &Hooks)
}

/// The first character the printed `node` starts with (a kept or added parenthesis counts).
pub(crate) fn first_char(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<char> {
    shapes::first_char(cx, node, &POLICY, &Hooks)
}

/// The kinds that are declarations when they stand where a statement does.
fn declaration_position(pk: SyntaxKind) -> bool {
    matches!(
        pk,
        k::PROGRAM
            | k::BLOCK_STATEMENT
            | k::FUNCTION_BODY
            | k::SWITCH_CASE
            | k::STATIC_BLOCK
            | k::TS_MODULE_BLOCK
            | k::EXPORT_DECLARATION
            | k::EXPORT_NAMED_DECLARATION
            | k::EXPORT_DEFAULT_DECLARATION
            | k::LABELED_STATEMENT
            | k::IF_STATEMENT
            | k::FOR_STATEMENT
            | k::FOR_IN_STATEMENT
            | k::FOR_OF_STATEMENT
            | k::WHILE_STATEMENT
            | k::DO_WHILE_STATEMENT
            | k::WITH_STATEMENT
            | k::METHOD_DEFINITION
            | k::OBJECT_PROPERTY
            | k::TS_INTERFACE_BODY
            | k::TS_EXTERNAL_MODULE_DECLARATION
    )
}

/// Does `node` need parentheses where its parent holds it? (`node` is not a parenthesized
/// expression; parentheses around it do not count.)
pub(crate) fn needs_parens(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Need {
    let Some(p) = parent(node) else {
        return Need::No;
    };
    let c = outer(node);
    let (kind, pk) = (node.kind(), p.kind());
    if pk == k::DECORATOR {
        return Need::Keep;
    }
    let yes = |b: bool| if b { Need::Yes } else { Need::No };
    let first = p.first_child().as_ref() == Some(&c);
    let last = p.last_child().as_ref() == Some(&c);
    let member_p = is_member(pk);
    let is_callee = matches!(pk, k::CALL_EXPRESSION | k::NEW_EXPRESSION) && first;
    let is_object = member_p && first;
    let super_class = pk == k::CLASS
        && std::iter::successors(c.prev_sibling_or_token(), |e| e.prev_sibling_or_token())
            .find(|e| !cx.grammar.is_trivia(e.kind()))
            .is_some_and(|e| e.as_token().is_some_and(|t| t.text() == "extends"));

    // A leading `{`, `function` or `class` of a statement, `{` of an arrow body or the
    // declaration of `export default` would be read as something else.
    if matches!(kind, k::OBJECT_EXPRESSION | k::FUNCTION | k::CLASS)
        && (c != *node || !declaration_position(pk))
    {
        let at = |n: &SyntaxNode| *n == *node;
        let statement = node
            .ancestors()
            .find(|a| a.kind() == k::EXPRESSION_STATEMENT);
        if statement
            .and_then(|s| s.children().next())
            .is_some_and(|e| starts_with(&e, &at))
        {
            return Need::Yes;
        }
        if kind == k::OBJECT_EXPRESSION
            && let Some(body) = node
                .ancestors()
                .find(|a| a.kind() == k::ARROW_FUNCTION_EXPRESSION)
                .and_then(|a| a.children().last())
            && un(&body).kind() != k::SEQUENCE_EXPRESSION
            && starts_with(&body, &at)
        {
            return Need::Yes;
        }
        let export = node
            .ancestors()
            .find(|a| a.kind() == k::EXPORT_DEFAULT_DECLARATION)
            .and_then(|a| a.children().last());
        // (a function or class that starts a longer expression leaves the parentheses to it)
        if kind != k::OBJECT_EXPRESSION && export.as_ref().is_some_and(|e| starts_with(e, &at)) {
            return yes(export.is_some_and(|e| un(&e) == *node));
        }
    }
    if super_class
        && matches!(
            kind,
            k::ARROW_FUNCTION_EXPRESSION
                | k::ASSIGNMENT_EXPRESSION
                | k::AWAIT_EXPRESSION
                | k::BINARY_EXPRESSION
                | k::CONDITIONAL_EXPRESSION
                | k::LOGICAL_EXPRESSION
                | k::NEW_EXPRESSION
                | k::OBJECT_EXPRESSION
                | k::SEQUENCE_EXPRESSION
                | k::TAGGED_TEMPLATE_EXPRESSION
                | k::UNARY_EXPRESSION
                | k::UPDATE_EXPRESSION
                | k::YIELD_EXPRESSION
                | k::TS_NON_NULL_EXPRESSION
        )
    {
        return Need::Yes;
    }
    if pk == k::EXPORT_DEFAULT_DECLARATION
        && (kind == k::SEQUENCE_EXPRESSION
            || (!matches!(kind, k::FUNCTION | k::CLASS | k::TAGGED_TEMPLATE_EXPRESSION)
                && starts_with(node, &|n| matches!(n.kind(), k::FUNCTION | k::CLASS))))
    {
        return Need::Yes;
    }
    let is_cast = |kd: SyntaxKind| matches!(kd, k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION);
    match kind {
        k::UPDATE_EXPRESSION | k::UNARY_EXPRESSION => {
            let op = operator_text(cx, node);
            if kind == k::UPDATE_EXPRESSION && pk == k::UNARY_EXPRESSION {
                let prefix = node
                    .children_with_tokens()
                    .next()
                    .is_some_and(|e| e.as_token().is_some());
                let pop = operator_text(cx, &p);
                return yes(prefix && ((op == "++" && pop == "+") || (op == "--" && pop == "-")));
            }
            match pk {
                k::UNARY_EXPRESSION => yes(op == operator_text(cx, &p) && (op == "+" || op == "-")),
                k::TAGGED_TEMPLATE_EXPRESSION | k::TS_NON_NULL_EXPRESSION => Need::Yes,
                _ if member_p => yes(is_object),
                k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
                k::BINARY_EXPRESSION => yes(first && operator_text(cx, &p) == "**"),
                _ => Need::No,
            }
        }
        k::BINARY_EXPRESSION
        | k::LOGICAL_EXPRESSION
        | k::TS_AS_EXPRESSION
        | k::TS_SATISFIES_EXPRESSION
        | k::TS_TYPE_ASSERTION => {
            let op = operator_text(cx, node);
            if kind == k::BINARY_EXPRESSION {
                if pk == k::UPDATE_EXPRESSION {
                    return Need::Yes;
                }
                if op == "in" && in_for_init(node) {
                    return Need::Yes;
                }
            }
            match pk {
                k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION => yes(!is_cast(kind)),
                // The branches of a conditional wrap a cast and `??`.
                k::CONDITIONAL_EXPRESSION if first => {
                    if is_cast(kind) {
                        Need::Keep
                    } else {
                        yes(kind == k::LOGICAL_EXPRESSION && op == "??")
                    }
                }
                k::CONDITIONAL_EXPRESSION => {
                    yes(is_cast(kind) || (kind == k::LOGICAL_EXPRESSION && op == "??"))
                }
                k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
                k::CLASS => yes(super_class),
                k::TS_TYPE_ASSERTION
                | k::TAGGED_TEMPLATE_EXPRESSION
                | k::UNARY_EXPRESSION
                | k::JSX_SPREAD_ATTRIBUTE
                | k::SPREAD_ELEMENT
                | k::AWAIT_EXPRESSION
                | k::TS_NON_NULL_EXPRESSION
                | k::UPDATE_EXPRESSION => Need::Yes,
                _ if member_p => yes(is_object),
                k::ASSIGNMENT_EXPRESSION => {
                    yes(first && (kind == k::TS_TYPE_ASSERTION || is_cast(kind)))
                }
                k::LOGICAL_EXPRESSION | k::BINARY_EXPRESSION => {
                    if kind == k::TS_TYPE_ASSERTION {
                        return Need::Keep;
                    }
                    if is_cast(kind) || !has(kind, "binary") {
                        return Need::Yes;
                    }
                    let pop = operator_text(cx, &p);
                    if pk == k::LOGICAL_EXPRESSION && kind == k::LOGICAL_EXPRESSION {
                        return yes(pop != op);
                    }
                    binary_needs(cx.grammar, &pop, &op, first, true)
                }
                _ => Need::No,
            }
        }
        k::SEQUENCE_EXPRESSION => match pk {
            k::FOR_STATEMENT | k::RETURN_STATEMENT | k::THROW_STATEMENT => Need::No,
            k::ARROW_FUNCTION_EXPRESSION => yes(!last),
            _ => Need::Yes,
        },
        k::YIELD_EXPRESSION | k::AWAIT_EXPRESSION => {
            if kind == k::YIELD_EXPRESSION
                && matches!(pk, k::AWAIT_EXPRESSION | k::TS_TYPE_ASSERTION)
            {
                return Need::Yes;
            }
            match pk {
                k::TAGGED_TEMPLATE_EXPRESSION
                | k::UNARY_EXPRESSION
                | k::LOGICAL_EXPRESSION
                | k::SPREAD_ELEMENT
                | k::TS_AS_EXPRESSION
                | k::TS_SATISFIES_EXPRESSION
                | k::TS_NON_NULL_EXPRESSION
                | k::TS_INSTANTIATION_EXPRESSION
                | k::BINARY_EXPRESSION => Need::Yes,
                _ if member_p => yes(is_object),
                k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
                k::CONDITIONAL_EXPRESSION => yes(first),
                _ => Need::No,
            }
        }
        k::NUMERIC_LITERAL => yes(is_object),
        k::STRING_LITERAL => yes(pk == k::EXPRESSION_STATEMENT
            && p.parent().is_some_and(|g| {
                matches!(g.kind(), k::PROGRAM | k::BLOCK_STATEMENT | k::FUNCTION_BODY)
            })),
        k::ASSIGNMENT_EXPRESSION => match pk {
            k::ARROW_FUNCTION_EXPRESSION if last => Need::Yes,
            k::EXPRESSION_STATEMENT => yes(node
                .children()
                .next()
                .is_some_and(|l| l.kind() == k::OBJECT_ASSIGNMENT_TARGET)),
            k::FOR_STATEMENT | k::ASSIGNMENT_EXPRESSION => Need::No,
            k::ARRAY_EXPRESSION if super::embed::is_root(cx, node) => Need::No,
            k::SEQUENCE_EXPRESSION if p.parent().is_some_and(|g| g.kind() == k::FOR_STATEMENT) => {
                Need::No
            }
            _ => Need::Yes,
        },
        k::CONDITIONAL_EXPRESSION => match pk {
            k::TAGGED_TEMPLATE_EXPRESSION
            | k::UNARY_EXPRESSION
            | k::SPREAD_ELEMENT
            | k::BINARY_EXPRESSION
            | k::LOGICAL_EXPRESSION
            | k::AWAIT_EXPRESSION
            | k::JSX_SPREAD_ATTRIBUTE
            | k::TS_TYPE_ASSERTION
            | k::TS_AS_EXPRESSION
            | k::TS_SATISFIES_EXPRESSION
            | k::TS_NON_NULL_EXPRESSION => Need::Yes,
            k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
            k::CONDITIONAL_EXPRESSION => yes(first),
            _ if member_p => yes(is_object),
            _ => Need::No,
        },
        k::FUNCTION => match pk {
            k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
            k::TAGGED_TEMPLATE_EXPRESSION => Need::Yes,
            _ => Need::No,
        },
        k::ARROW_FUNCTION_EXPRESSION => match pk {
            k::BINARY_EXPRESSION
            | k::TS_AS_EXPRESSION
            | k::TS_SATISFIES_EXPRESSION
            | k::TS_NON_NULL_EXPRESSION
            | k::TAGGED_TEMPLATE_EXPRESSION
            | k::UNARY_EXPRESSION
            | k::LOGICAL_EXPRESSION
            | k::AWAIT_EXPRESSION
            | k::TS_INSTANTIATION_EXPRESSION
            | k::TS_TYPE_ASSERTION => Need::Yes,
            k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
            k::CONDITIONAL_EXPRESSION => yes(first),
            _ if member_p => yes(is_object),
            _ => Need::No,
        },
        k::CLASS => {
            let decorated = node
                .children()
                .next()
                .is_some_and(|c| c.kind() == k::DECORATOR);
            yes((decorated && !declaration_position(pk)) || (pk == k::NEW_EXPRESSION && first))
        }
        k::OBJECT_EXPRESSION => Need::No,
        k::CHAIN_EXPRESSION => match pk {
            k::TS_NON_NULL_EXPRESSION => Need::Keep,
            k::TAGGED_TEMPLATE_EXPRESSION => Need::Yes,
            // `(a?.b)?.c` is the one chain `a?.b?.c`
            k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee && !optional_link(&p)),
            _ if member_p => yes(is_object && !optional_link(&p)),
            _ => Need::No,
        },
        k::CALL_EXPRESSION
        | k::STATIC_MEMBER_EXPRESSION
        | k::COMPUTED_MEMBER_EXPRESSION
        | k::PRIVATE_FIELD_EXPRESSION
        | k::TAGGED_TEMPLATE_EXPRESSION
        | k::TS_NON_NULL_EXPRESSION => {
            yes(pk == k::NEW_EXPRESSION && first && has_call_in_object_chain(node))
        }
        k::JSX_ELEMENT | k::JSX_FRAGMENT => {
            // oxc does not read `<></>.x`, so a member object keeps its parentheses.
            yes(is_callee
                || is_object
                || (first && pk == k::BINARY_EXPRESSION && operator_text(cx, &p) == "<"))
        }
        k::IDENTIFIER_REFERENCE => yes((is_let(cx, node) && let_needs_parens(node, &p, is_object))
            || (pk == k::TS_AS_EXPRESSION || pk == k::TS_SATISFIES_EXPRESSION)
                && first
                && starts_cast_statement(cx, node)),
        k::IMPORT_EXPRESSION => yes(pk == k::NEW_EXPRESSION && first),
        k::THIS_EXPRESSION
        | k::ARRAY_EXPRESSION
        | k::TEMPLATE_LITERAL
        | k::REG_EXP_LITERAL
        | k::BOOLEAN_LITERAL
        | k::NULL_LITERAL
        | k::BIG_INT_LITERAL
        | k::SUPER
        | k::NEW_EXPRESSION => Need::No,
        _ => Need::Keep,
    }
}

/// `(type) as T;`, `(let) satisfies T;`: a statement that starts with one of these names and `as` or
/// `satisfies` would read as a declaration or keyword.
fn starts_cast_statement(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    if !cx.first_token(node).is_some_and(|t| {
        matches!(
            t.text(),
            "await" | "interface" | "module" | "using" | "yield" | "let" | "type"
        )
    }) {
        return false;
    }
    let mut cur = outer(node);
    while let Some(p) = cur.parent() {
        match p.kind() {
            k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION
                if p.first_child().as_ref() == Some(&cur) =>
            {
                cur = outer(&p);
            }
            k::EXPRESSION_STATEMENT => return true,
            _ => return false,
        }
    }
    false
}

fn is_let(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    cx.first_token(n).is_some_and(|t| t.text() == "let")
}

/// `(let)[a] = 1` at the start of a statement or head of a `for`, `for (((let)()).a of x)`: without the
/// parentheses the code reads as a declaration. (`for ((let) of x)` keeps the parentheses of its source.)
fn let_needs_parens(node: &SyntaxNode, p: &SyntaxNode, is_object: bool) -> bool {
    let at = |n: &SyntaxNode| *n == *node;
    let in_head = |stmt: &SyntaxNode| {
        stmt.first_child().is_some_and(|head| {
            head.text_range().contains_range(node.text_range()) && starts_with(&head, &at)
        })
    };
    if p.kind() != k::FOR_OF_STATEMENT
        && node
            .ancestors()
            .find(|a| a.kind() == k::FOR_OF_STATEMENT)
            .is_some_and(|f| in_head(&f))
    {
        return true;
    }
    p.kind() == k::COMPUTED_MEMBER_EXPRESSION
        && is_object
        && !optional_link(p)
        && node
            .ancestors()
            .find(|a| {
                matches!(
                    a.kind(),
                    k::EXPRESSION_STATEMENT | k::FOR_STATEMENT | k::FOR_IN_STATEMENT
                )
            })
            .is_some_and(|stmt| in_head(&stmt))
}

/// Is `n` a member or call written with `?.`?
fn optional_link(n: &SyntaxNode) -> bool {
    n.children_with_tokens()
        .any(|e| e.as_token().is_some_and(|t| t.kind() == k::QUESTION_DOT))
}

/// `new (foo())`, `new (a.b().c)`: a call in the callee's object chain would take the arguments.
fn has_call_in_object_chain(node: &SyntaxNode) -> bool {
    let mut n = node.clone();
    loop {
        match n.kind() {
            k::CALL_EXPRESSION | k::IMPORT_EXPRESSION => return true,
            k::STATIC_MEMBER_EXPRESSION
            | k::COMPUTED_MEMBER_EXPRESSION
            | k::PRIVATE_FIELD_EXPRESSION
            | k::TAGGED_TEMPLATE_EXPRESSION
            | k::TS_NON_NULL_EXPRESSION => match n.children().next() {
                Some(c) => n = un(&c),
                None => return false,
            },
            _ => return false,
        }
    }
}

/// Is `node` inside the first part of a `for (;;)` head (or the left side of a `for in`)?
fn in_for_init(node: &SyntaxNode) -> bool {
    let mut child = outer(node);
    while let Some(p) = child.parent() {
        if p.kind() == k::FOR_STATEMENT
            && p.children_with_tokens()
                .find(|e| e.as_token().is_some_and(|t| t.kind() == k::SEMICOLON))
                .is_some_and(|semi| child.text_range().end() <= semi.text_range().start())
        {
            return true;
        }
        // `for (var a = (b in c) in d)`: the initializer of the declaration on the left.
        if p.kind() == k::FOR_IN_STATEMENT && p.first_child().as_ref() == Some(&child) {
            return true;
        }
        child = p;
    }
    false
}
