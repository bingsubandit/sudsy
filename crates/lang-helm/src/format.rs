//! Helm template formatting: the text between actions (the host YAML) stays as written; the
//! actions are laid out by the shared template layout.
//!
//! concept: none (glue: the root of the template layout)

use core_config::shared;
use core_format::template_layout::{layout, value};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::template_actions::GO;
use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct HelmFormatter;

impl NodeFormatter for HelmFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(node
            .parent()
            .is_none()
            .then(|| doc::verbatim(layout(cx, node, &GO, &["define"]))))
    }

    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        value(item)
    }
}
