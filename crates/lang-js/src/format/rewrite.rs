//! Changes to the source text that the safety check could not follow if the formatter made them:
//! the generic check compares nodes and tokens, so a token the output adds or drops is added or
//! dropped here, and the tree is read again (`NodeFormatter::rewrite_source`).
//! - Parentheses around a type that its place does not need, and a leading `&` or lone `|` of a
//!   type (`format/ts/parens.rs`).
//! - An arrow's lone parameter gets its parentheses.
//!
//! The semicolons that mean nothing (`};`, `;;`, a `;` after a class method) and `new Foo()` are
//! rows of `format.rules` (`[rewrite]`).
//!
//! concept: new source-rewrite
//! what: Edits to the source text made before layout that the safety check cannot follow.
//! tools: Prettier, ktlint rewrites, Black

use crate::generated::kind as k;
use core_config::ResolvedOptions;
use core_format::comments;
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

/// A cut of the byte at `.0` or an insertion of `.1` before it.
type Edit = (usize, Option<&'static str>);

/// The text with the edits applied; `None` when there is none.
pub(crate) fn rewrite(
    tree: &SyntaxNode,
    text: &str,
    grammar: &Grammar,
    options: &ResolvedOptions,
    rows: &'static core_format::rules::RuleFormatter,
) -> Option<String> {
    let avoid_parens = super::arrow_parens(options) == super::ArrowParens::Avoid;
    let mut edits: Vec<Edit> = Vec::new();
    // Comments in front of a cut token lead what follows it; ones after it would move.
    let mut cut = |tok: SyntaxToken| {
        edits.push((usize::from(tok.text_range().start()), None));
    };
    let mut inserts: Vec<Edit> = Vec::new();
    let mut later: Vec<Edit> = Vec::new();
    // Text moved from elsewhere: where it goes and what.
    let mut moved: Vec<(usize, String)> = Vec::new();
    let keys = super::object::KeyEdits::new(options);
    let mut key_edits: Vec<Edit> = Vec::new();
    let mut scan = rows.rewrite_scan();
    for node in tree.descendants() {
        scan.visit(&node);
        keys.of(&node, text, grammar, &mut key_edits);
        // `) // c` then `{` on the next line becomes `) { // c`: the comment moves inside the block.
        if matches!(
            node.kind(),
            k::BLOCK_STATEMENT | k::FUNCTION_BODY | k::SWITCH_STATEMENT
        ) && let Some(open) = (if node.kind() == k::SWITCH_STATEMENT {
            node.children_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| t.kind() == k::L_BRACE)
        } else {
            first_token(grammar, &node)
        }) && open.kind() == k::L_BRACE
            && comments::trailing_comments(grammar, &open).is_empty()
        {
            let mut between = Vec::new();
            let mut prev = open.prev_token();
            while let Some(t) = prev.clone().filter(|t| grammar.is_trivia(t.kind())) {
                prev = t.prev_token();
                between.push(t);
            }
            // Block comments in front of the first line comment stay in front of the `{`.
            between.reverse();
            let first_line = between.iter().position(|t| t.kind() == k::LINE_COMMENT);
            let after = between
                .iter()
                .take(first_line.unwrap_or(0))
                .rfind(|t| t.kind() == k::BLOCK_COMMENT)
                .map(|t| usize::from(t.text_range().end()));
            if let Some(prev) = prev
                && first_line.is_some()
                && between[first_line.unwrap_or(0)..]
                    .iter()
                    .all(|t| t.kind() != k::BLOCK_COMMENT)
            {
                let brace = if node.kind() == k::TS_INTERFACE_BODY {
                    " {
"
                } else {
                    " {"
                };
                inserts.push((
                    after.unwrap_or(usize::from(prev.text_range().end())),
                    Some(brace),
                ));
                // The line break after the old `{` goes too, or a blank line would follow.
                if let Some(next) = open.next_token().filter(|t| t.kind() == k::NEWLINE) {
                    let at = usize::from(next.text_range().start());
                    let len = if next.text().starts_with("\r\n") {
                        2
                    } else {
                        1
                    };
                    (0..len).for_each(|i| later.push((at + i, None)));
                }
                cut(open);
            }
        }
        // `class A // c` then `extends B {`: the line comments of the header go on lines of
        // their own inside the body, so that formatting the output keeps them there.
        if matches!(node.kind(), k::CLASS_BODY | k::TS_INTERFACE_BODY)
            && let Some(open) = first_token(grammar, &node)
            && open.kind() == k::L_BRACE
            && let Some(decl) = node.parent()
        {
            // The node starts with the trivia in front of its first token: those are not header.
            let Some(start) = first_token(grammar, &decl).map(|t| t.text_range().start()) else {
                continue;
            };
            let mut header = Vec::new();
            let mut t = open.prev_token();
            while let Some(tok) = t {
                if tok.text_range().start() < start {
                    break;
                }
                if tok.kind() == k::LINE_COMMENT
                    && !tok.parent_ancestors().take_while(|a| *a != decl).any(|a| {
                        matches!(
                            a.kind(),
                            k::TS_TYPE_LITERAL
                                | k::TS_MAPPED_TYPE
                                | k::OBJECT_EXPRESSION
                                | k::FUNCTION_BODY
                                | k::BLOCK_STATEMENT
                        )
                    })
                {
                    header.push(tok.clone());
                }
                t = tok.prev_token();
            }
            if !header.is_empty() {
                header.reverse();
                let mut text = String::new();
                for c in &header {
                    text.push('\n');
                    text.push_str(c.text().trim_end());
                    let from = usize::from(c.text_range().start());
                    (from..usize::from(c.text_range().end())).for_each(|b| later.push((b, None)));
                }
                // An empty body `{}` needs the line break, or the comment would swallow `}`.
                if !open.next_token().is_some_and(|t| t.kind() == k::NEWLINE) {
                    text.push('\n');
                }
                moved.push((usize::from(open.text_range().end()), text));
            }
        }
        // `interface A { // c` puts the comment on a line of its own inside the body.
        if matches!(node.kind(), k::TS_INTERFACE_BODY | k::TS_TYPE_LITERAL)
            && let Some(open) = first_token(grammar, &node)
            && open.kind() == k::L_BRACE
            && comments::trailing_comments(grammar, &open)
                .first()
                .is_some_and(|c| c.kind == comments::CommentKind::Line)
        {
            inserts.push((usize::from(open.text_range().end()), Some("\n")));
        }
        // `a && (b && c)` is `a && b && c`: parentheses around a right operand of the same
        // logical operator go (the tree then has the shape of the output).
        if node.kind() == k::PARENTHESIZED_EXPRESSION
            && let Some(parent) = node.parent().filter(|p| p.kind() == k::LOGICAL_EXPRESSION)
            && parent.last_child().as_ref() == Some(&node)
            && let Some(inner) = node
                .first_child()
                .filter(|c| c.kind() == k::LOGICAL_EXPRESSION)
            && logical_operator(&parent) == logical_operator(&inner)
            && logical_operator(&inner).is_some()
        {
            node.children_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| matches!(t.kind(), k::L_PAREN | k::R_PAREN))
                .for_each(&mut cut);
        }
        // `(x!) = y` is `x! = y`: the parentheses of an assignment target stay only where the
        // printer adds them back (a cast).
        if matches!(node.kind(), k::ASSIGNMENT_EXPRESSION | k::UPDATE_EXPRESSION)
            && node.first_child().is_some_and(|t| {
                !matches!(
                    t.kind(),
                    k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION | k::TS_TYPE_ASSERTION
                )
            })
        {
            node.children_with_tokens()
                .filter_map(|e| e.into_token())
                .take_while(|t| node.kind() == k::UPDATE_EXPRESSION || t.kind() != k::OPERATOR)
                .filter(|t| matches!(t.kind(), k::L_PAREN | k::R_PAREN))
                .for_each(&mut cut);
        }
        // `for ((a) of b)` is `for (a of b)`: the parentheses of the left side go. `let` at the
        // start of the left side takes its own (`for ((let.a) of b)` is `for ((let).a of b)`).
        if matches!(node.kind(), k::FOR_IN_STATEMENT | k::FOR_OF_STATEMENT)
            && let Some(left) = node.first_child()
            && left.kind() != k::IDENTIFIER_REFERENCE
        {
            let tokens: Vec<_> = node
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .collect();
            let keyword = tokens
                .iter()
                .find(|t| {
                    t.text_range().start() >= left.text_range().end()
                        && matches!(t.text(), "of" | "in")
                })
                .map(|t| t.text_range().start());
            let first = first_token(grammar, &left);
            if let Some(keyword) = keyword {
                let mut head_open = false;
                let mut cuts = 0;
                for t in tokens {
                    if t.kind() == k::L_PAREN && !std::mem::replace(&mut head_open, true) {
                        continue;
                    }
                    if matches!(t.kind(), k::L_PAREN | k::R_PAREN)
                        && t.text_range().start() < keyword
                    {
                        cuts += 1;
                        cut(t);
                    }
                }
                if let Some(first) = first.filter(|f| f.text() == "let" && cuts > 0) {
                    inserts.push((usize::from(first.text_range().start()), Some("(")));
                    inserts.push((usize::from(first.text_range().end()), Some(")")));
                }
            }
        }
        if let Some((from, to)) = core_format::rewrite::parenthesize(grammar, options, &node) {
            inserts.push((from, Some("(")));
            inserts.push((to, Some(")")));
        }
        match node.kind() {
            k::TS_PARENTHESIZED_TYPE | k::TS_UNION_TYPE | k::TS_INTERSECTION_TYPE => {
                for t in super::ts::redundant_type_tokens(&node, grammar) {
                    // The blank after a bar or `&` goes with it (`/*1*/ | /*2*/ C`).
                    let blank = matches!(t.text(), "|" | "&")
                        .then(|| t.next_token().filter(|n| n.text() == " "))
                        .flatten();
                    cut(t);
                    if let Some(b) = blank {
                        cut(b);
                    }
                }
            }
            // `(): () => void => x`: a function type returned by an arrow takes parentheses.
            k::TS_FUNCTION_TYPE
                if node
                    .parent()
                    .filter(|p| p.kind() == k::TS_TYPE_ANNOTATION)
                    .and_then(|p| p.parent())
                    .is_some_and(|a| a.kind() == k::ARROW_FUNCTION_EXPRESSION) =>
            {
                if let (Some(first), Some(last)) =
                    (first_token(grammar, &node), last_token(grammar, &node))
                {
                    inserts.push((usize::from(first.text_range().start()), Some("(")));
                    inserts.push((usize::from(last.text_range().end()), Some(")")));
                }
            }
            // `x()\n// c\n;` is `x();` with the comment after it (also for `break`, `continue`, `debugger`).
            k::EXPRESSION_STATEMENT
            | k::BREAK_STATEMENT
            | k::CONTINUE_STATEMENT
            | k::DEBUGGER_STATEMENT => {
                let semi = node
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| t.kind() == k::SEMICOLON);
                if let Some(semi) = semi
                    && comments::trailing_comments(grammar, &semi).is_empty()
                {
                    let mut prev = semi.prev_token();
                    while let Some(t) = prev.clone().filter(|t| grammar.is_trivia(t.kind())) {
                        prev = t.prev_token();
                    }
                    // (a line comment after the last token is the same case)
                    if let Some(prev) = prev
                        && (!comments::leading_comments(grammar, &semi).is_empty()
                            || comments::trailing_comments(grammar, &prev)
                                .iter()
                                .any(|c| c.kind == comments::CommentKind::Line))
                    {
                        inserts.push((usize::from(prev.text_range().end()), Some(";")));
                        cut(semi);
                    }
                }
            }
            // `typeof X[number]` is written `(typeof X)[number]`.
            k::TS_INDEXED_ACCESS_TYPE
                if node
                    .first_child()
                    .is_some_and(|c| c.kind() == k::TS_TYPE_QUERY) =>
            {
                if let (Some(first), Some(last)) = (
                    node.first_child().and_then(|c| first_token(grammar, &c)),
                    node.first_child().and_then(|c| last_token(grammar, &c)),
                ) {
                    inserts.push((usize::from(first.text_range().start()), Some("(")));
                    inserts.push((usize::from(last.text_range().end()), Some(")")));
                }
            }
            k::FORMAL_PARAMETERS
                if avoid_parens
                    && node.parent().is_some_and(|p| {
                        p.kind() == k::ARROW_FUNCTION_EXPRESSION
                            && !p.children().any(|c| {
                                matches!(
                                    c.kind(),
                                    k::TS_TYPE_ANNOTATION | k::TS_TYPE_PARAMETER_DECLARATION
                                )
                            })
                    })
                    && plain_lone_param(&node, grammar) =>
            {
                for t in node.children_with_tokens().filter_map(|e| e.into_token()) {
                    let at = usize::from(t.text_range().start());
                    // `async(x)` must not become `asyncx`.
                    if t.kind() == k::L_PAREN
                        && text[..at]
                            .chars()
                            .next_back()
                            .is_some_and(char::is_alphanumeric)
                    {
                        inserts.push((at, Some(" ")));
                    }
                    cut(t);
                }
            }
            k::FORMAL_PARAMETERS
                if !avoid_parens
                    && node
                        .parent()
                        .is_some_and(|p| p.kind() == k::ARROW_FUNCTION_EXPRESSION)
                    && !node
                        .children_with_tokens()
                        .any(|e| e.as_token().is_some_and(|t| t.kind() == k::L_PAREN)) =>
            {
                if let (Some(first), Some(last)) =
                    (first_token(grammar, &node), last_token(grammar, &node))
                {
                    // A line `(a) => 1` would continue the line before it: a statement of its
                    // own after one without `;` gets one.
                    if continues_previous_line(grammar, &node) {
                        inserts.push((usize::from(first.text_range().start()), Some(";")));
                    }
                    inserts.push((usize::from(first.text_range().start()), Some("(")));
                    inserts.push((usize::from(last.text_range().end()), Some(")")));
                }
            }
            _ => {}
        }
    }
    for (from, to, with) in scan.edits(grammar, tree) {
        edits.extend((from..to).map(|at| (at, None)));
        if !with.is_empty() {
            edits.push((from, Some(with)));
        }
    }
    edits.extend(inserts);
    edits.extend(later);
    edits.extend(key_edits);
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|e| (e.0, e.1.is_none()));
    moved.sort_by_key(|m| m.0);
    let mut out = String::with_capacity(text.len() + 8);
    let mut from = 0;
    let mut moved = moved.into_iter().peekable();
    for (at, insert) in edits {
        while let Some((to, add)) = moved.next_if(|m| m.0 <= at) {
            out.push_str(&text[from.min(to)..to]);
            from = from.max(to);
            out.push_str(&add);
        }
        out.push_str(&text[from.min(at)..at]);
        from = at;
        match insert {
            Some(s) => out.push_str(s),
            None => from = at + 1,
        }
    }
    for (to, add) in moved {
        out.push_str(&text[from.min(to)..to]);
        from = from.max(to);
        out.push_str(&add);
    }
    out.push_str(&text[from..]);
    Some(out)
}

