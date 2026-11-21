//! Indentation (sqlfluff's LT02): the parser's `Indent` and `Dedent` give each line a balance;
//! an indent that no line break follows is untaken and does not count for the lines after it,
//! and an indent that is taken but whose dedent has no line break forces one.
//!
//! concept: new indent-balance-layout
//! what: Indentation from the indent and dedent balance of each line.
//! tools: sqlfluff LT02

use super::seq::{Point, Seq};
use std::borrow::Cow;

#[derive(Clone)]
struct IPoint {
    idx: usize,
    impulse: i32,
    trough: i32,
    init: i32,
    last_break: Option<usize>,
    is_break: bool,
    untaken: Vec<i32>,
}

impl IPoint {
    fn closing(&self) -> i32 {
        self.init + self.impulse
    }
}

struct Line {
    init: i32,
    points: Vec<IPoint>,
}

impl Line {
    fn from_points(points: Vec<IPoint>) -> Line {
        let last = points.last().and_then(|p| p.last_break).unwrap_or(0);
        let init = if last != 0 { points[0].closing() } else { 0 };
        Line { init, points }
    }

    fn opening(&self) -> i32 {
        match self.points.last().and_then(|p| p.last_break) {
            None => 0,
            Some(_) => self.points[0].closing(),
        }
    }

    fn closing(&self) -> i32 {
        self.points.last().map_or(0, IPoint::closing)
    }

    fn desired_units(&self, forced: &[i32]) -> i32 {
        let first = &self.points[0];
        let relevant = if first.trough != 0 {
            let limit = self.init - (first.impulse - first.trough);
            first.untaken.iter().filter(|&&i| i <= limit).count()
        } else {
            first.untaken.len()
        };
        self.init - relevant as i32 + forced.len() as i32
    }

    /// The blocks of the line (those between its first and last point).
    fn blocks(&self) -> std::ops::Range<usize> {
        let end = self.points.last().map_or(0, |p| p.idx);
        match self.points.last().and_then(|p| p.last_break) {
            None => 0..end,
            Some(_) => self.points[0].idx..end,
        }
    }
}

/// Untaken indents that survive a point, and those it adds.
fn prune(untaken: &[i32], balance: i32, stats: (i32, i32), has_newline: bool) -> Vec<i32> {
    let (impulse, trough) = stats;
    let limit = if trough < impulse {
        balance + impulse + trough
    } else {
        balance + impulse
    };
    let mut out: Vec<i32> = untaken.iter().copied().filter(|&x| x <= limit).collect();
    if impulse > trough && !has_newline {
        out.extend((trough..impulse).map(|i| balance + i + 1));
    }
    out
}

fn crawl(seq: &Seq) -> Vec<IPoint> {
    let mut out = Vec::new();
    let mut last_break: Option<usize> = None;
    let mut balance = 0;
    let mut untaken: Vec<i32> = Vec::new();
    let mut cached: Option<((i32, i32), IPoint)> = None;
    for (i, point) in seq.points.iter().enumerate() {
        let mut stats = point.stats();
        if let Some((first, cp)) = cached.take() {
            stats = (first.0 + stats.0, first.1.min(first.0 + stats.1));
            if cp.is_break {
                out.push(IPoint {
                    impulse: stats.0,
                    trough: stats.1,
                    init: balance,
                    untaken: untaken.clone(),
                    ..cp
                });
                untaken = prune(&untaken, balance, stats, true);
                balance += stats.0;
                stats = (0, 0);
            } else {
                out.push(IPoint {
                    impulse: 0,
                    trough: 0,
                    init: balance,
                    untaken: untaken.clone(),
                    ..cp
                });
            }
        }
        let has_newline = point.newlines > 0 && Some(i) != last_break;
        let ip = IPoint {
            idx: i,
            impulse: stats.0,
            trough: stats.1,
            init: balance,
            last_break,
            is_break: has_newline,
            untaken: untaken.clone(),
        };
        if has_newline {
            last_break = Some(i);
        }
        if seq.is_comment(i) {
            cached = Some((stats, ip));
            continue;
        }
        if has_newline || stats.0 != 0 || stats.1 != 0 || i == 0 || seq.is_eof(i) {
            out.push(ip);
        }
        untaken = prune(&untaken, balance, stats, has_newline);
        balance += stats.0;
    }
    out
}

