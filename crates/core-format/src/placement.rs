//! Which leading comments a layout has already printed. A token owns the comments in front of
//! it, but a block that ends right before it may print the deeper ones inside itself
//! ([`FormatCtx::end_comments`]); the token then prints the rest ([`FormatCtx::lead`]).

use crate::comments::{self, Comment};
use crate::context::{FormatCtx, comment_doc, leading_comments_doc};
use crate::doc::{Doc, concat, hardline};
use crate::shapes::dangling_comments;
use core_syntax::{SyntaxNode, SyntaxToken};

impl FormatCtx<'_> {
    /// Records that `n` more leading comments of `tok` are printed.
    pub fn consume(&self, tok: &SyntaxToken, n: usize) {
        *self
            .consumed
            .lock()
            .expect("consumed lock")
            .entry(tok.text_range().start())
            .or_insert(0) += n;
    }

    /// How many leading comments of `tok` are printed.
    pub fn consumed(&self, tok: &SyntaxToken) -> usize {
        self.consumed
            .lock()
            .expect("consumed lock")
            .get(&tok.text_range().start())
            .copied()
            .unwrap_or(0)
    }

    /// The leading comments of `tok` that no block before it has printed.
    pub fn remaining_comments(&self, tok: &SyntaxToken) -> Vec<Comment> {
        let n = self.consumed(tok);
        let mut cs = comments::leading_comments(self.grammar, tok);
        cs.drain(..n.min(cs.len()));
        cs
    }

    /// A token that starts a statement or clause: its remaining comments, printed in front.
    pub fn lead(&self, tok: &SyntaxToken) -> Doc {
        self.hold_leading(tok);
        leading_comments_doc(&self.remaining_comments(tok))
    }

    /// The comments of the token after `last` that stand at column `col` or deeper (`col` is
    /// not 0), printed after `last` at the block's indentation, with the blank line the source
    /// had before them.
    pub fn end_comments(&self, last: &SyntaxNode, col: usize) -> Doc {
        let Some(next) = self
            .last_token(last)
            .and_then(|t| comments::next_significant(self.grammar, &t))
        else {
            return Doc::Nil;
        };
        self.hold_leading(&next);
        let cs = self.remaining_comments(&next);
        let take = if col == 0 {
            0
        } else {
            comments::indented_run(&cs, col)
        };
        if take == 0 {
            return Doc::Nil;
        }
        self.consume(&next, take);
        let blank = comments::blank_lines_before(self.grammar, &next) > 0;
        dangling_comments(&cs[..take], blank)
    }
}

/// Own-line comments one per line (a comment keeps one blank line after it).
pub fn comment_lines(cs: &[Comment]) -> Doc {
    let mut parts = Vec::new();
    for (i, c) in cs.iter().enumerate() {
        if i > 0 {
            parts.push(hardline());
            if cs[i - 1].breaks_after >= 2 {
                parts.push(hardline());
            }
        }
        parts.push(comment_doc(c));
    }
    concat(parts)
}
