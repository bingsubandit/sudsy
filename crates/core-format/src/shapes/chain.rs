//! concept: member-chain
//! what: Member chains broken by call count, head shape and receiver kind.
//! tools: Prettier member-chain, google-java-format dot chains
//! reads: option:member_chain_min_calls

use crate::FormatCtx;
use crate::comments::{self, CommentKind};
use crate::doc::{self, Doc};
use core_syntax::SyntaxToken;

/// Does a line comment follow this token on its line? Whatever comes next must then start a new
/// line, or the comment would move to the end of a later line and change owner.
pub fn ends_in_line_comment(cx: &FormatCtx<'_>, tok: &SyntaxToken) -> bool {
    comments::trailing_comments(cx.grammar, tok)
        .iter()
        .any(|c| c.kind != CommentKind::Block)
}

/// The space after an operator, or a line break when a line comment ends its line.
pub fn gap_after_operator(cx: &FormatCtx<'_>, op: &SyntaxToken) -> Doc {
    if ends_in_line_comment(cx, op) {
        doc::hardline()
    } else {
        doc::lit(" ")
    }
}

/// The pieces of a chain of member accesses and calls, already laid out.
pub struct MemberChain {
    /// The head and whatever stays on its line.
    pub head: Vec<Doc>,
    /// The links from each dot to the next, one group per line when the chain breaks.
    pub groups: Vec<Vec<Doc>>,
    /// Calls in the chain, counted against `min_calls`.
    pub calls: usize,
    /// A chain with fewer calls is never broken at the dots.
    pub min_calls: usize,
    /// Break at the dots whatever the width (a line comment sits between the links).
    pub force_break: bool,
    /// Break at the dots when a group other than the last has a forced break inside it (a call
    /// with a block lambda).
    pub break_on_inner_break: bool,
    /// The forced break may also be in the head or the last group (a chain that breaks as a whole
    /// when anything in it breaks).
    pub inner_break_anywhere: bool,
    /// Levels of indentation of the broken groups.
    pub indent: usize,
}

/// `head.A().B().C()` on one line if it fits, else the head and then one group per line,
/// indented.
pub fn member_chain(chain: MemberChain) -> Doc {
    let MemberChain {
        head,
        groups,
        calls,
        min_calls,
        force_break,
        break_on_inner_break,
        inner_break_anywhere,
        indent,
    } = chain;
    // A short chain is never broken at the dots, unless a call in the middle has a block lambda.
    let short = calls < min_calls && !force_break;
    if groups.is_empty() || (short && !(break_on_inner_break && groups.len() >= 2)) {
        let mut parts = head;
        parts.extend(groups.into_iter().flatten());
        return doc::concat(parts);
    }
    // Both candidates are built from the same parts: share them, or a chain nested in a chain
    // would copy its parts once more at every level.
    let head = doc::shared(doc::concat(head));
    let groups: Vec<Doc> = groups
        .into_iter()
        .map(|g| doc::shared(doc::concat(g)))
        .collect();
    let inner_break = break_on_inner_break
        && if inner_break_anywhere && !groups.is_empty() {
            head.clone().propagate_breaks() || groups.iter().any(|g| g.clone().propagate_breaks())
        } else {
            groups
                .split_last()
                .is_some_and(|(_, init)| init.iter().any(|g| g.clone().propagate_breaks()))
        };
    if short && !inner_break {
        let mut parts = vec![head];
        parts.extend(groups);
        return doc::concat(parts);
    }
    let force_break = force_break || inner_break;
    let flat = (!force_break).then(|| {
        let mut parts = vec![head.clone()];
        parts.extend(groups.iter().cloned());
        doc::concat(parts)
    });
    let mut tail = Vec::new();
    for g in groups {
        tail.push(doc::hardline());
        tail.push(g);
    }
    let mut tail = doc::concat(tail);
    for _ in 0..indent {
        tail = doc::indent(tail);
    }
    let broken = doc::concat(vec![head, tail]);
    match flat {
        Some(flat) => doc::conditional_group(vec![flat, broken]),
        None => broken,
    }
}