/// The lines and the places where an indent was untaken on the way up but its dedent is taken.
fn map_lines(seq: &Seq, ipoints: &[IPoint]) -> (Vec<Line>, Vec<usize>) {
    let mut lines = Vec::new();
    let mut buffer: Vec<IPoint> = Vec::new();
    let mut imbalanced = Vec::new();
    let mut untaken_locs: Vec<(i32, usize)> = Vec::new();
    for ip in ipoints {
        buffer.push(ip.clone());
        if !ip.is_break {
            if ip.impulse > ip.trough {
                untaken_locs.retain(|&(b, _)| b != ip.init + ip.impulse);
                untaken_locs.push((ip.init + ip.impulse, ip.idx));
            }
            continue;
        }
        lines.push(Line::from_points(std::mem::take(&mut buffer)));
        if ip.trough != 0 && !seq.is_eof(ip.idx) {
            for i in ip.init + ip.trough + 1..=ip.init {
                let Some(&(_, loc)) = untaken_locs.iter().find(|&&(b, _)| b == i) else {
                    break;
                };
                if seq.blocks[loc].kind == crate::generated::kind::L_PAREN {
                    continue;
                }
                let line = lines.last().unwrap();
                if line.points.iter().any(|p| p.idx == loc) {
                    continue;
                }
                // Only comments between the end of the reference line and this point: a
                // misplaced indent.
                let line_end = ipoints
                    .iter()
                    .find(|p| p.idx >= loc && p.idx < ip.idx && p.is_break)
                    .map_or(ip.idx, |p| p.idx);
                if (line_end..ip.idx).all(|j| seq.is_comment(j)) {
                    continue;
                }
                imbalanced.push(loc);
            }
        }
        untaken_locs.retain(|&(b, _)| b <= ip.init + ip.trough);
        buffer.push(ip.clone());
    }
    if buffer.len() > 1 {
        lines.push(Line::from_points(buffer));
    }
    (lines, imbalanced)
}

/// Anchor lines of comments to the line that follows them.
fn revise_comment_lines(seq: &Seq, lines: &mut [Line]) {
    let mut pending: Vec<usize> = Vec::new();
    for idx in 0..lines.len() {
        let blocks = lines[idx].blocks();
        let all_comments = !blocks.is_empty() && blocks.clone().all(|b| seq.is_comment(b));
        if all_comments {
            pending.push(idx);
        } else {
            let init = lines[idx].init;
            for p in pending.drain(..) {
                lines[p].init = init;
            }
        }
    }
    for p in pending {
        lines[p].init = 0;
    }
}

fn indent_to(point: &mut Point, indent: &str) {
    if point.newlines == 0 {
        point.newlines = 1;
    }
    point.ws = Cow::Owned(indent.to_string());
}

fn current_indent(seq: &Seq, last_break: Option<usize>) -> String {
    seq.points[last_break.unwrap_or(0)].ws.to_string()
}

fn lint_starting(seq: &mut Seq, line: &Line, unit: &str, forced: &[i32]) {
    let first = &line.points[0];
    let last_break = line.points.last().and_then(|p| p.last_break);
    let current = current_indent(seq, last_break);
    let desired = unit.repeat(line.desired_units(forced).max(0) as usize);
    if current == desired {
        return;
    }
    let at = first.idx;
    if at > 0 && at < seq.points.len() - 1 {
        if seq.is_comment(at) {
            let previous = current_indent(seq, first.last_break);
            if current.len() == previous.len() {
                return;
            }
        }
        if seq.blocks[at - 1].kind == crate::generated::kind::BLOCK_COMMENT
            && seq.blocks[at].kind == crate::generated::kind::BLOCK_COMMENT
            && current.len() > desired.len()
        {
            return;
        }
    }
    if at == 0 && !first.is_break {
        seq.points[0].ws = Cow::Borrowed("");
    } else {
        indent_to(&mut seq.points[at], &desired);
    }
}

