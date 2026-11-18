//! Reading the JavaScript tree of a script: names of keys, calls and object members. Plain
//! accessors over `lang_js`'s node kinds; no rule logic.
//!
//! concept: role-accessors

use core_rules::{Accessor, names};
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange, code_span, first_code_token, grammars};
use lang_js::generated::kind as j;

/// The node without the trivia around it.
pub(super) fn span(n: &SyntaxNode) -> TextRange {
    code_span(n, grammars::JS.get()).unwrap_or_else(|| n.text_range())
}

/// The node under parentheses and `as`.
pub(super) fn unwrap(mut n: SyntaxNode) -> SyntaxNode {
    while matches!(n.kind(), j::PARENTHESIZED_EXPRESSION | j::TS_AS_EXPRESSION)
        && let Some(inner) = n.children().next()
    {
        n = inner;
    }
    n
}

/// The text of the first token: an identifier, or a string with its quotes.
pub(super) fn first(n: &SyntaxNode) -> Option<String> {
    first_code_token(n, grammars::JS.get()).map(|t| t.text().to_string())
}

/// The name of an identifier reference node.
pub(super) fn ident(n: &SyntaxNode) -> Option<String> {
    (n.kind() == j::IDENTIFIER_REFERENCE)
        .then(|| first(n))
        .flatten()
}

/// The value of a string literal or a template literal without substitutions.
pub(super) fn string(n: &SyntaxNode) -> Option<String> {
    let text = match n.kind() {
        j::STRING_LITERAL => first(n)?,
        j::TEMPLATE_LITERAL => n.text().to_string(),
        _ => return None,
    };
    let inner = text.get(1..text.len().checked_sub(1)?)?;
    (!inner.contains("${")).then(|| inner.to_string())
}

/// The statically known name of a property key (a key of an object, a type member).
pub(super) fn key_name(key: &SyntaxNode) -> Option<String> {
    names::static_name(grammars::JS.get(), key)
}

/// The arguments of a call.
pub(super) fn args(call: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    grammars::JS
        .get()
        .slots(call, "arguments")
        .filter_map(|e| e.into_node())
}

/// The first type argument of a call.
pub(super) fn type_arg(call: &SyntaxNode) -> Option<SyntaxNode> {
    call.child(j::TS_TYPE_PARAMETER_INSTANTIATION)?
        .children()
        .next()
}

/// Is `n` a call of the plain name `name`?
pub(super) fn calls(n: &SyntaxNode, name: &str) -> bool {
    n.kind() == j::CALL_EXPRESSION
        && n.children()
            .next()
            .and_then(|c| ident(&c))
            .is_some_and(|c| c == name)
}

/// The member of an object expression: key, value and the shape of the property.
pub(super) struct Member {
    pub key: SyntaxNode,
    pub value: Option<SyntaxNode>,
    pub accessor: Option<Accessor>,
    pub shorthand: bool,
}

/// The parts of an object property; `None` for a spread element or anything else.
pub(super) fn member(p: &SyntaxNode) -> Option<Member> {
    if p.kind() != j::OBJECT_PROPERTY {
        return None;
    }
    let mut nodes = p.children();
    let key = nodes.next()?;
    let accessor = names::accessor(grammars::JS.get(), p);
    let shorthand = key.child(j::IDENTIFIER_REFERENCE).is_some();
    Some(Member {
        key,
        value: nodes.next(),
        accessor,
        shorthand,
    })
}

/// The properties of an object expression (spreads left out).
pub(super) fn members(obj: &SyntaxNode) -> impl Iterator<Item = (SyntaxNode, Member)> {
    obj.children().filter_map(|p| member(&p).map(|m| (p, m)))
}

/// The static name of an object property, the name of `getStaticPropertyName`.
pub(super) fn static_name(m: &Member) -> Option<String> {
    key_name(&m.key)
}

/// The property `name` of an object expression, when a property with a static name has it.
pub(super) fn property(obj: &SyntaxNode, name: &str) -> Option<(SyntaxNode, Member)> {
    members(obj).find(|(_, m)| static_name(m).is_some_and(|n| n == name))
}
