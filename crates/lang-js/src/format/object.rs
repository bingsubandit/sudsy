//! Objects, arrays and destructuring patterns (Prettier's `printObject`, `printArray`,
//! `printProperty`): an object stays expanded when the source has a line break after its `{`,
//! numbers fill lines, a pattern that holds a nested pattern always breaks. The quotes of property
//! names (`quote_props`) are decided before formatting, in [`KeyEdits`].

use super::ast::{parent, un};
use super::util::{R, inline, is_tok, kids, node_of, shape_error, style, token_of};
use crate::generated::kind as k;
use core_config::{OptionValue, ResolvedOptions};
use core_format::doc::{Doc, concat, hardline, indent, lit};
use core_format::shapes::{self, ListLayout, ListStyle, TrailingComma};
use core_format::{FormatCtx, FormatError};
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let doc = match node.kind() {
        k::OBJECT_EXPRESSION | k::OBJECT_PATTERN | k::OBJECT_ASSIGNMENT_TARGET => object(node, cx)?,
        k::ARRAY_EXPRESSION | k::ARRAY_PATTERN | k::ARRAY_ASSIGNMENT_TARGET => array(node, cx)?,
        k::OBJECT_PROPERTY | k::BINDING_PROPERTY | k::ASSIGNMENT_TARGET_PROPERTY_PROPERTY => {
            property(node, cx)?
        }
        k::ASSIGNMENT_TARGET_PROPERTY_IDENTIFIER
        | k::ASSIGNMENT_PATTERN
        | k::ASSIGNMENT_TARGET_WITH_DEFAULT
        | k::BINDING_REST_ELEMENT
        | k::ASSIGNMENT_TARGET_REST
        | k::FORMAL_PARAMETER_REST => inline(cx, &kids(cx, node))?,
        _ => return Ok(None),
    };
    Ok(Some(doc))
}

/// The delimiters of a list node and whether it holds nothing else.
fn brackets(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
) -> Result<(SyntaxToken, SyntaxToken, bool), FormatError> {
    let elems = kids(cx, node);
    let (Some(open), Some(close)) = (
        elems.first().and_then(token_of),
        elems.last().and_then(token_of),
    ) else {
        return Err(shape_error("list without brackets"));
    };
    Ok((open.clone(), close.clone(), elems.len() == 2))
}

/// The list style Prettier gives objects, arrays and parameters: a blank line between items
/// breaks the list, the comma per `trailing_comma` (`all_only`: only at `all`).
pub(crate) fn list_style(cx: &FormatCtx<'_>, all_only: bool, layout: ListLayout) -> ListStyle {
    ListStyle {
        trailing_comma: if style(cx).commas >= if all_only { 2 } else { 1 } {
            TrailingComma::WhenBroken
        } else {
            TrailingComma::Never
        },
        layout,
        blank_breaks: true,
        ..ListStyle::default()
    }
}

/// `{ a, b }` of an object, an object pattern or an assignment target.
fn object(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let (open, close, empty) = brackets(cx, node)?;
    if empty {
        return Ok(empty_with_comments(cx, &open, &close));
    }
    let is_pattern = node.kind() != k::OBJECT_EXPRESSION;
    let p = parent(node);
    let pk = p.as_ref().map(SyntaxNode::kind);
    let nested_pattern = is_pattern
        && !matches!(
            pk,
            Some(k::FORMAL_PARAMETER | k::CATCH_PARAMETER | k::ASSIGNMENT_PATTERN)
        )
        && node.children().any(|prop| {
            prop.children_with_tokens()
                .any(|e| e.as_token().is_some_and(|t| t.kind() == k::COLON))
                && prop.last_child().is_some_and(|v| {
                    matches!(
                        v.kind(),
                        k::OBJECT_PATTERN
                            | k::ARRAY_PATTERN
                            | k::OBJECT_ASSIGNMENT_TARGET
                            | k::ARRAY_ASSIGNMENT_TARGET
                    )
                })
        });
    // A lone pattern parameter hugs the parentheses and a pattern on the left of `=` shares the
    // group of the assignment.
    let shared = is_pattern
        && match (&p, pk) {
            (Some(p), Some(k::FORMAL_PARAMETER)) => {
                pattern_with_type_literal(p)
                    || (!p.children_with_tokens().any(|e| is_tok(&e, "="))
                        && hugs_only_param(p)
                        && p.parent().is_some_and(|fp| fp.children().count() == 1))
            }
            (_, Some(k::VARIABLE_DECLARATOR | k::ASSIGNMENT_EXPRESSION)) => !nested_pattern,
            _ => false,
        };
    let mut list = list_style(
        cx,
        false,
        if is_pattern {
            ListLayout::Fit
        } else {
            ListLayout::PreserveFirstBreak
        },
    );
    list.padding = style(cx).bracket_spacing;
    list.force_break = nested_pattern;
    list.shared = shared;
    shapes::delimited_list(cx, node, &list)
}

