//! The layout of the actions of a template language (`{{ if .x }}` of Go templates) in a file whose
//! other text is another language, which stays as written: one blank inside the delimiters of a
//! single-line action, and an own-line control action that trims the blanks before it (`{{-`)
//! stands at the indent of the content it opens, its `else` and `end` where their opener stands;
//! blank lines at the start and the end of the file go.
//! The trim removes the blanks and the line break in front of the action, so the text the template
//! produces is the same. The delimiters and block words are an [`Actions`] value; the actions are
//! found from the tokens of the tree (a token that starts with the opening delimiter up to one that
//! ends with the closing one), so a delimiter inside a string is not one.
//!
//! concept: template-action-layout
//! what: Template actions laid out inside text of another language: blanks inside the delimiters, own-line block actions at the indent of the content they open.
//! tools: none (a layout no tool has)
//! reads: option:align_control_actions, path:core_syntax::template_actions::Actions

use crate::FormatCtx;
use core_syntax::template_actions::{Actions, Step};
use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

/// One action of the file: where it stands and what it does to blocks.
struct Action {
    start: usize,
    end: usize,
    step: Option<Step>,
    /// A block whose text is a template of its own (`define`): it stands where it is.
    detached: bool,
    comment: bool,
    /// The opening delimiter has the trim marker (the blanks before the action are not output).
    left_trim: bool,
}

/// The text of `root` with its actions laid out.
///
/// `detached` are the openers whose block is a template of its own, not part of the text around it
/// (`define`): it stands where it is.
pub fn layout(
    cx: &FormatCtx<'_>,
    root: &SyntaxNode,
    actions: &Actions,
    detached: &[&str],
) -> String {
    let src = root.text().to_string();
    let found = find(cx, root, &src, actions, detached);
    let targets = if cx.options.bool(core_config::opt::ALIGN_CONTROL_ACTIONS) {
        targets(&src, &found)
    } else {
        vec![None; found.len()]
    };
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for (a, target) in found.iter().zip(targets) {
        if let Some(n) = target {
            let line = line_start(&src, a.start);
            out.push_str(&src[at..line]);
            out.push_str(&" ".repeat(n));
            at = a.start;
        }
        out.push_str(&src[at..a.start]);
        match spaced(&src, actions, a) {
            Some(text) => out.push_str(&text),
            None => out.push_str(&src[a.start..a.end]),
        }
        at = a.end;
    }
    out.push_str(&src[at..]);
    // No blank lines at the start of the file, none at its end (the engine writes the final newline).
    let first = out.find(|c: char| !c.is_whitespace()).unwrap_or(out.len());
    let start = out[..first].rfind('\n').map_or(0, |i| i + 1);
    out.truncate(out.trim_end().len());
    out.split_off(start.min(out.len()))
}

/// The actions of `root` in order.
fn find(
    cx: &FormatCtx<'_>,
    root: &SyntaxNode,
    src: &str,
    actions: &Actions,
    detached: &[&str],
) -> Vec<Action> {
    let mut out = Vec::new();
    let mut open: Option<(usize, bool)> = None;
    let mut at = 0;
    for t in root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let text = t.text();
        let trivia = cx.grammar.is_trivia(t.kind());
        match open {
            None if !trivia && text.starts_with(actions.open) => open = Some((at, false)),
            Some((start, comment)) => {
                let comment = comment || (trivia && cx.grammar.has_tag(t.kind(), "comment"));
                open = Some((start, comment));
                if !trivia && text.ends_with(actions.close) {
                    out.push(action(
                        src,
                        (actions, detached),
                        (start, at + text.len()),
                        comment,
                    ));
                    open = None;
                }
            }
            None => {}
        }
        at += text.len();
    }
    out
}

fn action(
    src: &str,
    (actions, detached): (&Actions, &[&str]),
    (start, end): (usize, usize),
    comment: bool,
) -> Action {
    let inner = &src[start + actions.open.len()..end - actions.close.len()];
    Action {
        start,
        end,
        step: actions.step(inner),
        detached: detached.contains(&actions.first_word(inner)),
        comment,
        left_trim: trim_left(actions, inner).is_some(),
    }
}

/// `inner` without its left trim marker, when it has one (the marker is followed by a blank).
fn trim_left<'a>(actions: &Actions, inner: &'a str) -> Option<&'a str> {
    (inner.strip_prefix(actions.trim)).filter(|r| r.starts_with(char::is_whitespace))
}

