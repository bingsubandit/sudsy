//! `sudsy-ignore`, `sudsy-ignore-start`, `sudsy-ignore-end` comments and the foreign markers a
//! grammar maps onto them (`prettier-ignore`, `fmt: off`, `fmt: skip`). The marker text comes
//! from each grammar's `suppress` rows (roles `ignore`, `ignore-start`, `ignore-end`,
//! `ignore-line`) and the comment delimiters from its `comments` line.

use crate::comments::{Comment, CommentKind};
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgnoreMark {
    /// Leave the next node as written.
    Ignore,
    /// Leave everything as written from the next node until the matching end.
    Start,
    End,
    /// A trailing comment: leave the statement it ends as written.
    Line,
}

/// The comment's text without its delimiters; empty for what is not a plain comment.
fn body(g: &Grammar, c: &Comment) -> String {
    match c.kind {
        CommentKind::Line | CommentKind::Block => g.comment_body(&c.text()).to_string(),
        CommentKind::Lines | CommentKind::Directive => String::new(),
    }
}

/// A role may list several markers (`sudsy-ignore` next to `prettier-ignore`); a marker is the
/// whole comment body or its start, followed by a blank (`fmt: skip # why`).
fn has_marker(g: &Grammar, role: &str, body: &str) -> bool {
    g.suppressions.iter().any(|s| {
        s.role == role
            && body
                .strip_prefix(s.marker.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    })
}

/// Is `text` (a comment body, delimiters removed) the marker of `role`? For hosts that keep
/// their comments as text, like the HTML tree.
pub fn is_marker(g: &Grammar, role: &str, text: &str) -> bool {
    has_marker(g, role, text.trim())
}

pub fn mark_of(g: &Grammar, c: &Comment) -> Option<IgnoreMark> {
    let body = body(g, c);
    let body = body.trim();
    [
        ("ignore", IgnoreMark::Ignore),
        ("ignore-start", IgnoreMark::Start),
        ("ignore-end", IgnoreMark::End),
        ("ignore-line", IgnoreMark::Line),
    ]
    .into_iter()
    .find(|(role, _)| has_marker(g, role, body))
    .map(|(_, mark)| mark)
}

/// The ignore mark among a token's leading comments (the last one wins).
pub fn mark_in(g: &Grammar, comments: &[Comment]) -> Option<IgnoreMark> {
    comments.iter().rev().find_map(|c| mark_of(g, c))
}

/// Does a trailing comment of `tok` ask for the line it ends to stay as written?
pub fn line_ignored(g: &Grammar, tok: &SyntaxToken) -> bool {
    crate::comments::trailing_comments(g, tok)
        .iter()
        .any(|c| mark_of(g, c) == Some(IgnoreMark::Line))
}

/// Does a comment inside `node` (a container that holds nothing else, like JSX `{/* ... */}`)
/// carry the `ignore` mark?
pub fn holds_mark(g: &Grammar, node: &SyntaxNode) -> bool {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| g.has_tag(t.kind(), "comment") && is_marker(g, "ignore", g.comment_body(t.text())))
}

/// Walks items in order and says which are kept as written: a `Start` mark opens a region that
/// the next `End` closes, an `Ignore` mark covers one item.
#[derive(Default)]
pub struct IgnoreRun {
    open: bool,
}

impl IgnoreRun {
    pub fn skips(&mut self, mark: Option<IgnoreMark>) -> bool {
        match mark {
            Some(IgnoreMark::Start) => self.open = true,
            Some(IgnoreMark::End) => self.open = false,
            _ => {}
        }
        self.open || mark == Some(IgnoreMark::Ignore)
    }
}

/// The end (exclusive) of the region a `Start` mark at item `i` of `len` opens: the next item
/// whose mark is `End`, or `len`.
pub fn region_end<E>(
    len: usize,
    i: usize,
    mut mark_at: impl FnMut(usize) -> Result<Option<IgnoreMark>, E>,
) -> Result<usize, E> {
    let mut j = i + 1;
    while j < len && mark_at(j)? != Some(IgnoreMark::End) {
        j += 1;
    }
    Ok(j)
}
