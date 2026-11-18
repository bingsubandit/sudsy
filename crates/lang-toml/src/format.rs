//! TOML formatting: the shared shapes with TOML's layout. A table is its header and then its pairs
//! one per line, without indentation (there are no braces); an inline table never breaks (TOML 1.0
//! forbids line breaks in it); arrays follow the list shape.

use crate::generated::kind as k;
use core_config::{ResolvedOptions, shared};
use core_format::rules::LazyRules;
use core_format::shapes::{ListLayout, ListStyle, TrailingComma, delimited_list};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::{SyntaxElement, SyntaxNode, grammars};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct TomlFormatter;

static ROWS: LazyRules = LazyRules::new(|| grammars::TOML.get(), include_str!("../format.rules"));

fn array_style(options: &ResolvedOptions) -> ListStyle {
    ListStyle {
        padding: options.bool(shared::ARRAY_BRACKET_SPACING),
        trailing_comma: if trailing_commas(options) == TrailingCommas::Multiline {
            TrailingComma::WhenBroken
        } else {
            TrailingComma::Never
        },
        layout: match collapse(options) {
            Collapse::Always => ListLayout::Fit,
            Collapse::Never => ListLayout::Break,
            Collapse::Preserve => ListLayout::PreserveBreak,
        },
        ..ListStyle::default()
    }
}

impl NodeFormatter for TomlFormatter {
    /// `end_of_line` rewrites the line breaks inside multi-line strings; nothing else may change.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) => t.text().replace("\r\n", "\n").into(),
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let doc = match node.kind() {
            k::ARRAY => delimited_list(cx, node, &array_style(cx.options))?,
            k::INLINE_TABLE => delimited_list(
                cx,
                node,
                &ListStyle {
                    padding: cx.options.bool(shared::BRACKET_SPACING),
                    layout: ListLayout::Flat,
                    ..ListStyle::default()
                },
            )?,
            k::DOCUMENT | k::TABLE | k::ARRAY_TABLE | k::PAIR => return ROWS.format_node(node, cx),
            _ => return Ok(None),
        };
        Ok(Some(doc))
    }
}
