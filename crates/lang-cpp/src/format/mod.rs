//! C and C++ formatting after clang-format: the tokens of each unwrapped line are spaced by
//! `annotate`, broken by `wrap` (a `Doc` on the shared printer), and indented by `print`, which also
//! places braces, blank lines and comments. A line the formatter does not trust (a preprocessor
//! directive inside it) is kept as written.
//!
//! concept: new verbatim-formatter
//! what: A file or node printed as written.

mod annotate;
mod model;
mod print;
mod style;
mod verify;
mod wrap;

use crate::Dialect;
use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::SyntaxNode;

include!("../schema.generated.rs");

pub struct CppFormatter {
    dialect: Dialect,
}

impl CppFormatter {
    pub const fn new(dialect: Dialect) -> Self {
        CppFormatter { dialect }
    }
}

impl CppFormatter {
    fn layout(&self, src: &str, node: &SyntaxNode, style: &style::Style, verify: bool) -> String {
        let items = model::build(node, src);
        let mut p = print::Printer::new(src, style, self.dialect);
        p.verify = verify;
        p.items(&items, 0, print::Scope::File, 0);
        p.render()
    }
}

impl NodeFormatter for CppFormatter {
    /// The layout of the file; the source itself when the layout would change its meaning or is
    /// not stable under a second format.
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let src = node.text().to_string();
        let style = style::Style::new(cx.options);
        let mut out = self.layout(&src, node, &style, true);
        if out != src {
            let again = crate::parse(&out, self.dialect).tree;
            let same = verify::same(&src, &out, self.dialect);
            let stable = same && self.layout(&out, &again, &style, true) == out;
            if !stable {
                out = src;
            }
        }
        Ok(Some(Doc::Text(out.into())))
    }
}
