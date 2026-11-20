//! `curly`: the body of an `if`, `else`, loop or `when` entry that is a single statement is put in
//! braces. Style `always` (Checkstyle NeedBraces) braces every such body; style `chain` (ktlint
//! multiline-if-else, multiline loops) braces the branches of an `if`/`else` chain or of a
//! `when` when one of them has braces, spans lines or would pass the line width on one line, and
//! the body of a loop that stands on a line of its own. The nodes are found by the grammar tags
//! (`#conditional` with the slots `then` and `else`, `#loop` with `body`, `#switch` over `#case`
//! nodes with `body`); a `#statements` node around a branch is looked through. When the formatter runs the rule (`format_rules` lists it) the
//! style is the language's `curly_style` option, else the rule's `style` option.
//!
//! concept: curly-braces
//! what: Single-statement bodies of if, else, loops and when entries put in braces.
//! tools: Checkstyle NeedBraces, ktlint multiline-if-else
//! rules: curly
//! reads: tag:conditional, tag:loop, tag:switch, tag:case, tag:block, tag:statements, tag:empty-statement

use crate::diagnostic::Edit;
use crate::options::{Check, Opt, OptionDecl};
use crate::rule;
use crate::rule::{FileContext, RuleContext};
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, SyntaxToken, TextRange, TextSize};

rule! {
    pub Curly {
        id: "curly",
        category: Style,
        default: Off,
        fix: Safe,
        description: "The body of an `if`, `else`, loop or `when` entry is a statement without \
                  braces. Style `always` wants braces on every body; `chain` only on the branches \
                  of a chain where one has braces, spans lines or passes the line width, and on a \
                  loop body on its own line. `else if` is fine.",
        example: "if (x) return;",
        options: [
            OptionDecl {
                name: "style",
                default: Opt::Str("always"),
                check: Check::OneOf(&["always", "chain"]),
                description: "`always`: every body; `chain`: see the rule. The `style` row of \
                          the language's `curly_style` option overrides it in the formatter.",
            },
        ],
        format: true,
    }

    fn format_kinds(&self, grammar: &Grammar) -> Vec<core_syntax::SyntaxKind> {
        ["conditional", "loop", "switch"]
            .into_iter()
            .flat_map(|t| grammar.nodes_with_tag(t).iter().copied())
            .collect()
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let g = file.grammar;
        let style = match ctx.format.and_then(|f| f.options.get("curly_style")) {
            Some(core_config::OptionValue::String(s)) => s.as_str(),
            _ => ctx.options.string("style"),
        };
        let chain = style == "chain";
        let width = ctx
            .format
            .map_or(0, |f| usize::try_from(f.options.int("line_width")).unwrap_or(0));
        let kinds = self.format_kinds(g);
        for n in ctx.nodes(&kinds) {
            let mut edits = Vec::new();
            if g.has_tag(n.kind(), "conditional") {
                if g.slot(&n, "then").is_none() || (chain && in_chain(g, &n)) {
                    continue;
                }
                conditional(g, &n, file.text, chain, width, &mut edits);
            } else if g.has_tag(n.kind(), "loop") {
                loop_body(g, &n, file.text, chain, &mut edits);
            } else if chain {
                switch(g, &n, file.text, width, &mut edits);
            }
            if edits.is_empty() {
                continue;
            }
            if let Some(at) = code_start(g, &n) {
                ctx.report_fix(
                    TextRange::empty(TextSize::from(at as u32)),
                    "the body is not in braces",
                    ("Add the braces", edits),
                );
            }
        }
    }
}

fn trivia(g: &Grammar, t: &SyntaxToken) -> bool {
    g.is_trivia(t.kind())
}

fn code_tokens<'a>(g: &'a Grammar, n: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + 'a {
    n.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(move |t| !trivia(g, t))
}

fn code_start(g: &Grammar, n: &SyntaxNode) -> Option<usize> {
    code_tokens(g, n)
        .next()
        .map(|t| usize::from(t.text_range().start()))
}

fn code_end(g: &Grammar, n: &SyntaxNode) -> Option<usize> {
    code_tokens(g, n)
        .last()
        .map(|t| usize::from(t.text_range().end()))
}

/// What a branch (`then`, `else`, a loop body, a `when` entry's body) holds: the slot's node, or
/// the first thing in the `#statements` node around it.
fn content(g: &Grammar, slot: Option<SyntaxElement>) -> Option<SyntaxNode> {
    let node = slot?.into_node()?;
    if g.has_tag(node.kind(), "statements") {
        node.children().find(|c| !c.text_range().is_empty())
    } else {
        Some(node)
    }
}

fn braced(g: &Grammar, n: &SyntaxNode) -> bool {
    g.has_tag(n.kind(), "block") && code_tokens(g, n).next().is_some_and(|t| t.text() == "{")
}

/// An `else if`: the node is the `else` of another `if`.
fn in_chain(g: &Grammar, n: &SyntaxNode) -> bool {
    let mut parent = n.parent();
    if parent
        .as_ref()
        .is_some_and(|p| g.has_tag(p.kind(), "statements"))
    {
        parent = parent.and_then(|p| p.parent());
    }
    parent.is_some_and(|p| {
        g.has_tag(p.kind(), "conditional") && content(g, g.slot(&p, "else")).as_ref() == Some(n)
    })
}

