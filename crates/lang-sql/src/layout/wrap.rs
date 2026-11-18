//! Long lines (sqlfluff's LT05): the places a line may break are its indent points (by indent
//! balance) and the operators and commas around it (between the balances, by precedence); the
//! outermost are broken. A trailing line comment moves to the line before.
//!
//! concept: new line-wrapping
//! what: Long lines broken at indent points and operators.
//! tools: sqlfluff LT05

use super::breaks::{self, Pos, Span};
use super::seq::{NONE, Seq};
use crate::generated::kind as k;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

/// The first block of every line, and the end of the last.
fn line_starts(seq: &Seq) -> Vec<usize> {
    let mut out = vec![0];
    out.extend((1..seq.blocks.len()).filter(|&i| seq.points[i].newlines > 0));
    out.push(seq.blocks.len() - 1);
    out
}

fn width(seq: &Seq, s: usize, e: usize, tab: usize) -> usize {
    (s..e)
        .map(|i| {
            let ws = &seq.points[i].ws;
            let w = if i == s || seq.points[i].newlines == 0 {
                ws.chars().map(|c| if c == '\t' { tab } else { 1 }).sum()
            } else {
                0
            };
            w + seq.blocks[i].text.chars().count()
        })
        .sum()
}

/// A place that wants line breaks around it: the blocks `start..=end`, where the breaks go and
/// how late they are taken (a higher `priority` is broken after the lower ones).
struct Rebreak {
    start: usize,
    end: usize,
    pos: Pos,
    priority: i32,
    rank: u8,
}

/// The precedence of a break next to an operator or comma span.
fn priority(seq: &Seq, sp: &Span) -> i32 {
    let b = &seq.blocks[sp.s];
    if seq.nodes[b.node as usize].kind == k::COMPARISON_OPERATOR {
        return 5;
    }
    match b.text.to_ascii_uppercase().as_str() {
        "," => 1,
        "OR" => 3,
        "AND" => 4,
        "*" | "/" | "%" => 7,
        _ => 6,
    }
}

/// Operators, commas and set operators, and the clauses that sqlfluff's layout config wants alone
/// on their lines (`ORDER BY` leading), in the order of their first block.
fn rebreaks(seq: &Seq, comma: Pos) -> Vec<Rebreak> {
    let mut out: Vec<Rebreak> = breaks::spans(seq, comma)
        .iter()
        .map(|sp| Rebreak {
            start: sp.s,
            end: sp.e,
            pos: sp.pos,
            priority: priority(seq, sp),
            rank: 0,
        })
        .collect();
    for n in &seq.nodes {
        let pos = match n.kind {
            k::SELECT_CLAUSE
            | k::WHERE_CLAUSE
            | k::FROM_CLAUSE
            | k::JOIN_CLAUSE
            | k::GROUPBY_CLAUSE
            | k::HAVING_CLAUSE
            | k::LIMIT_CLAUSE => Pos::Alone,
            k::ORDERBY_CLAUSE => Pos::Leading,
            _ => continue,
        };
        if n.first != NONE {
            out.push(Rebreak {
                start: n.first as usize,
                end: n.last as usize,
                pos,
                priority: 6,
                rank: 1,
            });
        }
    }
    out.sort_by_key(|r| (r.start, r.rank));
    out
}

/// Do the blocks around the point `j` make up one operator the parser lexed apart (`-|-`)?
fn fused(seq: &Seq, j: usize) -> bool {
    let op = |i: usize| {
        let b = &seq.blocks[i];
        matches!(b.kind, k::OPERATOR | k::BINARY_OPERATOR)
            && !b.text.chars().any(|c| c.is_alphanumeric() || c == '_')
    };
    j > 0 && seq.points[j].newlines == 0 && seq.points[j].ws.is_empty() && op(j - 1) && op(j)
}

/// Is the point before block `j` followed on its line by nothing but a `;`?
fn before_terminator(seq: &Seq, j: usize, e: usize) -> bool {
    let mut found = false;
    for b in j..e {
        if seq.blocks[b].kind == k::SEMICOLON {
            found = true;
        } else if !seq.is_comment(b) {
            return false;
        }
    }
    found
}