/// A parameter `{ a, b }: { a: A; b: B }`: the pattern and the type literal break together,
/// as one group of the parameter.
pub(crate) fn pattern_with_type_literal(param: &SyntaxNode) -> bool {
    param.kind() == k::FORMAL_PARAMETER
        && param
            .first_child()
            .is_some_and(|c| c.kind() == k::OBJECT_PATTERN)
        && param
            .children()
            .find(|c| c.kind() == k::TS_TYPE_ANNOTATION)
            .and_then(|a| a.first_child())
            .is_some_and(|t| t.kind() == k::TS_TYPE_LITERAL)
}

/// A lone parameter that is a pattern, kept on the line of the parentheses (Prettier's
/// `shouldHugTheOnlyFunctionParameter`): `({ a, b }) =>`, `([a]: T) =>`, `({ a } = {}) =>`.
pub(crate) fn hugs_only_param(param: &SyntaxNode) -> bool {
    if param.kind() != k::FORMAL_PARAMETER {
        return false;
    }
    let nodes: Vec<SyntaxNode> = param.children().collect();
    let has_eq = param
        .children_with_tokens()
        .any(|e| e.as_token().is_some_and(|t| t.text() == "="));
    let pattern = nodes
        .first()
        .is_some_and(|n| matches!(n.kind(), k::OBJECT_PATTERN | k::ARRAY_PATTERN));
    pattern
        && match (
            has_eq,
            nodes[1..]
                .iter()
                .filter(|n| n.kind() != k::TS_TYPE_ANNOTATION)
                .collect::<Vec<_>>()
                .as_slice(),
        ) {
            (false, rest) => rest.is_empty(),
            (true, [init]) => match init.kind() {
                k::IDENTIFIER_REFERENCE => true,
                k::OBJECT_EXPRESSION | k::ARRAY_EXPRESSION => init.children().next().is_none(),
                _ => false,
            },
            (true, _) => false,
        }
}

/// `[a, b]`: a group, or a fill when it holds only numbers.
fn array(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let (open, close, empty) = brackets(cx, node)?;
    if empty {
        return Ok(empty_with_comments(cx, &open, &close));
    }
    let concise = node.kind() == k::ARRAY_EXPRESSION && super::call::is_concise_array(cx, node);
    let mut list = list_style(
        cx,
        false,
        if concise {
            ListLayout::Fill
        } else {
            ListLayout::Fit
        },
    );
    list.blank_breaks = false;
    // Several arrays or objects of several entries each break the outer array.
    list.force_break = node.kind() == k::ARRAY_EXPRESSION && {
        let elems: Vec<SyntaxNode> = node.children().map(|n| un(&n)).collect();
        elems.len() > 1
            && elems.iter().enumerate().all(|(i, e)| {
                matches!(e.kind(), k::OBJECT_EXPRESSION | k::ARRAY_EXPRESSION)
                    && elems.get(i + 1).is_none_or(|n| n.kind() == e.kind())
                    && e.children().count() > 1
            })
    };
    shapes::delimited_list(cx, node, &list)
}