/// The action written with one blank inside each delimiter, or `None` when it stays as written (it
/// spans lines, holds a comment or has nothing in it).
fn spaced(src: &str, actions: &Actions, a: &Action) -> Option<String> {
    let text = &src[a.start..a.end];
    if a.comment || text.contains('\n') {
        return None;
    }
    let inner = &text[actions.open.len()..text.len() - actions.close.len()];
    let rest = trim_left(actions, inner);
    let rest = rest.unwrap_or(inner);
    let end = rest.trim_end();
    let right = (end.strip_suffix(actions.trim)).filter(|r| r.ends_with(char::is_whitespace));
    let body = right.unwrap_or(rest).trim();
    if body.is_empty() {
        return None;
    }
    Some(format!(
        "{}{}{body}{}{}",
        actions.open,
        if a.left_trim {
            format!("{} ", actions.trim)
        } else {
            " ".into()
        },
        if right.is_some() {
            format!(" {}", actions.trim)
        } else {
            " ".into()
        },
        actions.close,
    ))
}

fn line_start(src: &str, at: usize) -> usize {
    src[..at].rfind('\n').map_or(0, |i| i + 1)
}

fn line_end(src: &str, at: usize) -> usize {
    src[at..].find('\n').map_or(src.len(), |i| at + i)
}

fn blank(s: &str) -> bool {
    s.chars().all(|c| matches!(c, ' ' | '\t' | '\r'))
}

/// The indent each aligned action gets (`None`: it stays). Only an action alone on its line, with
/// the trim marker and spaces in front of it, moves.
fn targets(src: &str, found: &[Action]) -> Vec<Option<usize>> {
    let mut out = vec![None; found.len()];
    // Where the opener of each open block stands.
    let mut stack: Vec<usize> = Vec::new();
    for (i, a) in found.iter().enumerate() {
        let Some(step) = a.step else { continue };
        let line = line_start(src, a.start);
        let before = &src[line..a.start];
        let indent = before.len() - before.trim_start_matches(' ').len();
        let own = a.left_trim
            && !a.comment
            && before.bytes().all(|b| b == b' ')
            && blank(&src[a.end..line_end(src, a.end)]);
        match step {
            Step::Open => {
                let content = (own && !a.detached)
                    .then(|| content_indent(src, found, i))
                    .flatten();
                out[i] = content.filter(|&c| c != indent);
                stack.push(content.unwrap_or(indent));
            }
            Step::Continue | Step::Close => {
                let target = if step == Step::Close {
                    stack.pop()
                } else {
                    stack.last().copied()
                };
                out[i] = target.filter(|&t| own && t != indent);
            }
        }
    }
    out
}

/// The indent of the first line after action `i` that has content: not blank, not a comment or an
/// opening block action alone on its line. `None` when a branch or the end of the block comes
/// first, the line is indented by a tab or the text ends.
fn content_indent(src: &str, found: &[Action], i: usize) -> Option<usize> {
    let mut at = line_end(src, found[i].end);
    let mut next = i + 1;
    while at < src.len() {
        let start = at + 1;
        let end = line_end(src, start);
        let rest = src[start..end].trim_start_matches(' ');
        let spaces = end - start - rest.len();
        if blank(rest) {
            at = end;
            continue;
        }
        if rest.starts_with('\t') {
            return None;
        }
        let q = start + spaces;
        while next < found.len() && found[next].end <= q {
            next += 1;
        }
        match found.get(next) {
            // Inside an action that started on an earlier line.
            Some(a) if a.start < q => return None,
            Some(a) if a.start == q && (a.step.is_some() || a.comment) => {
                if !blank(&src[a.end..line_end(src, a.end)]) {
                    return Some(spaces);
                }
                match a.step {
                    Some(Step::Open) | None => at = line_end(src, a.end),
                    _ => return None,
                }
            }
            _ => return Some(spaces),
        }
    }
    None
}

/// What the safety check compares of an element: the text of a token without what [`layout`]
/// changes (the blank lines at the start and the end of the file, the blanks of the line in front of
/// an action).
pub fn value(item: &SyntaxElement) -> Cow<'_, str> {
    let SyntaxElement::Token(t) = item else {
        return item.to_string().into();
    };
    let last = t.next_token().is_none_or(|n| n.text().is_empty());
    let mut s = t.text();
    if t.prev_token().is_none() {
        let at = s.find(|c: char| !c.is_whitespace()).unwrap_or(s.len());
        s = &s[s[..at].rfind('\n').map_or(0, |i| i + 1)..];
    }
    if last {
        return s.trim_end().into();
    }
    match s.rfind('\n') {
        Some(i) if s[i + 1..].chars().all(|c| matches!(c, ' ' | '\t')) => s[..=i].into(),
        _ => s.into(),
    }
}
