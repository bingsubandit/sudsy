//! Where a line breaks: a search over the break choices of an unwrapped line for the cheapest
//! layout, as clang-format's penalty search does it. A state is the column and the brackets
//! that are open (where a wrapped line inside each starts); a token goes on the line or on a
//! new one, a break costs the penalty of its place, and a column past the limit costs far more.

use super::annotate::{Ann, Role};
use super::model::Tk;
use super::style::{BracketAlign, Style};
use crate::generated::kind as k;
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Lv {
    /// Where a wrapped line of this bracket starts.
    indent: usize,
    /// Where the line that holds the bracket's current item started.
    last: usize,
    /// Where the current expression started (wrapped operands line up with it).
    expr: usize,
    /// The column of the last `?` of the level.
    question: usize,
    /// The token that opened the level, to finish it when the first token after it is placed.
    fresh: bool,
    /// A break after a comma or the opening bracket: the list goes one item per line.
    list_broken: bool,
    shift_broken: bool,
    /// A break inside the level, or a forced one: every comma of the list breaks.
    force: bool,
    /// Where the first string of the current run of strings starts, and the first `<<`.
    str_col: usize,
    shift_col: usize,
}

#[derive(Clone)]
struct St {
    col: usize,
    stack: Vec<Lv>,
    pen: u64,
    /// The last break in the arena.
    last: u32,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    col: usize,
    stack: Vec<Lv>,
}

pub fn width(text: &str) -> usize {
    match text.rfind('\n') {
        Some(i) => text[i + 1..].chars().count(),
        None => text.chars().count(),
    }
}

/// The break positions: token index and the column the token starts at.
pub struct Solver<'a> {
    pub toks: &'a [Tk<'a>],
    pub ann: &'a [Ann],
    pub style: &'a Style,
    pub decl: bool,
    /// Column limit; 0 for none.
    pub limit: usize,
}