/// `key: value` laid out as an assignment; a shorthand property or a method is printed as it
/// is written.
fn property(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let Some(colon) = elems.iter().position(|e| is_tok(e, ":")) else {
        return inline(cx, &elems);
    };
    let (Some(tok), Some(value)) = (
        token_of(&elems[colon]),
        elems.get(colon + 1).and_then(node_of),
    ) else {
        return Err(shape_error("property"));
    };
    let computed = elems[..colon].iter().any(|e| is_tok(e, "["));
    let short_key = !computed
        && elems[..colon]
            .iter()
            .rev()
            .find_map(node_of)
            .is_some_and(|key| {
                let width = super::ast::text_of(cx, key).chars().count();
                width < usize::try_from(cx.options.int("indent_size")).unwrap_or(2) + 3
            });
    super::assign::assign_like(
        cx,
        node,
        inline(cx, &elems[..colon])?,
        cx.token(tok),
        Some(value),
        short_key,
    )
}

/// The key of a property that is written with a `:` or as a method, with whether it is
/// quoted. `None` for computed keys, shorthand properties and anything but a name or a string.
fn key_of(prop: &SyntaxNode, ts: &dyn Fn() -> bool) -> Option<(SyntaxNode, bool)> {
    let shorthand = match prop.kind() {
        k::METHOD_DEFINITION
        | k::PROPERTY_DEFINITION
        | k::TS_PROPERTY_SIGNATURE
        | k::TS_METHOD_SIGNATURE
        | k::TS_ENUM_MEMBER => false,
        k::OBJECT_PROPERTY => prop.children().count() == 1,
        k::BINDING_PROPERTY | k::ASSIGNMENT_TARGET_PROPERTY_PROPERTY => !prop
            .children_with_tokens()
            .any(|e| e.as_token().is_some_and(|t| t.kind() == k::COLON)),
        _ => return None,
    };
    let computed = prop
        .children_with_tokens()
        .any(|e| e.as_token().is_some_and(|t| t.kind() == k::L_BRACKET));
    if shorthand || computed {
        return None;
    }
    let key = prop
        .children()
        .find(|c| !matches!(c.kind(), k::DECORATOR))?;
    match key.kind() {
        // (Prettier keeps the quotes of class properties in TypeScript)
        k::STRING_LITERAL if prop.kind() == k::PROPERTY_DEFINITION && ts() => None,
        k::STRING_LITERAL => Some((key, true)),
        k::IDENTIFIER_NAME => Some((key, false)),
        _ => None,
    }
}

/// `1` and `2.5`, as Prettier prints them (`String(Number(s)) === s`).
fn is_simple_number(s: &str) -> bool {
    let (int, frac) = s.split_once('.').map_or((s, None), |(i, f)| (i, Some(f)));
    !int.is_empty()
        && int.bytes().all(|b| b.is_ascii_digit())
        && (int == "0" || !int.starts_with('0'))
        && frac.is_none_or(|f| {
            !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()) && !f.ends_with('0')
        })
}

/// Does the tree hold TypeScript syntax (Prettier's TypeScript parser keeps some quotes)?
fn has_ts(tree: &SyntaxNode, grammar: &Grammar) -> bool {
    tree.descendants()
        .any(|n| grammar.name(n.kind()).starts_with("TS"))
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c == '$' || c == '_' || c.is_alphabetic())
        && chars.all(|c| c == '$' || c == '_' || c.is_alphanumeric())
}

/// How `quote_props` is applied to the property names of the objects (and class bodies,
/// interfaces, type literals and enums) of one tree, node by node (`rewrite` walks the tree once).
pub(crate) struct KeyEdits<'a> {
    mode: &'a str,
    /// Does the tree hold TypeScript syntax? Found when first asked.
    ts: std::cell::OnceCell<bool>,
}

