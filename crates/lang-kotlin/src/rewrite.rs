//! What ktlint rewrites rather than lays out, done on the source before it is formatted (the
//! formatter's `rewrite_source`): the import order. Each rewrite is a list of edits of the text,
//! so what stays is untouched; the result is parsed again.
//!
//! concept: source-rewrite, sorted-imports

use crate::generated::kind as k;
use core_syntax::SyntaxNode;

/// The order of `ktlint_official`: all imports in ASCII order of their path, `java.`, `javax.`
/// and `kotlin.` imports after the others, aliased imports last; no duplicates.
fn import_group(path: &str, alias: bool) -> u8 {
    if alias {
        return 4;
    }
    ["java.", "javax.", "kotlin."]
        .iter()
        .position(|p| path.starts_with(p))
        .map_or(0, |i| i as u8 + 1)
}

fn sort_imports(tree: &SyntaxNode, text: &str, edits: &mut Vec<(usize, usize, String)>) {
    let Some(list) = tree.children().find(|n| n.kind() == k::IMPORT_LIST) else {
        return;
    };
    let mut items: Vec<(u8, String, &str)> = Vec::new();
    let mut span: Option<(usize, usize)> = None;
    let mut gap_from = 0;
    for n in list.children().filter(|n| n.kind() == k::IMPORT_DIRECTIVE) {
        let r = n.text_range();
        let (s, e) = (usize::from(r.start()), usize::from(r.end()));
        let piece = text[s..e].trim();
        let start = s + (text[s..e].len() - text[s..e].trim_start().len());
        // Comments between or inside imports belong to them: leave the list as written.
        if piece.contains("//")
            || piece.contains("/*")
            || (span.is_some() && !text[gap_from..start].trim().is_empty())
        {
            return;
        }
        gap_from = start + piece.len();
        span = Some((span.map_or(start, |(a, _)| a), gap_from));
        let alias = n.children().any(|c| c.kind() == k::IMPORT_ALIAS);
        let path: String = piece
            .strip_prefix("import")
            .unwrap_or(piece)
            .split_whitespace()
            .take_while(|w| *w != "as")
            .collect();
        items.push((import_group(&path, alias), path, piece));
    }
    let Some((from, to)) = span else { return };
    items.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    items.dedup_by(|a, b| a.2 == b.2);
    let sorted = items.iter().map(|i| i.2).collect::<Vec<_>>().join("\n");
    if sorted != text[from..to] {
        edits.push((from, to, sorted));
    }
}

/// A blank line between a `//` comment and a documentation comment that follows it directly.
fn blank_before_kdoc(tree: &SyntaxNode, text: &str, edits: &mut Vec<(usize, usize, String)>) {
    for t in tree
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let (s, e) = (
            usize::from(t.text_range().start()),
            usize::from(t.text_range().end()),
        );
        if t.kind() == k::COMMENT && text[s..e].starts_with("//") && text[e..].starts_with("\n") {
            let rest = text[e + 1..].trim_start_matches([' ', '\t']);
            if rest.starts_with("/**") {
                edits.push((e, e, "\n".into()));
            }
        }
    }
}

/// The last entry of an enum with one entry per line gets a trailing comma; a `;` after it goes
/// on a line of its own.
fn enum_trailing_comma(tree: &SyntaxNode, text: &str, edits: &mut Vec<(usize, usize, String)>) {
    for body in tree.descendants().filter(|n| n.kind() == k::CLASS_BODY) {
        let entries: Vec<_> = body
            .children()
            .filter(|n| n.kind() == k::ENUM_ENTRY)
            .collect();
        let Some(entry) = entries.last() else {
            continue;
        };
        let toks: Vec<_> = entry
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .collect();
        if toks.iter().any(|t| t.kind() == k::COMMA) {
            continue;
        }
        let start = code_start(entry);
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        let indent = &text[line_start..start];
        let own_line = line_start > 0 && indent.trim().is_empty();
        if !own_line && entries.len() < 2 {
            continue;
        }
        if let Some(semi) = toks.iter().find(|t| t.kind() == k::SEMICOLON) {
            let (s, e) = (
                usize::from(semi.text_range().start()),
                usize::from(semi.text_range().end()),
            );
            if own_line {
                edits.push((s, e, format!(",\n{indent};")));
            }
        } else if let Some(last) = last_code_end(entry) {
            edits.push((last, last, ",".into()));
        }
    }
}

