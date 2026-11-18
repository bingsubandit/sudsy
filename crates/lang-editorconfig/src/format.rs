//! EditorConfig formatting: `[glob]` headers, `key = value` pairs one per line, a blank line before
//! each section. Keys, globs, values and comments are kept as written.

use core_format::rules::LazyRules;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::{SyntaxNode, grammars};

use core_config::shared;

include!("schema.generated.rs");

pub struct EditorConfigFormatter;

static ROWS: LazyRules = LazyRules::new(
    || grammars::EDITORCONFIG.get(),
    include_str!("../format.rules"),
);

impl NodeFormatter for EditorConfigFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        ROWS.format_node(node, cx)
    }

    fn blank_between(&self, cx: &FormatCtx<'_>, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
        ROWS.blank_between(cx, prev, next)
    }
}