impl<'a> KeyEdits<'a> {
    pub(crate) fn new(options: &'a ResolvedOptions) -> Self {
        let mode = match options.get("quote_props") {
            Some(OptionValue::String(s)) => s.as_str(),
            _ => "preserve",
        };
        KeyEdits {
            mode,
            ts: std::cell::OnceCell::new(),
        }
    }

    /// The edits of the names of `object`, when it is a list of members: quotes that are not
    /// needed go, and with `consistent` the names are quoted when one needs it. A cut is
    /// `(offset, None)`, an insertion `(offset, Some(text))`.
    pub(crate) fn of(
        &self,
        object: &SyntaxNode,
        text: &str,
        grammar: &Grammar,
        edits: &mut Vec<(usize, Option<&'static str>)>,
    ) {
        if self.mode == "preserve"
            || !matches!(
                object.kind(),
                k::OBJECT_EXPRESSION
                    | k::OBJECT_PATTERN
                    | k::OBJECT_ASSIGNMENT_TARGET
                    | k::CLASS_BODY
                    | k::TS_INTERFACE_BODY
                    | k::TS_TYPE_LITERAL
                    | k::TS_ENUM_BODY
            )
        {
            return;
        }
        let ts = || {
            *self.ts.get_or_init(|| {
                let root = object.ancestors().last().unwrap_or_else(|| object.clone());
                has_ts(&root, grammar)
            })
        };
        let tok = |n: &SyntaxNode| super::rewrite::first_token(grammar, n);
        let keys: Vec<(SyntaxNode, bool)> =
            object.children().filter_map(|p| key_of(&p, &ts)).collect();
        // A quoted name that is not a plain identifier (or number) keeps the quotes of all names.
        let needs_quotes = self.mode == "consistent"
            && keys.iter().any(|(key, quoted)| {
                *quoted
                    && tok(key).is_some_and(|t| {
                        let raw = t.text();
                        let inner = &raw[1..raw.len().saturating_sub(1).max(1)];
                        !is_identifier(inner) && !inner.bytes().all(|b| b.is_ascii_digit())
                    })
            });
        for (key, quoted) in keys {
            let Some(t) = tok(&key) else { continue };
            let start = usize::from(t.text_range().start());
            let end = usize::from(t.text_range().end());
            if quoted && !needs_quotes {
                let inner = &t.text()[1..t.text().len() - 1];
                let glued = text[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c == '$' || c == '_' || c.is_alphanumeric());
                if (is_identifier(inner) || (is_simple_number(inner) && !ts())) && !glued {
                    edits.push((start, None));
                    edits.push((end - 1, None));
                }
            } else if !quoted && needs_quotes {
                edits.push((start, Some("\"")));
                edits.push((end, Some("\"")));
            }
        }
    }
}

/// `[]` or `{}`; comments on lines of their own inside go indented between the brackets.
fn empty_with_comments(cx: &FormatCtx<'_>, open: &SyntaxToken, close: &SyntaxToken) -> Doc {
    use core_format::comments::{self, CommentKind};
    let mut all = comments::trailing_comments(cx.grammar, open);
    all.extend(comments::leading_comments(cx.grammar, close));
    let own_lines = all
        .iter()
        .any(|c| c.kind != CommentKind::Block || c.breaks_after > 0);
    if !own_lines {
        // `{/* c */}`: inline comments sit tight between the brackets.
        let inner = all
            .iter()
            .map(core_format::context::comment_doc)
            .collect::<Vec<_>>();
        return concat(vec![
            cx.leading_comments_doc(open),
            cx.token_text(open),
            core_format::doc::join(&lit(" "), inner),
            cx.token_text(close),
            cx.trailing_comments_doc(close),
        ]);
    }
    concat(vec![
        cx.leading_comments_doc(open),
        cx.token_text(open),
        indent(core_format::shapes::dangling_comments(&all, false)),
        hardline(),
        cx.token_text(close),
        cx.trailing_comments_doc(close),
    ])
}
