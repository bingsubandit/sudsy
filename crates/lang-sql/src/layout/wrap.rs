//! Long lines (sqlfluff's LT05): a line over the width is broken after its outermost indent and
//! before the matching dedent; a trailing line comment moves to the line before.

use super::seq::{NONE, Seq};
use crate::generated::kind as k;
use std::borrow::Cow;

/// The first block of every line, and the end of the last.
fn line_starts(seq: &Seq) -> Vec<usize> {
    let mut out = vec![0];
    out.extend((1..seq.blocks.len()).filter(|&i| seq.points[i].newlines > 0));
    out.push(seq.blocks.len());
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

/// The indent and dedent points of the outermost pairs on the line (the lowest balance): an
/// indent point that is net positive and has code behind it, a dedent point that is net negative.
fn candidates(seq: &Seq, s: usize, e: usize) -> Vec<(usize, Option<usize>)> {
    let mut stack: Vec<(usize, i32)> = Vec::new();
    let mut pairs: Vec<(i32, usize, Option<usize>)> = Vec::new();
    let mut bal = 0;
    for j in s + 1..e {
        for &m in &seq.points[j].metas {
            if m > 0 {
                stack.push((j, bal));
                bal += 1;
            } else {
                bal -= 1;
                if let Some((at, level)) = stack.pop() {
                    pairs.push((level, at, Some(j)));
                }
            }
        }
    }
    pairs.extend(stack.into_iter().map(|(at, level)| (level, at, None)));
    let low = pairs.iter().map(|p| p.0).min();
    pairs
        .into_iter()
        .filter(|p| Some(p.0) == low)
        .map(|p| (p.1, p.2))
        .collect()
}

/// The blocks after the commas that sit at the lowest bracket depth of the line.
fn comma_breaks(seq: &Seq, s: usize, e: usize) -> Vec<usize> {
    let mut depth = 0i32;
    let mut commas: Vec<(i32, usize)> = Vec::new();
    for i in s..e {
        match seq.blocks[i].kind {
            k::L_PAREN | k::L_SQUARE => depth += 1,
            k::R_PAREN | k::R_SQUARE => depth -= 1,
            k::COMMA if i + 1 < e && seq.points[i + 1].newlines == 0 && !seq.is_comment(i + 1) => {
                commas.push((depth, i + 1));
            }
            _ => {}
        }
    }
    let low = commas.iter().map(|c| c.0).min();
    commas
        .into_iter()
        .filter(|c| Some(c.0) == low)
        .map(|c| c.1)
        .collect()
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
}

/// Do all pieces of the line fit, indented by the balance at their start?
fn fits(seq: &Seq, s: usize, e: usize, max: usize, tab: usize) -> bool {
    let base: usize = seq.points[s].ws.chars().count();
    let mut bal = 0i32;
    let mut len = 0usize;
    let mut indent = base;
    for i in s..e {
        let p = &seq.points[i];
        if i > s {
            bal += p.stats().0;
        }
        if i > s && p.newlines > 0 {
            if indent + len > max {
                return false;
            }
            indent = base + tab * bal.max(0) as usize;
            len = 0;
        } else if i > s {
            len += p.ws.chars().count();
        }
        len += seq.blocks[i].text.chars().count();
    }
    indent + len <= max
}

/// A line break before block `at`, unless there is one; returns whether it added one.
fn split(seq: &mut Seq, at: usize) -> bool {
    if seq.points[at].newlines > 0 {
        return false;
    }
    seq.points[at].newlines = 1;
    seq.points[at].ws = Cow::Borrowed("");
    true
}

/// Fixes every line that is too long and can be (from the end, so indexes stay valid); returns
/// whether it fixed any.
pub fn long_lines(seq: &mut Seq, max: usize, tab: usize) -> bool {
    if max == 0 {
        return false;
    }
    let starts = line_starts(seq);
    let mut changed = false;
    for w in starts.windows(2).rev() {
        let (s, e) = (w[0], w[1]);
        if e - s < 2 || width(seq, s, e, tab) <= max {
            continue;
        }
        let last = e - 1;
        if seq.blocks[last].kind == k::LINE_COMMENT
            && seq.points[last].newlines == 0
            && !seq.is_comment(s)
        {
            lift_comment(seq, s, last);
            changed = true;
            continue;
        }
        let mut pairs = candidates(seq, s, e);
        let mut broke = false;
        pairs.reverse();
        let mut added: Vec<usize> = Vec::new();
        for (at, dedent) in pairs {
            let up =
                !seq.points[at].implicit && !seq.is_comment(at) && seq.points[at].stats().0 > 0;
            if up && split(seq, at) {
                added.push(at);
            }
            if let Some(d) = dedent
                && seq.points[d].stats().0 < 0
                && !matches!(seq.blocks[d].kind, k::COMMA | k::SEMICOLON)
                && split(seq, d)
            {
                added.push(d);
            }
            if !added.is_empty() && fits(seq, s, e, max, tab) {
                break;
            }
        }
        broke |= !added.is_empty();
        if broke {
            changed = true;
        } else {
            for at in comma_breaks(seq, s, e) {
                seq.points[at].newlines = 1;
                seq.points[at].ws = Cow::Borrowed("");
                changed = true;
            }
        }
    }
    changed
}