/// The blanks (with a line break) between the code before `n` and `n`; with a comment in between,
/// the end of the code before it (the brace goes there and the comment ends up inside) and `true`.
fn break_before(g: &Grammar, n: &SyntaxNode, text: &str) -> Option<(usize, usize, bool)> {
    let start = code_start(g, n)?;
    let mut at = start;
    let mut commented = false;
    let mut tok = n
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| usize::from(t.text_range().start()) == start)?
        .prev_token();
    while let Some(t) = tok {
        if !trivia(g, &t) {
            break;
        }
        commented |= !t.text().trim().is_empty();
        at = t.text_range().start().into();
        tok = t.prev_token();
    }
    if commented {
        return Some((at, start, true));
    }
    text[at..start].contains('\n').then_some((at, start, false))
}

fn wrap(g: &Grammar, n: &SyntaxNode, text: &str, edits: &mut Vec<Edit>) {
    let (Some(start), Some(end)) = (code_start(g, n), code_end(g, n)) else {
        return;
    };
    let range =
        |a: usize, b: usize| TextRange::new(TextSize::from(a as u32), TextSize::from(b as u32));
    match break_before(g, n, text) {
        Some((at, _, true)) => edits.push(Edit::insert(TextSize::from(at as u32), " {")),
        Some((at, _, false)) => edits.push(Edit::replace(range(at, start), " { ")),
        None => edits.push(Edit::insert(TextSize::from(start as u32), "{ ")),
    }
    // A line comment after the body stays on its line: the brace follows below it.
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
    let commented = g
        .comments
        .line
        .as_deref()
        .is_some_and(|m| text[end..line_end].trim_start().starts_with(m));
    if commented {
        edits.push(Edit::insert(TextSize::from(line_end as u32), "\n}"));
    } else {
        edits.push(Edit::insert(TextSize::from(end as u32), " }"));
    }
}

/// Does the text from `start` to `end`, on one line, go past the line width?
fn too_long(text: &str, start: usize, end: usize, width: usize) -> bool {
    let column = start - text[..start].rfind('\n').map_or(0, |i| i + 1);
    let flat: usize = text[start..end]
        .split_whitespace()
        .map(|w| w.chars().count() + 1)
        .sum();
    width > 0 && column + flat > width + 1
}

fn multi(g: &Grammar, n: &SyntaxNode, text: &str) -> bool {
    (braced(g, n) && n.children().any(|c| !c.text_range().is_empty()))
        || code_start(g, n)
            .zip(code_end(g, n))
            .is_some_and(|(s, e)| text[s..e].contains('\n'))
        || break_before(g, n, text).is_some()
}

fn bracing(g: &Grammar, branches: &[SyntaxNode], text: &str, must: bool, edits: &mut Vec<Edit>) {
    if !must && !branches.iter().any(|b| multi(g, b, text)) {
        return;
    }
    for b in branches
        .iter()
        .filter(|b| !braced(g, b) && !g.has_tag(b.kind(), "empty-statement"))
    {
        wrap(g, b, text, edits);
    }
}

fn conditional(
    g: &Grammar,
    n: &SyntaxNode,
    text: &str,
    chain: bool,
    width: usize,
    edits: &mut Vec<Edit>,
) {
    let (mut branches, mut cur) = (Vec::new(), n.clone());
    loop {
        branches.extend(content(g, g.slot(&cur, "then")));
        let Some(next) = content(g, g.slot(&cur, "else")) else {
            // (a chain that ends in an `else if` without `else` is left alone)
            if chain && cur != *n {
                branches.clear();
            }
            break;
        };
        if g.has_tag(next.kind(), "conditional") {
            if !chain {
                break;
            }
            cur = next;
        } else {
            branches.push(next);
            break;
        }
    }
    // (a lone `if` is only braced for a branch of its own line: the layout decides the width)
    let long = chain
        && branches.len() > 1
        && branches
            .last()
            .and_then(|b| code_end(g, b))
            .zip(code_start(g, n))
            .is_some_and(|(e, s)| too_long(text, s, e, width));
    bracing(g, &branches, text, !chain || long, edits);
}

/// A loop body: every one for `always`, one that stands on a line of its own for `chain`.
fn loop_body(g: &Grammar, n: &SyntaxNode, text: &str, chain: bool, edits: &mut Vec<Edit>) {
    let Some(body) = content(g, g.slot(n, "body")) else {
        return;
    };
    if braced(g, &body) || g.has_tag(body.kind(), "empty-statement") {
        return;
    }
    let after_paren = code_start(g, &body)
        .and_then(|s| {
            let first = body
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| usize::from(t.text_range().start()) == s)?;
            let mut prev = first.prev_token();
            while let Some(t) = prev.clone().filter(|t| trivia(g, t)) {
                prev = t.prev_token();
            }
            prev.map(|t| (t.text() == ")", usize::from(t.text_range().end()), s))
        })
        .filter(|(close, _, _)| *close);
    if !chain || after_paren.is_some_and(|(_, end, s)| text[end..s].contains('\n')) {
        wrap(g, &body, text, edits);
    }
}

/// The entries of a `when`.
fn switch(g: &Grammar, n: &SyntaxNode, text: &str, width: usize, edits: &mut Vec<Edit>) {
    let entries: Vec<_> = n
        .children()
        .filter(|e| g.has_tag(e.kind(), "case"))
        .collect();
    let long = entries.iter().any(|e| {
        code_start(g, e)
            .zip(code_end(g, e))
            .is_some_and(|(s, end)| too_long(text, s, end, width))
    });
    let bodies: Vec<_> = entries
        .iter()
        .filter_map(|e| content(g, g.slot(e, "body")))
        .collect();
    bracing(g, &bodies, text, long, edits);
}
