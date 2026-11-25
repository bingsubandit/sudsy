//! Lines of a construct and of the statements in its body that start with the blanks the source
//! has in front of them, at any depth (a Makefile conditional the style does not indent).
//!
//! concept: source-indented-lines
//! what: The lines of a construct and its body statements keep the indentation the source has.
//! tools: mbake conditional bodies left as written
//! reads: path:source_indent::SourceIndent

use crate::FormatCtx;
use crate::doc::Doc;
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

/// The kinds that decide which lines keep the source's blanks.
pub struct SourceIndent {
    /// A line of a node of one of these kinds starts at the blanks of the source...
    pub lines: &'static [SyntaxKind],
    /// ...when the node is one of these or stands in the body of one of these...
    pub containers: &'static [SyntaxKind],
    /// ...unless it stands inside one of these (a recipe: its lines are commands).
    pub stops: &'static [SyntaxKind],
    /// Of the `lines` kinds, these only when the node is a direct child of a container (a command
    /// line that stands in the body of a conditional, not one of a rule's recipe).
    pub direct: &'static [SyntaxKind],
}

impl SourceIndent {
    /// Does a line of `node` keep the blanks of the source?
    pub fn applies(&self, node: &SyntaxNode) -> bool {
        if !self.lines.contains(&node.kind()) {
            return false;
        }
        if self.direct.contains(&node.kind())
            && !node
                .parent()
                .is_some_and(|p| self.containers.contains(&p.kind()))
        {
            return false;
        }
        let mut within = self.containers.contains(&node.kind());
        for a in node.ancestors().skip(1) {
            if self.stops.contains(&a.kind()) {
                return false;
            }
            within |= self.containers.contains(&a.kind());
        }
        within
    }
}

impl FormatCtx<'_> {
    /// `build()` (the line of `node`) at the blanks of the source when `kinds` say so.
    pub fn source_indented<E>(
        &self,
        node: &SyntaxNode,
        kinds: &SourceIndent,
        build: impl FnOnce() -> Result<Doc, E>,
    ) -> Result<Doc, E> {
        match self.first_token(node) {
            Some(first) => self.source_indented_at(node, &first, kinds, build),
            None => build(),
        }
    }

    /// [`FormatCtx::source_indented`] for the line that `first` starts (a keyword of `node`).
    pub fn source_indented_at<E>(
        &self,
        node: &SyntaxNode,
        first: &SyntaxToken,
        kinds: &SourceIndent,
        build: impl FnOnce() -> Result<Doc, E>,
    ) -> Result<Doc, E> {
        if kinds.applies(node) {
            self.at_source_indent(first, build)
        } else {
            build()
        }
    }
}
