//! Shared comment rules, written against the grammar's comment syntax: `comment-space` and
//! `no-empty-comment`. They read comment tokens, so they cost nothing when off.
//!
//! concept: comment-space, no-empty-comment
//! what: Rules on comment tokens: text after the marker, no empty comments.
//! rules: comment-space, no-empty-comment
//! reads: rule:comment-space

use crate::diagnostic::Edit;
use crate::options::{Check, Opt, OptionDecl};
use crate::rule;
use crate::rule::{FileContext, RuleContext};
use crate::tree::{is_code, structured_trivia};
use core_syntax::{Grammar, SyntaxToken, TextRange};

/// The comment tokens of the file (line and block comments, and documentation exteriors).
fn comments<'a>(file: &'a FileContext<'a>) -> impl Iterator<Item = SyntaxToken> + 'a {
    file.root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| file.grammar.has_tag(t.kind(), "comment"))
}

rule! {
    pub CommentSpace {
        id: "comment-space",
        category: Style,
        default: Off,
        fix: Safe,
        description: "A line comment has text right after its marker. Exempt: nothing but \
                  whitespace after the marker, a marker repeated (`////`, `##`), `--` separator \
                  lines, and a `#!` shebang on the first line. The fix inserts the space.",
        example: "//comment   // write: // comment",
        options: [OptionDecl {
            name: "line",
            default: Opt::Bool(true),
            check: Check::Any,
            description: "Check line comments (`// a`). Off, only block comments are checked \
                      (with `block`).",
        }, OptionDecl {
            name: "block",
            default: Opt::Bool(false),
            check: Check::Any,
            description: "Also check block comments: text right after the opening or before the \
                      closing delimiter is reported (`/*a*/`). Exempt: an empty comment, and a \
                      delimiter repeated (`/**`, `/***/`). The fix inserts the space.",
        }],
        format: true,
    }

    fn format_applies(
        &self,
        grammar: &Grammar,
        text: &str,
        own: &crate::options::RuleOptions,
        _format: &core_config::ResolvedOptions,
    ) -> bool {
        let block = own.bool("block")
            && grammar
                .comments
                .block
                .as_ref()
                .is_some_and(|(open, close)| block_tight(text, open, close));
        let line = own.bool("line")
            && grammar
                .comments
                .line
                .as_deref()
                .is_some_and(|marker| line_tight(text, marker));
        block || line
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        if ctx.options.bool("block")
            && let Some((open, close)) = &file.grammar.comments.block
        {
            block_space(file, ctx, open, close);
        }
        let Some(marker) = file.grammar.comments.line.as_deref() else {
            return;
        };
        if !ctx.options.bool("line") {
            return;
        }
        let repeated = marker.chars().last().unwrap_or(' ');
        // (only the comments that start at a marker with text right after it)
        for at in tight_markers(file.text, marker) {
            let Some(token) = file
                .root
                .token_at_offset(core_syntax::TextSize::from(u32::try_from(at).unwrap_or(u32::MAX)))
                .right_biased()
                .filter(|t| {
                    usize::from(t.text_range().start()) == at
                        && file.grammar.has_tag(t.kind(), "comment")
                })
            else {
                continue;
            };
            let Some(rest) = token.text().strip_prefix(marker) else {
                continue;
            };
            let start = token.text_range().start();
            let exempt = rest.trim().is_empty()
                || rest.starts_with([' ', repeated])
                || rest.starts_with("--")
                || (usize::from(start) == 0 && rest.starts_with('!'));
            if exempt {
                continue;
            }
            let after = start + core_syntax::TextSize::of(marker);
            ctx.report_fix(
                TextRange::new(start, after),
                format!("a comment should start with a space after `{marker}`"),
                ("Insert a space", vec![Edit::insert(after, " ")]),
            );
        }
    }
}

/// The offsets of a line comment marker followed right away by text.
fn tight_markers<'a>(text: &'a str, marker: &'a str) -> impl Iterator<Item = usize> + 'a {
    let repeated = marker.chars().last().unwrap_or(' ');
    text.match_indices(marker).filter_map(move |(at, _)| {
        text[at + marker.len()..]
            .chars()
            .next()
            .is_some_and(|c| !c.is_whitespace() && c != repeated)
            .then_some(at)
    })
}

/// Is a line comment marker followed right away by text?
fn line_tight(text: &str, marker: &str) -> bool {
    tight_markers(text, marker).next().is_some()
}

