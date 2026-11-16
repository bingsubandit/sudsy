//! Spacing between blocks on one line (sqlfluff's LT01, LT06): each block wants `Single` space,
//! `Touch` (none) or `Any` (as written) before and after; an `inline` variant also removes a line
//! break. Blocks claim the wishes of the nodes they start or end, and the innermost node that
//! holds both neighbours may ask for them to touch.

use super::seq::{NONE, Seq, is_comment};
use crate::generated::kind as k;
use core_syntax::SyntaxKind;

#[derive(Clone, Copy, PartialEq)]
enum Sp {
    Single,
    Touch,
    Any,
}

/// `(spacing, strip line breaks)`.
type Wish = (Sp, bool);

const SINGLE: Wish = (Sp::Single, false);
const TOUCH: Wish = (Sp::Touch, false);
const INLINE: Wish = (Sp::Touch, true);
const JOIN: Wish = (Sp::Single, true);
const ANY: Wish = (Sp::Any, false);

/// Is `parent` a common table expression? Its brackets stay on the line of the name and `AS`.
fn cte_bracket(seq: &Seq, parent: u32) -> bool {
    parent != NONE && seq.nodes[parent as usize].kind == k::COMMON_TABLE_EXPRESSION
}

/// What the block `i` asks for before and after.
fn wishes(seq: &Seq, i: usize) -> (Wish, Wish) {
    let (mut before, mut after) = (SINGLE, SINGLE);
    let (mut start, mut end) = (true, true);
    let mut n = seq.blocks[i].node;
    while n != NONE && (start || end) {
        let node = &seq.nodes[n as usize];
        start &= node.first == i as u32;
        end &= node.last == i as u32;
        if start {
            before = match node.kind {
                k::FUNCTION_CONTENTS
                | k::BRACKETED_ARGUMENTS
                | k::ARRAY_ACCESSOR
                | k::SEMI_STRUCTURED_EXPRESSION => INLINE,
                k::ALIAS_COLUMN_LIST => TOUCH,
                k::BRACKETED if cte_bracket(seq, node.parent) => JOIN,
                _ => before,
            };
        }
        n = node.parent;
    }
    match seq.blocks[i].kind {
        k::COMMA | k::SEMICOLON | k::R_PAREN | k::R_SQUARE | k::EOF | k::COLON => before = TOUCH,
        k::DOT | k::SLICE => (before, after) = (TOUCH, TOUCH),
        k::PREFIX => after = TOUCH,
        k::L_PAREN | k::L_SQUARE => after = TOUCH,
        k::CAST_OP => (before, after) = (TOUCH, INLINE),
        k::SIGN => after = INLINE,
        k::OPERATOR if seq.blocks[i].text == "@" => (before, after) = (TOUCH, TOUCH),
        k::OPERATOR if seq.blocks[i].node == 0 => before = TOUCH,
        k::LINE_COMMENT | k::BLOCK_COMMENT => (before, after) = (ANY, ANY),
        _ => {}
    }
    (before, after)
}

/// What the innermost node holding both blocks asks for.
fn within(kind: SyntaxKind) -> Option<Wish> {
    match kind {
        k::OBJECT_REFERENCE | k::FUNCTION_NAME | k::SEMI_STRUCTURED_EXPRESSION => Some(INLINE),
        k::COMPARISON_OPERATOR => Some(TOUCH),
        _ => None,
    }
}

/// Would the text of the two blocks fuse into other tokens with nothing between them?
fn fuses(prev: &str, next: &str, prev_is_sign: bool) -> bool {
    let (a, b) = (prev.chars().next_back(), next.chars().next());
    (a == Some('-') && b == Some('-'))
        || (prev_is_sign && b.is_some_and(|c| "+-*/<>=~!@#%^&|`?".contains(c)))
}

/// The bracket at block `i` after a function name or an object reference: does it touch the name
/// (`Some(true)`), or keep a space (`Some(false)`: a procedure's parameters)? Object references
/// are function names in `DROP FUNCTION`, `CREATE FUNCTION` and `DROP AGGREGATE`.
fn name_bracket(seq: &Seq, i: usize) -> Option<bool> {
    let prev = &seq.nodes[seq.blocks[i - 1].node as usize];
    if prev.last != i as u32 - 1 || !matches!(prev.kind, k::FUNCTION_NAME | k::OBJECT_REFERENCE) {
        return None;
    }
    let mut n = prev.parent;
    while n != NONE && seq.nodes[n as usize].kind != k::STATEMENT {
        n = seq.nodes[n as usize].parent;
    }
    let first = if n == NONE {
        0
    } else {
        seq.nodes[n as usize].first as usize
    };
    let head = |w: &str| {
        seq.blocks[first..(first + 4).min(seq.blocks.len())]
            .iter()
            .any(|b| b.text.eq_ignore_ascii_case(w))
    };
    if (head("procedure") || head("proc")) && head("create") {
        Some(false)
    } else if prev.kind == k::FUNCTION_NAME
        || head("function")
        || head("aggregate")
        || head("procedure")
    {
        Some(true)
    } else {
        None
    }
}