/// The end of the last token of `node` that is not trivia.
fn last_code_end(node: &SyntaxNode) -> Option<usize> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !matches!(t.kind(), k::WHITESPACE | k::NEWLINE | k::COMMENT))
        .last()
        .map(|t| usize::from(t.text_range().end()))
}

/// `fun f(): T { return x }` becomes `fun f(): T = x` (also `throw`): a body that is one `return` or
/// `throw` without a label and without comments.
fn expression_body(tree: &SyntaxNode, edits: &mut Vec<(usize, usize, String)>) {
    for fun in tree.descendants().filter(|n| n.kind() == k::FUN) {
        let Some(block) = fun.children().find(|n| n.kind() == k::BLOCK) else {
            continue;
        };
        let mut items = block.children();
        let (Some(stmt), None) = (items.next(), items.next()) else {
            continue;
        };
        let labelled = stmt.children().any(|c| c.kind() == k::LABEL_QUALIFIER);
        let value = stmt
            .children()
            .find(|c| !matches!(c.kind(), k::LABEL_QUALIFIER));
        let (Some(value), false) = (value, labelled) else {
            continue;
        };
        let simple = matches!(stmt.kind(), k::RETURN | k::THROW);
        let comment = block
            .descendants_with_tokens()
            .any(|e| e.kind() == k::COMMENT);
        if !simple || comment {
            continue;
        }
        let (b, v) = (block.text_range(), value.text_range());
        let rbrace = block
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == k::RBRACE);
        let Some(end) = rbrace.map(|t| t.text_range().end()) else {
            continue;
        };
        let prefix = if stmt.kind() == k::THROW {
            "= throw "
        } else {
            "= "
        };
        edits.push((b.start().into(), v.start().into(), prefix.into()));
        edits.push((v.end().into(), end.into(), String::new()));
    }
}

fn braced(n: &SyntaxNode) -> bool {
    n.kind() == k::BLOCK && n.children_with_tokens().any(|e| e.kind() == k::LBRACE)
}

/// What a branch (`then`, `else`, a loop body) holds.
fn content(branch: &SyntaxNode) -> Option<SyntaxNode> {
    branch.children().find(|c| !c.text_range().is_empty())
}

/// The start of the first token of `n` that is not trivia (a node starts with its leading blanks).
fn code_start(n: &SyntaxNode) -> usize {
    n.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !matches!(t.kind(), k::WHITESPACE | k::NEWLINE | k::COMMENT))
        .map_or(n.text_range().start().into(), |t| {
            t.text_range().start().into()
        })
}

/// The blanks (with a line break) between the code before a branch and the branch.
fn break_before(n: &SyntaxNode, text: &str) -> Option<(usize, usize)> {
    let start = code_start(n);
    let mut at = start;
    let mut tok = n
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| usize::from(t.text_range().start()) == start)?
        .prev_token();
    while let Some(t) = tok {
        match t.kind() {
            k::WHITESPACE | k::NEWLINE => at = t.text_range().start().into(),
            // The branch stays below the comment.
            k::COMMENT => return Some((start, start)),
            _ => break,
        }
        tok = t.prev_token();
    }
    (text[at..start].contains('\n')).then_some((at, start))
}

fn wrap(n: &SyntaxNode, text: &str, edits: &mut Vec<(usize, usize, String)>) {
    if let Some(end) = last_code_end(n) {
        let start = code_start(n);
        match break_before(n, text) {
            Some((at, _)) => edits.push((at, start, " { ".into())),
            None => edits.push((start, start, "{ ".into())),
        }
        edits.push((end, end, " }".into()));
    }
}

