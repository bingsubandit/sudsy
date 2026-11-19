//! Where a long line breaks: the tokens of one unwrapped line become a `Doc` for the shared
//! printer. Brackets are groups whose wrapped lines align after the opener, and the gaps where a
//! line may break are split by penalty, the lowest first (commas, assignment, `?:`, binary
//! operators by precedence): each tier is a fill or a group, the segments between its gaps are
//! laid out by the next tier. What follows a closing bracket up to the next gap goes with the
//! last item of the bracket, so that the fill measures it.
//!
//! concept: none (penalty-tier splitting of one unwrapped line into a Doc; generic, to move into core-format when a second language needs it)

use super::annotate::{Ann, NONE, Role};
use super::model::Tk;
use super::style::{BracketAlign, Style};
use crate::generated::kind as k;
use core_format::doc::{self, Doc};
use core_format::printer::{self, PrintOptions};

/// Marks the place of a trailing comment in the printed text; the comment is the row's own.
const COMMENT_MARK: char = '\u{1}';
/// Marks a line break inside one token (a block comment, a raw string).
const NEWLINE_MARK: char = '\u{2}';
/// A gap that costs this much or more is no place to break while a cheaper one is left: what
/// follows a closing bracket up to the next cheaper gap stays with it.
const EXPENSIVE: u32 = 100;

pub struct Wrapper<'a> {
    pub toks: &'a [Tk<'a>],
    pub ann: &'a [Ann],
    pub style: &'a Style,
    pub decl: bool,
    /// The column the first token starts at and the column limit (0: none).
    pub col: usize,
    pub limit: usize,
    close: Vec<usize>,
    shared: Vec<bool>,
}

/// One printed row: its text, its trailing comment and whether it holds a multi-line token.
pub struct PrintedRow {
    pub text: String,
    pub comment: Option<String>,
    pub fixed: bool,
}

pub fn width(text: &str) -> usize {
    match text.rfind('\n') {
        Some(i) => text[i + 1..].chars().count(),
        None => text.chars().count(),
    }
}

fn first_line(text: &str) -> usize {
    text.find('\n')
        .map_or_else(|| text.chars().count(), |p| text[..p].chars().count())
}

fn space(on: bool) -> Doc {
    if on { doc::text(" ") } else { doc::nil() }
}

