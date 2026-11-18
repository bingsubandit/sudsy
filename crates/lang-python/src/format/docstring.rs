//! Docstrings: quotes, indentation and trailing whitespace as ruff writes them.

use super::util::*;
use crate::generated::kind as k;
use core_format::FormatCtx;
use core_format::doc::{self, Doc, concat, hardline};
use core_format::literals::prefixed;
use core_syntax::SyntaxNode;

/// A docstring statement: the string with ruff's layout.
pub(crate) fn statement(cx: &FormatCtx<'_>, stmt: &SyntaxNode) -> R {
    let tokens: Vec<_> = stmt
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::STRING)
        .collect();
    let (Some(token), Some(last_token)) = (tokens.first(), tokens.last()) else {
        return Err(shape_error("docstring"));
    };
    let text = if tokens.len() == 1 {
        prefixed::string(syntax(), token.text(), style(cx).quote).into_owned()
    } else {
        // An implicit concatenation is the docstring when its parts join; else a plain string.
        let strings = stmt
            .children()
            .next()
            .ok_or_else(|| shape_error("docstring"))?;
        match super::expr::joined(cx, &strings) {
            Some(flat) => flat,
            None => return super::stmt::statement(cx, stmt),
        }
    };
    let Some((prefix, quote, body)) = prefixed::split(syntax(), &text) else {
        return Ok(cx.verbatim_node(stmt));
    };
    let body = clean(body);
    let lines: Vec<&str> = body.split('\n').collect();
    let mut parts: Vec<Doc> = vec![
        cx.leading_comments_doc(token),
        doc::text(format!("{prefix}{quote}")),
    ];
    let last = lines.len() - 1;
    if lines.len() == 1 {
        let l = lines[0].trim();
        if l.is_empty() {
            parts.push(doc::text(" "));
        } else {
            parts.push(doc::text(l.to_string()));
            if l.ends_with(quote.chars().next().unwrap_or('"')) || ends_with_odd_backslash(l) {
                parts.push(doc::text(" "));
            }
        }
        parts.push(doc::text(quote.to_string()));
        return Ok(concat(parts));
    }
    let indent_of = |l: &str| l.len() - l.trim_start().len();
    let min = lines[1..]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| indent_of(l))
        .min()
        .unwrap_or(0);
    parts.push(doc::text(lines[0].trim().to_string()));
    let closing_alone = lines[last].trim().is_empty();
    let end = if closing_alone { last } else { last + 1 };
    // Trailing blank lines before the closing quotes go.
    let stop = end;
    for l in &lines[1..stop] {
        parts.push(hardline());
        if l.trim().is_empty() {
            continue;
        }
        parts.push(doc::text(l[min.min(indent_of(l))..].trim_end().to_string()));
    }
    if closing_alone {
        parts.push(hardline());
    } else if let Some(l) = lines[1..stop].last()
        && (l.trim_end().ends_with(quote.chars().next().unwrap_or('"'))
            || ends_with_odd_backslash(l.trim_end()))
    {
        parts.push(doc::text(" "));
    }
    parts.push(doc::text(quote.to_string()));
    parts.push(cx.trailing_comments_doc(last_token));
    Ok(concat(parts))
}

fn ends_with_odd_backslash(l: &str) -> bool {
    l.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1
}

/// The docstring with the leading tabs of its lines expanded to spaces.
fn clean(body: &str) -> String {
    body.replace("\r\n", "\n")
        .split('\n')
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                return l.to_string();
            }
            let ws = l.len() - l.trim_start_matches([' ', '\t']).len();
            let mut col = 0;
            for c in l[..ws].chars() {
                if c == '\t' {
                    col = (col / 8 + 1) * 8;
                } else {
                    col += 1;
                }
            }
            format!("{}{}", " ".repeat(col), &l[ws..])
        })
        .collect::<Vec<_>>()
        .join("\n")
}