impl Solver<'_> {
    fn first_line_width(&self, i: usize) -> usize {
        let t = self.toks[i].text;
        match t.find('\n') {
            Some(p) => t[..p].chars().count(),
            None => t.chars().count(),
        }
    }

    /// The width of the tokens on one line, from a start column.
    pub fn one_line_end(&self, start: usize) -> usize {
        let mut col = start;
        for i in 0..self.toks.len() {
            let sp = (i > 0 && self.ann[i].space) as usize;
            col += sp;
            if self.toks[i].text.contains('\n') {
                col = width(self.toks[i].text);
            } else {
                col += self.toks[i].text.chars().count();
            }
        }
        col
    }

    pub fn solve(&self, indent: usize, start: usize) -> Vec<(usize, usize)> {
        let n = self.toks.len();
        let cont = self.style.continuation;
        let must_any = (1..n).any(|i| self.ann[i].must);
        if !must_any && (self.limit == 0 || self.one_line_end(start) <= self.limit) {
            return Vec::new();
        }
        let beam = (6000 / n.max(1)).clamp(3, 40);
        let mut arena: Vec<(u32, u32, u32)> = Vec::new(); // parent, token, column
        let first = St {
            col: start + self.first_line_width(0),
            stack: vec![Lv {
                indent: indent + cont,
                last: indent,
                expr: start,
                question: 0,
                fresh: false,
                list_broken: false,
                shift_broken: false,
                force: false,
                str_col: 0,
                shift_col: 0,
            }],
            pen: 0,
            last: u32::MAX,
        };
        let mut states = vec![first];
        let mut first_state = true;
        let mut start_state_open = false;
        let _ = (&mut first_state, &mut start_state_open);
        // The first token may itself open a bracket.
        states[0] = self.after(states[0].clone(), 0, start, false);
        for i in 1..n {
            let mut next: BTreeMap<Key, St> = BTreeMap::new();
            for s in &states {
                for brk in [false, true] {
                    if let Some(t) = self.step(s, i, brk, &mut arena) {
                        let key = Key {
                            col: t.col,
                            stack: t.stack.clone(),
                        };
                        match next.get(&key) {
                            Some(old) if old.pen <= t.pen => {}
                            _ => {
                                next.insert(key, t);
                            }
                        }
                    }
                }
            }
            if next.is_empty() {
                // No legal choice (a forced break that cannot be made): keep going on the line.
                for s in &states {
                    let mut t = s.clone();
                    let w = self.first_line_width(i);
                    t.col += self.ann[i].space as usize + w;
                    let key = Key {
                        col: t.col,
                        stack: t.stack.clone(),
                    };
                    next.insert(key, t);
                }
            }
            let mut v: Vec<St> = next.into_values().collect();
            v.sort_by_key(|s| (s.pen, s.col));
            v.truncate(beam);
            states = v;
        }
        let Some(best) = states.iter().min_by_key(|s| s.pen) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut at = best.last;
        while at != u32::MAX {
            let (parent, tok, col) = arena[at as usize];
            out.push((tok as usize, col as usize));
            at = parent;
        }
        out.reverse();
        out
    }

    /// The state after token `i` is placed at column `c` (its start).
    fn after(&self, mut s: St, i: usize, c: usize, brk: bool) -> St {
        let a = &self.ann[i];
        let t = &self.toks[i];
        let cont = self.style.continuation;
        let prev_role = if i > 0 {
            Some(self.ann[i - 1].role)
        } else {
            None
        };
        let prev = if i > 0 { Some(&self.toks[i - 1]) } else { None };
        let n = s.stack.len();
        // The first token after an opening bracket decides the bracket's alignment.
        if s.stack[n - 1].fresh && i > 0 {
            let top = &mut s.stack[n - 1];
            top.fresh = false;
            if brk {
                top.last = c;
                top.expr = c;
                top.list_broken = true;
                if matches!(
                    self.style.align,
                    BracketAlign::AlwaysBreak | BracketAlign::DontAlign
                ) {
                    // Wrapped lines of a bracket opened without alignment keep the continuation.
                }
            } else if self.style.align == BracketAlign::Align {
                top.indent = c;
                top.last = c;
                top.expr = c;
            } else {
                top.last = c;
                top.expr = c;
            }
        } else if brk {
            let top = &mut s.stack[n - 1];
            top.last = c;
            if matches!(prev_role, Some(Role::Comma | Role::Assign))
                || prev.is_some_and(|p| p.word("return"))
            {
                top.expr = c;
            }
            if prev_role == Some(Role::Comma) {
                top.list_broken = true;
            }
            if a.role == Role::Binary && t.kind == k::LESS_LESS {
                top.shift_broken = true;
            }
        } else {
            let top = &mut s.stack[n - 1];
            if matches!(
                prev_role,
                Some(Role::Comma | Role::Assign | Role::Question | Role::TernColon)
            ) || prev.is_some_and(|p| p.word("return") || p.word("co_return") || p.word("throw"))
            {
                top.expr = c;
            }
        }
        {
            let top = s.stack.last_mut().expect("a level");
            if t.kind == k::STRING && prev.is_none_or(|p| p.kind != k::STRING) {
                top.str_col = c;
            }
            if t.kind == k::LESS_LESS && a.role == Role::Binary && top.shift_col == 0 {
                top.shift_col = c;
            }
        }
        match a.role {
            Role::Close | Role::TplClose | Role::Cast if t.kind != k::RBRACE || true => {
                let drop = if t.kind == k::GREATER_GREATER && a.role == Role::TplClose {
                    2
                } else {
                    1
                };
                for _ in 0..drop {
                    if s.stack.len() > 1 {
                        s.stack.pop();
                    }
                }
            }
            Role::Open | Role::TplOpen | Role::Lambda => {
                let parent = s.stack.last().expect("a level");
                let lv = Lv {
                    indent: parent.last + cont,
                    last: parent.last,
                    expr: c,
                    question: 0,
                    fresh: true,
                    list_broken: false,
                    shift_broken: false,
                    force: false,
                    str_col: 0,
                    shift_col: 0,
                };
                s.stack.push(lv);
            }
            Role::Question => {
                let top = s.stack.last_mut().expect("a level");
                top.question = c;
            }
            Role::CtorColon => {
                let top = s.stack.last_mut().expect("a level");
                top.indent = c + 2;
            }
            _ => {}
        }
        s
    }

    fn step(&self, s: &St, i: usize, brk: bool, arena: &mut Vec<(u32, u32, u32)>) -> Option<St> {
        let a = &self.ann[i];
        let t = &self.toks[i];
        let prev = &self.toks[i - 1];
        let pa = &self.ann[i - 1];
        let cont = self.style.continuation;
        let top = s.stack.last().expect("a level");
        let mut must = a.must;
        // One item per line once a list has broken (no bin packing); a `<<` chain likewise.
        let bin = if self.decl {
            self.style.bin_params
        } else {
            self.style.bin_args
        };
        if !bin && pa.role == Role::Comma && top.list_broken && a.can {
            must = true;
        }
        if pa.role == Role::Comma && top.force && a.can && !t.comment() {
            must = true;
        }
        if pa.role == Role::Open
            && self.style.align == BracketAlign::AlwaysBreak
            && a.level > 0
            && a.can
            && prev.kind == k::LPAR
        {
            must = true;
        }
        let w = self.first_line_width(i);
        if brk {
            if !a.can && !must {
                return None;
            }
            // The column of the new line.
            let col = if pa.role == Role::Open
                || pa.role == Role::TplOpen
                || pa.role == Role::Lambda
            {
                top.indent
            } else if a.role == Role::Close && t.kind == k::RBRACE {
                s.stack
                    .get(s.stack.len().wrapping_sub(2))
                    .map_or(top.indent, |p| p.last)
            } else {
                match a.role {
                    Role::Question => top.expr + cont,
                    Role::TernColon => {
                        if top.question > 0 {
                            top.question
                        } else {
                            top.expr + cont
                        }
                    }
                    Role::Member => top.expr + cont,
                    Role::Arrow => top.last + cont,
                    Role::CtorColon => top.last + self.style.init_indent,
                    _ if a.base => top.last,
                    Role::Binary if t.kind == k::LESS_LESS && top.shift_col > 0 => top.shift_col,
                    Role::Binary if t.kind == k::LESS_LESS && s.stack.len() == 1 => top.expr + cont,
                    Role::Binary => top.expr,
                    _ if t.kind == k::STRING && prev.kind == k::STRING => top.str_col,
                    Role::Comment if t.nl > 0 && pa.role != Role::Comma => top.indent,
                    _ => match pa.role {
                        Role::Binary => top.expr,
                        Role::Assign => top.last + cont,
                        Role::Comma => top.indent,
                        Role::Question | Role::TernColon => top.expr,
                        _ => top.indent,
                    },
                }
            };
            let mut st = s.clone();
            st.pen += a.pen as u64;
            let end = col + w;
            if self.limit > 0 && end > self.limit && !t.comment() {
                st.pen += 100_000 * (end - self.limit) as u64;
            }
            // A break makes the lists around it one item per line.
            let depth = st.stack.len();
            for lv in &mut st.stack[1..depth.max(2) - 1] {
                lv.force = true;
            }
            if a.must && t.kind == k::STRING {
                st.stack[depth - 1].force = true;
            }
            arena.push((s.last, i as u32, col as u32));
            st.last = (arena.len() - 1) as u32;
            st.col = if t.text.contains('\n') {
                width(t.text)
            } else {
                end
            };
            Some(self.after(st, i, col, true))
        } else {
            if must {
                return None;
            }
            let c = s.col + a.space as usize;
            let end = c + w;
            let mut st = s.clone();
            if self.limit > 0 && end > self.limit && !t.comment() {
                st.pen += 100_000 * (end - self.limit) as u64;
            }
            st.col = if t.text.contains('\n') {
                width(t.text)
            } else {
                end
            };
            Some(self.after(st, i, c, false))
        }
    }
}