/// Is a block delimiter right against text (no walk of the comments without it)?
fn block_tight(text: &str, open: &str, close: &str) -> bool {
    let first = open.chars().last().unwrap_or(' ');
    let last = close.chars().next().unwrap_or(' ');
    text.match_indices(open).any(|(at, _)| {
        text[at + open.len()..]
            .chars()
            .next()
            .is_some_and(|c| !c.is_whitespace() && c != first)
    }) || text.match_indices(close).any(|(at, _)| {
        text[..at]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_whitespace() && c != last)
    })
}

/// The space of block comments: `/*a*/` wants `/* a */`.
fn block_space(file: &FileContext<'_>, ctx: &mut RuleContext<'_>, open: &str, close: &str) {
    let (first, last) = (
        open.chars().last().unwrap_or(' '),
        close.chars().next().unwrap_or(' '),
    );
    if !block_tight(file.text, open, close) {
        return;
    }
    for token in comments(file) {
        let text = token.text();
        let Some(inner) = text.strip_prefix(open).and_then(|t| t.strip_suffix(close)) else {
            continue;
        };
        let start = token.text_range().start();
        let at = |offset: usize| start + core_syntax::TextSize::of(&text[..offset]);
        if !inner.is_empty() && inner.trim_matches([first, last]).is_empty() {
            continue;
        }
        let tight_open = !inner.starts_with([' ', '\t', '\n', '\r', first]);
        let tight_close = !inner.ends_with([' ', '\t', '\n', '\r', last]);
        if tight_open {
            let pos = at(open.len());
            ctx.report_fix(
                TextRange::new(start, pos),
                format!("a comment should start with a space after `{open}`"),
                ("Insert a space", vec![Edit::insert(pos, " ")]),
            );
        }
        if tight_close {
            let pos = at(text.len() - close.len());
            ctx.report_fix(
                TextRange::new(pos, start + core_syntax::TextSize::of(text)),
                format!("a comment should end with a space before `{close}`"),
                ("Insert a space", vec![Edit::insert(pos, " ")]),
            );
        }
    }
}

/// Does `token` follow code on its own line, which makes it part of that code's trailing trivia?
fn after_code_on_line(token: &SyntaxToken, grammar: &Grammar) -> bool {
    let mut t = token.prev_token();
    while let Some(p) = t {
        if grammar.has_tag(p.kind(), "whitespace") {
            t = p.prev_token();
        } else {
            return is_code(&p, grammar);
        }
    }
    false
}

/// Does the comment `token` have another comment, directive or disabled text before it
/// (`forward` false) or after it (true) in the same run of trivia? A run is a token's trailing
/// trivia (up to the end of its line) or the leading trivia that follows.
fn has_neighbour(token: &SyntaxToken, grammar: &Grammar, forward: bool) -> bool {
    let trailing = after_code_on_line(token, grammar);
    let mut crossed = false;
    let mut t = if forward {
        token.next_token()
    } else {
        token.prev_token()
    };
    while let Some(n) = t {
        if grammar.has_tag(n.kind(), "whitespace") {
        } else if grammar.has_tag(n.kind(), "newline") {
            if forward && trailing {
                return false;
            }
            crossed = true;
        } else if is_code(&n, grammar) {
            return false;
        } else if forward || !crossed {
            return true;
        } else {
            // Before the comment, across a line break: the same run unless that trivia is the
            // trailing trivia of the code before it.
            let first = structured_trivia(&n, grammar)
                .and_then(|s| s.first_token())
                .unwrap_or(n);
            return !after_code_on_line(&first, grammar);
        }
        t = if forward {
            n.next_token()
        } else {
            n.prev_token()
        };
    }
    false
}

rule! {
    pub NoEmptyComment {
        id: "no-empty-comment",
        category: Style,
        default: Off,
        description: "A comment has no text. A block comment is reported wherever it is; an empty line \
                  comment only when it begins or ends a run of comments, because a bare `//` \
                  between two comment lines separates paragraphs.",
        example: "// ----\n//\nint x;   // the empty comment above is reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let comment = &file.grammar.comments;
        for token in comments(file) {
            let text = token.text();
            let (empty, block) = match (comment.line.as_deref(), &comment.block) {
                (Some(line), _) if text.starts_with(line) => {
                    (text[line.len()..].trim().is_empty(), false)
                }
                (_, Some((open, close)))
                    if text.len() >= open.len() + close.len()
                        && text.starts_with(open.as_str())
                        && text.ends_with(close.as_str()) =>
                {
                    (
                        text[open.len()..text.len() - close.len()].trim().is_empty(),
                        true,
                    )
                }
                _ => continue,
            };
            if !empty {
                continue;
            }
            if block
                || !(has_neighbour(&token, file.grammar, false)
                    && has_neighbour(&token, file.grammar, true))
            {
                ctx.report(token.text_range(), "the comment has no text");
            }
        }
    }
}