/// The branches of the `if`/`else` chain that starts at `n`; empty when there is no `else`.
fn if_branches(n: &SyntaxNode) -> Vec<SyntaxNode> {
    let (mut out, mut cur) = (Vec::new(), n.clone());
    loop {
        out.extend(
            cur.children()
                .find(|c| c.kind() == k::THEN)
                .and_then(|b| content(&b)),
        );
        let Some(c) = cur
            .children()
            .find(|c| c.kind() == k::ELSE)
            .and_then(|b| content(&b))
        else {
            out.clear();
            return out;
        };
        if c.kind() == k::IF {
            cur = c;
        } else {
            out.push(c);
            return out;
        }
    }
}

/// The body of a loop when it stands on a line of its own.
fn loop_body(n: &SyntaxNode, text: &str) -> Option<SyntaxNode> {
    let body = n
        .children()
        .find(|c| c.kind() == k::BODY)
        .and_then(|b| content(&b))?;
    let close = n
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == k::RPAR)?;
    let gap = &text[usize::from(close.text_range().end())..code_start(&body)];
    (gap.contains('\n') && !braced(&body)).then_some(body)
}

/// Does the text from `start` to `end`, on one line, go past the line width?
fn too_long(text: &str, start: usize, end: usize, width: usize) -> bool {
    let column = start - text[..start].rfind('\n').map_or(0, |i| i + 1);
    let flat: usize = text[start..end]
        .split_whitespace()
        .map(|w| w.chars().count() + 1)
        .sum();
    column + flat > width + 1
}

/// Braces around the branches that have none, when another one has them or spans lines (or would,
/// on one line, go past the width), or the body of a loop is on a line of its own: `if`/`else`
/// chains, `when` entries, loops.
fn bracing(tree: &SyntaxNode, text: &str, width: usize, edits: &mut Vec<(usize, usize, String)>) {
    let multi = |n: &SyntaxNode| {
        (braced(n) && n.children().any(|c| !c.text_range().is_empty()))
            || last_code_end(n).is_some_and(|end| text[code_start(n)..end].contains('\n'))
            || break_before(n, text).is_some()
    };
    for n in tree.descendants() {
        let (branches, long): (Vec<SyntaxNode>, bool) = match n.kind() {
            k::IF if n.parent().is_none_or(|p| p.kind() != k::ELSE) => {
                let branches = if_branches(&n);
                let end = branches.last().and_then(last_code_end);
                let long = end.is_some_and(|e| too_long(text, code_start(&n), e, width));
                (branches, long)
            }
            k::WHEN => {
                let entries: Vec<_> = n.children().filter(|e| e.kind() == k::WHEN_ENTRY).collect();
                let long = entries.iter().any(|e| {
                    last_code_end(e).is_some_and(|end| too_long(text, code_start(e), end, width))
                });
                (
                    entries.iter().filter_map(|e| e.children().last()).collect(),
                    long,
                )
            }
            k::WHILE | k::FOR => (loop_body(&n, text).into_iter().collect(), true),
            _ => continue,
        };
        if !long && !branches.iter().any(multi) {
            continue;
        }
        for b in branches
            .iter()
            .filter(|b| !braced(b) && b.kind() != k::WHEN_CONDITION_EXPRESSION)
        {
            wrap(b, text, edits);
        }
    }
}

/// The source with the rewrites applied, or `None` when there is nothing to rewrite.
pub fn rewrite(tree: &SyntaxNode, text: &str, width: usize) -> Option<String> {
    let mut edits = Vec::new();
    sort_imports(tree, text, &mut edits);
    blank_before_kdoc(tree, text, &mut edits);
    enum_trailing_comma(tree, text, &mut edits);
    expression_body(tree, &mut edits);
    bracing(tree, text, width, &mut edits);
    if edits.is_empty() {
        return None;
    }
    edits.sort();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (s, e, with) in edits {
        out.push_str(&text[at..s]);
        out.push_str(&with);
        at = e;
    }
    out.push_str(&text[at..]);
    Some(out)
}