/// Returns the balances forced by a line break added for an untaken indent.
fn lint_untaken_positive(seq: &mut Seq, line: &Line, unit: &str, imbalanced: &[usize]) -> Vec<i32> {
    let mut forced: Vec<i32> = Vec::new();
    for ip in &line.points {
        if imbalanced.contains(&ip.idx) {
            let untaken = ip.untaken.iter().filter(|u| !forced.contains(u)).count();
            let desired = unit.repeat((ip.closing() - untaken as i32).max(0) as usize);
            indent_to(&mut seq.points[ip.idx], &desired);
            forced.push(ip.closing());
        }
    }
    if !forced.is_empty() {
        return forced;
    }
    let last = line.points.last().unwrap();
    if last.init + last.trough <= line.opening() {
        return forced;
    }
    let mut closing = last.init
        + if last.trough != 0 {
            last.trough
        } else {
            last.impulse
        };
    let mut balance = 0;
    for j in last.idx..seq.blocks.len() {
        if !seq.is_comment(j) || j + 1 >= seq.points.len() {
            break;
        }
        let stats = seq.points[j + 1].stats();
        if stats.0 > 0 {
            break;
        }
        closing = balance + stats.1;
        balance += stats.0;
    }
    if !last.untaken.contains(&closing) {
        return forced;
    }
    if let Some(ip) = line.points.iter().find(|p| p.closing() == closing) {
        let desired = unit.repeat((ip.closing() - ip.untaken.len() as i32).max(0) as usize);
        indent_to(&mut seq.points[ip.idx], &desired);
        forced.push(closing);
    }
    forced
}

fn lint_untaken_negative(seq: &mut Seq, line: &Line, unit: &str, forced: &[i32]) {
    if line.closing() >= line.opening() {
        return;
    }
    for ip in &line.points[..line.points.len() - 1] {
        if ip.is_break || ip.impulse >= 0 || ip.init + ip.trough >= line.opening() {
            continue;
        }
        let covered = ip.init + ip.trough + 1..=ip.init;
        let untaken: Vec<i32> = ip
            .untaken
            .iter()
            .copied()
            .filter(|u| !forced.contains(u))
            .collect();
        if covered.clone().all(|c| untaken.contains(&c)) {
            continue;
        }
        let next = &seq.blocks[ip.idx];
        if next.kind == crate::generated::kind::SEMICOLON
            || next.kind == crate::generated::kind::COMMA
        {
            continue;
        }
        let desired = ip.closing() - ip.untaken.len() as i32 + forced.len() as i32;
        let desired = unit.repeat(desired.max(0) as usize);
        indent_to(&mut seq.points[ip.idx], &desired);
    }
}

/// Fixes the line breaks and the indentation of the file; returns whether anything changed.
pub fn reindent(seq: &mut Seq, unit: &str) -> bool {
    let ipoints = crawl(seq);
    let (mut lines, imbalanced) = map_lines(seq, &ipoints);
    revise_comment_lines(seq, &mut lines);
    let before: Vec<(u32, Cow<str>)> = seq
        .points
        .iter()
        .map(|p| (p.newlines, p.ws.clone()))
        .collect();
    let mut forced: Vec<i32> = Vec::new();
    for line in &lines {
        lint_starting(seq, line, unit, &forced);
        let new = lint_untaken_positive(seq, line, unit, &imbalanced);
        if new.is_empty() {
            lint_untaken_negative(seq, line, unit, &forced);
            let closing = line.closing();
            forced.retain(|&i| i <= closing);
        } else {
            forced.extend(new);
        }
    }
    seq.points
        .iter()
        .zip(&before)
        .any(|(p, (n, ws))| p.newlines != *n || p.ws != *ws)
}