/// `(x)`: parentheses around one plain name, with nothing else (no type, default, comment or
/// comma) between them.
fn plain_lone_param(params: &SyntaxNode, grammar: &Grammar) -> bool {
    let mut parts = params
        .children_with_tokens()
        .filter(|e| !grammar.is_trivia(e.kind()));
    let tok = |e: Option<core_syntax::SyntaxElement>, kind| {
        e.and_then(|e| e.into_token()).is_some_and(|t| {
            t.kind() == kind
                && comments::leading_comments(grammar, &t).is_empty()
                && comments::trailing_comments(grammar, &t).is_empty()
        })
    };
    tok(parts.next(), k::L_PAREN)
        && parts.next().and_then(|e| e.into_node()).is_some_and(|p| {
            p.kind() == k::FORMAL_PARAMETER
                && p.children_with_tokens()
                    .filter(|e| !grammar.is_trivia(e.kind()))
                    .count()
                    == 1
                && p.first_child()
                    .is_some_and(|c| c.kind() == k::BINDING_IDENTIFIER)
        })
        && tok(parts.next(), k::R_PAREN)
        && parts.next().is_none()
}

pub(super) fn first_token(grammar: &Grammar, node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !grammar.is_trivia(t.kind()))
}

fn last_token(grammar: &Grammar, node: &SyntaxNode) -> Option<SyntaxToken> {
    let start = node.text_range().start();
    let mut cur = node.last_token();
    while let Some(t) = cur {
        if !grammar.is_trivia(t.kind()) {
            return Some(t);
        }
        cur = t.prev_token().filter(|p| p.text_range().start() >= start);
    }
    None
}

/// The operator token of a logical expression.
fn logical_operator(node: &SyntaxNode) -> Option<String> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == k::OPERATOR)
        .map(|t| t.text().to_string())
}

/// Does `params` (of an arrow) start a statement that follows one without a final `;`?
fn continues_previous_line(grammar: &Grammar, params: &SyntaxNode) -> bool {
    let Some(arrow) = params.parent() else {
        return false;
    };
    let Some(stmt) = arrow
        .parent()
        .filter(|p| p.kind() == k::EXPRESSION_STATEMENT)
        .filter(|p| first_token(grammar, p) == first_token(grammar, &arrow))
    else {
        return false;
    };
    stmt.prev_sibling().is_some_and(|prev| {
        matches!(
            prev.kind(),
            k::EXPRESSION_STATEMENT | k::VARIABLE_DECLARATION
        ) && last_token(grammar, &prev).is_some_and(|t| t.text() != ";")
    })
}