impl<'a> Wrapper<'a> {
    pub fn new(
        toks: &'a [Tk<'a>],
        ann: &'a [Ann],
        style: &'a Style,
        decl: bool,
        col: usize,
        limit: usize,
    ) -> Self {
        let n = toks.len();
        let mut close = vec![NONE; n];
        let mut shared = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        for i in 0..n {
            match ann[i].role {
                Role::Open | Role::TplOpen | Role::Lambda => stack.push(i),
                Role::Close | Role::Cast | Role::TplClose => {
                    let double =
                        toks[i].kind == k::GREATER_GREATER && ann[i].role == Role::TplClose;
                    for d in 0..if double { 2 } else { 1 } {
                        if let Some(o) = stack.pop() {
                            close[o] = i;
                            shared[o] = double && d == 0;
                        }
                    }
                }
                _ => {}
            }
        }
        Wrapper {
            toks,
            ann,
            style,
            decl,
            col,
            limit,
            close,
            shared,
        }
    }

    /// The width of tokens `s..e` on one line.
    fn flat(&self, s: usize, e: usize) -> usize {
        let mut w = 0;
        for i in s..e {
            if i > s && self.ann[i].space {
                w += 1;
            }
            w += first_line(self.toks[i].text);
        }
        w
    }

    /// The column the line ends at when it stays on one row.
    pub fn one_line_end(&self, start: usize) -> usize {
        let mut col = start;
        for i in 0..self.toks.len() {
            col += usize::from(i > 0 && self.ann[i].space);
            if self.toks[i].text.contains('\n') {
                col = width(self.toks[i].text);
            } else {
                col += self.toks[i].text.chars().count();
            }
        }
        col
    }

    /// The rows of the line, laid out and printed at `col`.
    pub fn rows(&self) -> Vec<PrintedRow> {
        let mut d = self.seq(0, self.toks.len(), false, &doc::nil());
        let line_width = if self.limit == 0 {
            usize::MAX / 4
        } else {
            self.limit.saturating_sub(self.col).max(1)
        };
        let opts = PrintOptions {
            line_width,
            indent_width: self.style.continuation,
            use_tabs: false,
        };
        let text = printer::print(&mut d, &opts);
        text.trim_end_matches('\n')
            .split('\n')
            .map(|l| {
                let (text, comment) = match l.split_once(COMMENT_MARK) {
                    Some((a, b)) => (a, Some(b.replace(NEWLINE_MARK, "\n"))),
                    None => (l, None),
                };
                let fixed = text.contains(NEWLINE_MARK)
                    || comment.as_ref().is_some_and(|c| c.contains('\n'));
                PrintedRow {
                    text: text.trim_end().replace(NEWLINE_MARK, "\n"),
                    comment,
                    fixed,
                }
            })
            .collect()
    }

    fn token(&self, i: usize) -> Doc {
        let t = &self.toks[i];
        if i > 0
            && t.comment()
            && t.nl == 0
            && (t.text.starts_with("//") || i + 1 == self.toks.len())
        {
            // A comment that ends its row is the row's own, aligned with the others.
            let text = t.text.trim_end().replace('\n', &NEWLINE_MARK.to_string());
            return doc::line_suffix(doc::text(format!("{COMMENT_MARK}{text}")));
        }
        if t.text.contains('\n') {
            doc::text(t.text.replace('\n', &NEWLINE_MARK.to_string()))
        } else {
            doc::text(t.text)
        }
    }

    fn is_open(&self, i: usize) -> bool {
        matches!(self.ann[i].role, Role::Open | Role::TplOpen | Role::Lambda)
    }

    fn is_close(&self, i: usize) -> bool {
        matches!(self.ann[i].role, Role::Close | Role::Cast | Role::TplClose)
    }

    /// What a break before token `i` costs, without the part for the brackets it is in.
    fn pen(&self, i: usize) -> u32 {
        self.ann[i].pen.saturating_sub(20 * self.ann[i].level)
    }

    fn is_gap(&self, i: usize) -> bool {
        self.ann[i].can || self.ann[i].must
    }

    /// The end of what follows the closer at `c` and goes with it: tokens up to the next gap or
    /// bracket.
    fn tail_end(&self, c: usize, e: usize) -> usize {
        let mut j = c + 1;
        while j < e
            && !self.ann[j].must
            && !(self.ann[j].can && (self.pen(j) < EXPENSIVE || self.ann[j].role == Role::Arrow))
            && !(self.is_open(j) && self.close[j] != NONE)
        {
            j += 1;
        }
        j
    }

    /// The index after the atom at `i` with its tail (`i + 1` for a token).
    fn after(&self, i: usize, e: usize) -> usize {
        if self.is_open(i) {
            let c = self.close[i];
            if c == NONE || c >= e {
                e
            } else {
                self.tail_end(c, e)
            }
        } else {
            i + 1
        }
    }

    /// The gaps (the index of the token after) where the line may break, outside brackets.
    fn cuts(&self, s: usize, e: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut i = s;
        while i < e {
            if i > s && self.is_gap(i) && !self.keeps_type(s, i) {
                out.push(i);
            }
            i = self.after(i, e);
        }
        out
    }

    /// Whether the gap at `i` is in the type of a declaration that starts the line, and the line
    /// reaches its first bracket on one row: then the parameters wrap, the type stays whole.
    fn keeps_type(&self, s: usize, i: usize) -> bool {
        if (s != 0 && !self.ann[s].must) || !self.decl || self.ann[i].must || self.limit == 0 {
            return false;
        }
        let Some(o) = (s..self.toks.len()).find(|&j| self.is_open(j) && self.close[j] != NONE)
        else {
            return false;
        };
        i < o && self.col + self.flat(s, o + 1) <= self.limit
    }

    fn gap(&self, i: usize) -> Doc {
        if self.ann[i].must {
            doc::hardline()
        } else if self.ann[i].space {
            doc::line()
        } else {
            doc::softline()
        }
    }

    /// Tokens `s..e` with no gap decided: spaces and bracket groups only; `tr` ends the last.
    fn plain(&self, s: usize, e: usize, tr: &Doc) -> Doc {
        let mut parts = Vec::new();
        let mut i = s;
        let mut used = false;
        while i < e {
            if i > s && self.ann[i].space {
                parts.push(doc::text(" "));
            }
            if self.is_open(i) {
                let next = self.after(i, e);
                used = next >= e;
                parts.push(self.atom(i, e, if used { tr } else { &Doc::Nil }));
                i = next.max(i + 1);
            } else {
                used = false;
                parts.push(self.token(i));
                i += 1;
            }
        }
        if !used {
            parts.push(tr.clone());
        }
        doc::concat(parts)
    }

    /// A bracket with what it holds and its tail; `tr` comes after the tail.
    fn atom(&self, o: usize, e: usize, tr: &Doc) -> Doc {
        let c = self.close[o];
        let open = self.token(o);
        if c == NONE || c >= e {
            let inner = self.seq(o + 1, e, true, tr);
            return doc::concat(vec![open, doc::align_here(inner)]);
        }
        let end = self.tail_end(c, e);
        let mut trailer = vec![];
        if !self.shared[o] {
            trailer.push(space(self.ann[c].space));
            trailer.push(self.token(c));
        }
        for j in c + 1..end {
            trailer.push(space(self.ann[j].space));
            trailer.push(self.token(j));
        }
        trailer.push(tr.clone());
        let trailer = doc::concat(trailer);
        if c == o + 1 {
            return doc::concat(vec![open, trailer]);
        }
        let inner = doc::shared(self.seq(o + 1, c, true, &trailer));
        let breakable = self.is_gap(o + 1);
        let always = self.style.align == BracketAlign::AlwaysBreak
            && self.toks[o].kind == k::LPAR
            && breakable;
        let gap = space(self.ann[o + 1].space);
        if always {
            return doc::concat(vec![
                open,
                doc::indent(doc::concat(vec![doc::hardline(), inner])),
            ]);
        }
        if !breakable {
            return doc::concat(vec![open, gap, doc::align_here(inner)]);
        }
        // The whole on one line; else the first line as far as it goes with the rest lined up
        // (or indented); else a break after the bracket.
        let flat = doc::concat(vec![open.clone(), gap.clone(), inner.clone()]);
        let hang = doc::concat(vec![
            open.clone(),
            doc::indent(doc::concat(vec![
                doc::hardline(),
                doc::group_broken(inner.clone()),
            ])),
        ]);
        // A forced break inside (a line comment) rules out the one-row candidate.
        let mut states = if inner.will_break() {
            Vec::new()
        } else {
            vec![flat]
        };
        if inner.will_break() && self.style.align != BracketAlign::Align {
            states.push(doc::concat(vec![
                open.clone(),
                gap.clone(),
                doc::indent(doc::group_broken(inner.clone())),
            ]));
        }
        if self.style.align == BracketAlign::Align {
            states.push(doc::concat(vec![
                open,
                gap,
                doc::align_here(doc::group_broken(inner)),
            ]));
        }
        states.push(hang);
        let group = doc::conditional_group(states);
        // An enclosing bracket counts this one whole when it fits a row of its own; one that
        // is longer has to be broken in place.
        let whole = self.flat(o, end) + self.style.continuation + tr.flat_width().unwrap_or(0);
        if self.limit == 0 || self.col + whole <= self.limit {
            doc::unit(group)
        } else {
            doc::head(group)
        }
    }

    /// Tokens `s..e` of one nesting level, followed by `tr`; `nested`: inside a bracket, whose
    /// alignment the wrapped lines follow.
    fn seq(&self, s: usize, e: usize, nested: bool, tr: &Doc) -> Doc {
        if s >= e {
            return tr.clone();
        }
        if e > s + 1
            && self.toks[s].kind == k::KEYWORD
            && matches!(
                self.toks[s].text,
                "return" | "co_return" | "co_yield" | "throw"
            )
        {
            // The expression after it: wrapped lines line up with its start.
            return doc::concat(vec![
                self.token(s),
                space(self.ann[s + 1].space),
                doc::align_here(self.seq(s + 1, e, true, tr)),
            ]);
        }
        if e > s + 1 && matches!(self.ann[s].role, Role::CtorColon | Role::InheritColon) {
            // The initializers or bases line up after the colon.
            return doc::concat(vec![
                self.token(s),
                space(self.ann[s + 1].space),
                doc::align_here(self.seq(s + 1, e, true, tr)),
            ]);
        }
        // `class A : public B, public C`: the bases line up after the colon.
        let mut i = s;
        while i < e {
            if i > s && i + 1 < e && self.ann[i].role == Role::InheritColon && !self.ann[i].must {
                return doc::concat(vec![
                    self.seq(s, i + 1, nested, &Doc::Nil),
                    space(self.ann[i + 1].space),
                    doc::align_here(self.seq(i + 1, e, true, tr)),
                ]);
            }
            i = self.after(i, e);
        }
        let cuts = self.cuts(s, e);
        if cuts.is_empty() {
            return self.plain(s, e, tr);
        }
        // Forced breaks first.
        let musts: Vec<usize> = cuts.iter().copied().filter(|&i| self.ann[i].must).collect();
        if !musts.is_empty() {
            return self.joined(s, e, &musts, nested, tr);
        }
        let min = cuts.iter().map(|&i| self.ann[i].pen).min().unwrap_or(0);
        let tier: Vec<usize> = cuts
            .iter()
            .copied()
            .filter(|&i| self.ann[i].pen == min)
            .collect();
        let first = tier[0];
        let (pa, ca) = (self.ann[first - 1].role, self.ann[first].role);
        if pa == Role::Comma {
            return self.list(s, e, &tier, nested, tr);
        }
        if pa == Role::Assign {
            return self.assignment(s, e, tier[0], nested, tr);
        }
        if ca == Role::Binary && self.toks[first].kind == k::LESS_LESS {
            return self.shifts(s, e, &tier, nested, tr);
        }
        if matches!(ca, Role::Question | Role::TernColon) {
            return self.ternary(s, e, &tier, nested, tr);
        }
        if pa == Role::Binary || ca == Role::Binary {
            return self.binary(s, e, &tier, nested, tr);
        }
        self.generic(s, e, &tier, nested, tr)
    }

    /// The parts of `s..e` between `cuts`, each laid out by the next tier; the last one ends
    /// with `tr`.
    fn parts(&self, s: usize, e: usize, cuts: &[usize], nested: bool, tr: &Doc) -> Vec<Doc> {
        let mut out = Vec::new();
        let mut from = s;
        for &c in cuts.iter().chain(std::iter::once(&e)) {
            out.push(self.seq(from, c, nested, if c == e { tr } else { &Doc::Nil }));
            from = c;
        }
        out
    }

    /// Docs with the gap docs between them.
    fn interleave(&self, docs: Vec<Doc>, cuts: &[usize]) -> Vec<Doc> {
        let mut v = Vec::new();
        for (i, d) in docs.into_iter().enumerate() {
            if i > 0 {
                v.push(self.gap(cuts[i - 1]));
            }
            v.push(d);
        }
        v
    }

    /// Docs whose later lines continue the first: indented at the statement, aligned to where
    /// the expression starts inside brackets or after an operator.
    fn continued(&self, s: usize, nested: bool, d: Doc) -> Doc {
        if s == 0 && !nested {
            doc::indent(d)
        } else {
            doc::align_here(d)
        }
    }

    /// Parts on rows of their own. A cut at the base (a declaration after `template <...>`)
    /// keeps the indentation of the row before; any other one continues the statement.
    fn joined(&self, s: usize, e: usize, cuts: &[usize], nested: bool, tr: &Doc) -> Doc {
        // The constructor's initializer list has its own indent.
        let flush = |parts: Vec<Doc>, indented: Option<usize>| {
            let d = doc::concat(parts);
            match indented {
                Some(w) if w != self.style.continuation => doc::align(w, d),
                Some(_) => doc::indent(d),
                None => d,
            }
        };
        let mut blocks = Vec::new();
        let mut cur: Vec<Doc> = Vec::new();
        let mut indented = None;
        let mut from = s;
        for &c in cuts.iter().chain(std::iter::once(&e)) {
            let part = self.seq(from, c, nested, if c == e { tr } else { &Doc::Nil });
            cur.push(part);
            if c != e {
                if self.ann[c].base {
                    cur.push(doc::hardline());
                } else {
                    blocks.push(flush(std::mem::take(&mut cur), indented));
                    cur.push(doc::hardline());
                    indented = (!nested).then(|| match self.ann[c].role {
                        Role::CtorColon => self.style.init_indent,
                        _ => self.style.continuation,
                    });
                }
            }
            from = c;
        }
        blocks.push(flush(cur, indented));
        doc::concat(blocks)
    }

    fn list(&self, s: usize, e: usize, cuts: &[usize], nested: bool, tr: &Doc) -> Doc {
        let docs = self.interleave(self.parts(s, e, cuts, nested, tr), cuts);
        let bin = if self.decl {
            self.style.bin_params
        } else {
            self.style.bin_args
        };
        let d = if bin {
            doc::fill(docs)
        } else {
            doc::group(doc::concat(docs))
        };
        if nested { d } else { doc::indent(d) }
    }

    /// `lhs = rhs`: on one line, or the right side on a line of its own when it fits there, or
    /// laid out where it stands.
    fn assignment(&self, s: usize, e: usize, c: usize, nested: bool, tr: &Doc) -> Doc {
        let lhs = self.seq(s, c, nested, &Doc::Nil);
        let rhs = self.seq(c, e, true, tr);
        let cont = self.style.continuation;
        // The column the expression starts at, when what stands before it is on one row.
        let start = if (nested || s > 0) && !self.ann[s].must {
            self.col + self.flat(0, s) + usize::from(s > 0 && self.ann[s].space)
        } else {
            self.col
        };
        let rhs_flat = self.flat(c, e) + self.tail_width(e);
        let whole = start + self.flat(s, c) + usize::from(self.ann[c].space) + rhs_flat;
        if self.limit != 0
            && !nested
            && whole > self.limit
            && start + cont + rhs_flat <= self.limit
            && self.ann[c].can
        {
            return doc::concat(vec![
                lhs,
                doc::indent(doc::concat(vec![doc::hardline(), rhs])),
            ]);
        }
        doc::concat(vec![lhs, space(self.ann[c].space), doc::align_here(rhs)])
    }

    /// The width of the closer at `e` and what goes with it (0 when `e` is no closer).
    fn tail_width(&self, e: usize) -> usize {
        if e >= self.toks.len() || !self.is_close(e) {
            return 0;
        }
        self.flat(e, self.tail_end(e, self.toks.len()))
    }

    fn ternary(&self, s: usize, e: usize, cuts: &[usize], nested: bool, tr: &Doc) -> Doc {
        let docs = self.interleave(self.parts(s, e, cuts, nested, tr), cuts);
        self.continued(s, nested, doc::group(doc::concat(docs)))
    }

    /// A chain of `<<`: the first stays, the next ones line up with it when the chain breaks.
    fn shifts(&self, s: usize, e: usize, cuts: &[usize], nested: bool, tr: &Doc) -> Doc {
        let first = cuts[0];
        let head = self.seq(s, first, nested, &Doc::Nil);
        let rest = &cuts[1..];
        let docs = self.interleave(self.parts(first, e, rest, nested, tr), rest);
        doc::concat(vec![
            head,
            space(self.ann[first].space),
            doc::align_here(doc::group(doc::concat(docs))),
        ])
    }

    fn binary(&self, s: usize, e: usize, cuts: &[usize], nested: bool, tr: &Doc) -> Doc {
        let docs = self.interleave(self.parts(s, e, cuts, nested, tr), cuts);
        self.continued(s, nested, doc::fill(docs))
    }

    fn generic(&self, s: usize, e: usize, cuts: &[usize], nested: bool, tr: &Doc) -> Doc {
        let docs = self.parts(s, e, cuts, nested, tr);
        let base = cuts.iter().any(|&c| self.ann[c].base);
        let mut it = docs.into_iter();
        let first = it.next().unwrap_or_else(doc::nil);
        let mut rest = Vec::new();
        for (i, d) in it.enumerate() {
            rest.push(self.gap(cuts[i]));
            rest.push(d);
        }
        if cuts.len() == 1 && rest.len() == 2 {
            // One gap: it breaks when what follows it does not fit on the row.
            let next = rest.pop().unwrap_or_else(doc::nil);
            let gap = rest.pop().unwrap_or_else(doc::nil);
            let tail = doc::group(doc::concat(vec![gap, next]));
            let tail = if base || nested {
                tail
            } else {
                doc::indent(tail)
            };
            return doc::concat(vec![first, tail]);
        }
        let tail = doc::concat(rest);
        let tail = if base || nested {
            tail
        } else {
            doc::indent(tail)
        };
        doc::group(doc::concat(vec![first, tail]))
    }
}