/// The places to break the line of blocks `s..e` (and the point `e` that closes it), by key:
/// 100 per indent balance, and between the balances the breaks next to operators and commas.
fn matched(seq: &Seq, s: usize, e: usize, spans: &[Rebreak]) -> BTreeMap<i32, Vec<usize>> {
    let mut rebreak: HashMap<usize, i32> = HashMap::new();
    // sqlfluff looks for spans from the second block of the line on, and needs their end on it.
    for sp in spans
        .iter()
        .filter(|sp| sp.start > s && sp.start < e && sp.end < e)
    {
        if matches!(sp.pos, Pos::Leading | Pos::Alone) {
            rebreak.insert(sp.start, sp.priority);
        }
        if matches!(sp.pos, Pos::Trailing | Pos::Alone) {
            rebreak.insert(sp.end + 1, sp.priority);
        }
    }
    let mut out: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    let mut balance = 0i32;
    for j in (if s == 0 { 0 } else { s + 1 })..=e {
        let (impulse, trough) = seq.points[j].stats();
        let mut keys: Vec<i32> = Vec::new();
        if trough < 0 {
            keys.extend((trough + 1..=0).map(|b| (balance + b) * 100));
            balance += impulse;
        } else if impulse > 0 {
            keys.extend((0..impulse).map(|b| (balance + b + 1) * 100));
            balance += impulse;
        }
        if before_terminator(seq, j, e) || fused(seq, j) {
            continue;
        }
        for key in keys {
            out.entry(key).or_default().push(j);
        }
        if let Some(p) = rebreak.get(&j) {
            out.entry(balance * 100 + 50 + p).or_default().push(j);
        }
    }
    out.retain(|_, v| v.as_slice() != [e]);
    out
}

fn indent_to(seq: &mut Seq, at: usize, indent: &str) {
    let p = &mut seq.points[at];
    p.newlines = p.newlines.max(1);
    p.ws = Cow::Owned(indent.to_string());
}

/// Breaks at the indent points: those that the line reaches before the width are skipped, up to
/// the last one that starts a section.
fn break_at_indents(
    seq: &mut Seq,
    s: usize,
    targets: &[usize],
    max: usize,
    outer: &str,
    inner: &str,
) {
    let start_col = |seq: &Seq, j: usize| -> usize {
        outer.chars().count()
            + (s..j)
                .map(|i| seq.blocks[i].text.chars().count())
                .sum::<usize>()
            + (s + 1..=j)
                .map(|i| seq.points[i].ws.chars().count())
                .sum::<usize>()
            + 1
    };
    let mut purge_before = 0;
    for &t in targets {
        if start_col(seq, t) > max {
            break;
        }
        if seq.points[t].stats().1 < 0 {
            continue;
        }
        purge_before = t;
    }
    for &t in targets.iter().filter(|&&t| t >= purge_before) {
        let (impulse, trough) = seq.points[t].stats();
        let indent = if impulse < 0 {
            if matches!(seq.blocks[t].kind, k::SEMICOLON | k::COMMA) {
                break;
            }
            outer
        } else {
            inner
        };
        indent_to(seq, t, indent);
        if trough < 0 {
            break;
        }
    }
}

/// Moves the trailing comment block `c` to a line of its own before the line that starts at `s`.
fn lift_comment(seq: &mut Seq, s: usize, c: usize) {
    let tail = seq.points.remove(c);
    if c < seq.points.len() {
        let mut metas = tail.metas;
        metas.append(&mut seq.points[c].metas);
        seq.points[c].metas = metas;
    }
    let comment = seq.blocks.remove(c);
    seq.blocks.insert(s, comment);
    let first = std::mem::take(&mut seq.points[s]);
    seq.points.insert(s, first.clone());
    let p = &mut seq.points[s + 1];
    p.newlines = 1;
    p.ws = first.ws;
    p.metas.clear();
    for n in &mut seq.nodes {
        for x in [&mut n.first, &mut n.last] {
            if *x != NONE && (s as u32..c as u32).contains(x) {
                *x += 1;
            }
        }
    }
    seq.gaps = super::space::gaps(seq);
}

/// Fixes every line that is too long and can be (from the end, so indexes stay valid); returns
/// whether it fixed any.
pub fn long_lines(seq: &mut Seq, max: usize, tab: usize, unit: &str, comma: Pos) -> bool {
    if max == 0 {
        return false;
    }
    let starts = line_starts(seq);
    let mut spans = rebreaks(seq, comma);
    let mut changed = false;
    for w in starts.windows(2).rev() {
        let (s, e) = (w[0], w[1]);
        if e - s < 2 || width(seq, s, e, tab) <= max {
            continue;
        }
        let indent = if s == 0 && seq.points[0].newlines == 0 {
            String::new()
        } else {
            seq.points[s].ws.to_string()
        };
        let last = e - 1;
        if seq.blocks[last].kind == k::LINE_COMMENT
            && seq.points[last].newlines == 0
            && !seq.is_comment(s)
        {
            let text = &seq.blocks[last].text;
            if !text.contains("noqa") && text.chars().count() + indent.chars().count() <= max {
                lift_comment(seq, s, last);
                spans = rebreaks(seq, comma);
                changed = true;
            }
            continue;
        }
        let found = matched(seq, s, e, &spans);
        let Some((&key, targets)) = found.iter().next() else {
            continue;
        };
        let inner = if key >= 100 {
            format!("{indent}{unit}")
        } else {
            indent.clone()
        };
        let targets: Vec<usize> = targets.iter().copied().filter(|&t| t != e).collect();
        if key % 100 == 0 {
            break_at_indents(seq, s, &targets, max, &indent, &inner);
        } else {
            for t in targets {
                indent_to(seq, t, &inner);
            }
        }
        changed = true;
    }
    changed
}
