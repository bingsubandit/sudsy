//! Ruby formatting (see docs/formatting/ruby.md).

use crate::generated::kind as k;
use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::SyntaxNode;

include!("schema.generated.rs");

pub struct RubyFormatter;

impl NodeFormatter for RubyFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        _: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(node
            .parent()
            .is_none()
            .then(|| doc::verbatim(node.text().to_string())))
    }

    /// A file that ends in the data after `__END__` keeps its end: the text is the data.
    fn exact_end(&self, root: &SyntaxNode) -> bool {
        let mut last = root.last_token();
        while let Some(t) = last {
            let kind = t.kind();
            if kind == k::DATA {
                return true;
            }
            if kind != k::EOF && kind != k::WHITESPACE && kind != k::NEWLINE && kind != k::COMMENT {
                return false;
            }
            last = t.prev_token();
        }
        false
    }
}