/// Is block `i` a `~` in front of its operand (not the binary `~` of a regex match)?
fn is_prefix_tilde(seq: &Seq, i: usize) -> bool {
    let b = &seq.blocks[i];
    b.kind == k::OPERATOR
        && b.text == "~"
        && (i == 0
            || !matches!(
                seq.blocks[i - 1].kind,
                k::IDENTIFIER
                    | k::QUOTED_IDENTIFIER
                    | k::NUMBER
                    | k::STRING
                    | k::R_PAREN
                    | k::R_SQUARE
                    | k::WORD_LITERAL
            ))
}

/// Sets the gap of every point that sits inside a line; returns whether anything changed.
pub fn respace(seq: &mut Seq) -> bool {
    (1..seq.blocks.len()).fold(false, |changed, i| respace_point(seq, i, false) | changed)
}

/// Sets the gap before block `i`; `strip` removes a line break whatever the blocks ask for.
pub fn respace_point(seq: &mut Seq, i: usize, strip: bool) -> bool {
    let (before, after) = (wishes(seq, i).0, wishes(seq, i - 1).1);
    let (mut pre, mut post) = (after, before);
    if seq.blocks[i].kind == k::L_PAREN {
        match name_bracket(seq, i) {
            Some(true) => post = INLINE,
            Some(false) => post = SINGLE,
            None => {}
        }
    }
    if seq.blocks[i].kind == k::L_SQUARE
        && matches!(
            seq.blocks[i - 1].kind,
            k::KEYWORD
                | k::IDENTIFIER
                | k::TYPE_IDENTIFIER
                | k::QUOTED_IDENTIFIER
                | k::R_SQUARE
                | k::R_PAREN
        )
    {
        post = TOUCH;
    }
    if seq.blocks[i - 1].text.eq_ignore_ascii_case("delimiter") {
        (pre, post) = (SINGLE, SINGLE);
    }
    if is_prefix_tilde(seq, i - 1) {
        if seq.blocks[i].text == "~" {
            (pre, post) = (ANY, ANY);
        } else {
            pre = TOUCH;
        }
    }
    let mut strip = strip || pre.1 || post.1;
    let common = seq.common_node(i - 1, i);
    if let Some((sp, inline)) = within(seq.nodes[common as usize].kind) {
        strip |= inline;
        if sp == Sp::Touch {
            pre.0 = if pre.0 == Sp::Any { Sp::Any } else { Sp::Touch };
            post.0 = if post.0 == Sp::Any {
                Sp::Any
            } else {
                Sp::Touch
            };
        }
    }
    if is_comment(seq.blocks[i - 1].kind) || is_comment(seq.blocks[i].kind) {
        strip = false;
    }
    let touch = pre.0 == Sp::Touch || post.0 == Sp::Touch;
    let any = pre.0 == Sp::Any || post.0 == Sp::Any;
    let fuse = (touch || any)
        && fuses(
            &seq.blocks[i - 1].text,
            &seq.blocks[i].text,
            seq.blocks[i - 1].kind == k::SIGN,
        );
    let strip = strip && !fuse;
    // The characters of one operator (`+=`, `||`) the parser lexed apart stay together.
    let op = |j: usize| {
        let b = &seq.blocks[j];
        (b.kind == k::OPERATOR || b.kind == k::BINARY_OPERATOR)
            && !b.text.chars().any(|c| c.is_alphanumeric() || c == '_')
    };
    let joined = op(i - 1) && op(i);
    let p = &mut seq.points[i];
    if joined && p.newlines == 0 && p.ws.is_empty() {
        return false;
    }
    if seq.blocks[i].kind == k::EOF {
        let changed = p.newlines == 0 && !p.ws.is_empty();
        if changed {
            p.set_ws("");
        }
        return changed;
    }
    if p.newlines > 0 && !strip {
        return false;
    }
    let had_break = p.newlines > 0;
    p.newlines = 0;
    if had_break {
        p.set_ws("");
    }
    let want = if any || (touch && fuse) {
        None
    } else {
        Some(if touch { "" } else { " " })
    };
    match want {
        Some(w) if p.ws != w => {
            p.set_ws(w);
            true
        }
        _ => had_break,
    }
}
