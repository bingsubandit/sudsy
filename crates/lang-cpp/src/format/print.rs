//! Printing the items: indentation, braces, blank lines, comments, and the rows of each line.
//!
//! concept: none (braces, blank lines, comments and directive rows of cpp; local until the shared block and comment shapes read the cpp tree)

use super::annotate::{Cx, annotate};
use super::model::{Block, Dir, Elem, Item, ItemKind, Line, Tk};
use super::style::{Style, Wrap};
use super::wrap::{Wrapper, width};
use crate::Dialect;
use crate::generated::kind as k;

/// One output line: code, an optional trailing comment, and the blank lines before it.
pub struct Row {
    pub text: String,
    pub comment: Option<String>,
    pub blank: u32,
    /// A row of a multi-line token or macro: never aligned.
    pub fixed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    File,
    Namespace,
    Record,
    Body,
    Switch,
}

pub struct Printer<'a> {
    pub src: &'a str,
    pub style: &'a Style,
    pub dialect: Dialect,
    pub rows: Vec<Row>,
    blank: u32,
    /// Items printed in the current block, to leave out blank lines at its start.
    first: bool,
    /// Check each item against the source and its own reformat (off for the reformat itself).
    pub verify: bool,
}

fn is_ident(t: &Tk) -> bool {
    matches!(t.kind, k::IDENT | k::KEYWORD)
}

/// A header and its one simple statement that the style keeps on one line (`if (x) return;`,
/// `case 1: break;`).
fn short_pair(style: &Style, head: &Line, body: &Line, label: bool, after: Option<&Item>) -> bool {
    let plain = |l: &Line| {
        !l.has_dir
            && l.elems
                .iter()
                .all(|e| matches!(e, Elem::T(t) if !t.comment()))
    };
    if !plain(head) || !plain(body) {
        return false;
    }
    let (h, b) = (first_tokens(head), first_tokens(body));
    let (Some(first), Some(last)) = (h.first(), h.last()) else {
        return false;
    };
    if b.last().is_none_or(|t| t.kind != k::SEMI)
        || b.first()
            .is_some_and(|t| control_word(t) || t.word("else") || t.word("do"))
        || case_label(&b)
    {
        return false;
    }
    let next_first = after.and_then(|x| match &x.kind {
        ItemKind::Line(l) => first_tokens(l).first().copied(),
        _ => None,
    });
    if label {
        return style.short_cases && after.is_none_or(|x| matches!(&x.kind, ItemKind::Line(l) if case_label(&first_tokens(l)) || first_tokens(l).first().is_some_and(|t| t.kind == k::RBRACE)));
    }
    if last.kind != k::RPAR {
        return false;
    }
    match first.text {
        "if" => match style.short_ifs.as_str() {
            "all" => true,
            "without_else" => !next_first.is_some_and(|t| t.word("else")),
            _ => false,
        },
        "for" | "while" => style.short_loops,
        _ => false,
    }
}

fn control_word(t: &Tk) -> bool {
    t.kind == k::KEYWORD && matches!(t.text, "if" | "for" | "while" | "switch" | "catch")
}

/// The kind of block a header opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Namespace,
    Extern,
    Class,
    Struct,
    Union,
    Enum,
    Function,
    Control,
    Lambda,
    Bare,
}

fn block_kind(toks: &[Tk], depth: i32) -> Kind {
    if depth > 0 {
        return Kind::Lambda;
    }
    let real: Vec<&Tk> = toks.iter().filter(|t| !t.comment()).collect();
    let Some(first) = real.first() else {
        return Kind::Bare;
    };
    let words = |w: &str| real.iter().any(|t| t.kind == k::KEYWORD && t.text == w);
    if first.word("else") || first.word("do") || first.word("try") || control_word(first) {
        return Kind::Control;
    }
    if first.kind == k::RBRACE
        && real
            .get(1)
            .is_some_and(|t| t.word("else") || t.word("catch") || t.word("while"))
    {
        return Kind::Control;
    }
    if real.last().is_some_and(|t| t.kind == k::COLON) {
        return Kind::Bare;
    }
    if words("namespace") {
        return Kind::Namespace;
    }
    // `extern "C" {` opens a linkage block; `extern "C" int f() {` is a function.
    if first.word("extern") && real.get(1).is_some_and(|t| t.kind == k::STRING) && real.len() <= 3 {
        return Kind::Extern;
    }
    let paren = real.iter().any(|t| t.kind == k::LPAR);
    if !paren
        || real.iter().position(|t| t.kind == k::LPAR)
            > real
                .iter()
                .position(|t| matches!(t.text, "class" | "struct" | "union" | "enum"))
            && real
                .iter()
                .any(|t| matches!(t.text, "class" | "struct" | "union" | "enum"))
    {
        if words("enum") {
            return Kind::Enum;
        }
        if words("class") {
            return Kind::Class;
        }
        if words("struct") {
            return Kind::Struct;
        }
        if words("union") {
            return Kind::Union;
        }
    }
    if paren {
        return Kind::Function;
    }
    Kind::Bare
}

fn wrap_for(kind: Kind) -> Option<Wrap> {
    Some(match kind {
        Kind::Namespace => Wrap::Namespace,
        Kind::Extern => Wrap::Extern,
        Kind::Class => Wrap::Class,
        Kind::Struct => Wrap::Struct,
        Kind::Union => Wrap::Union,
        Kind::Enum => Wrap::Enum,
        Kind::Function => Wrap::Function,
        Kind::Control => Wrap::Control,
        Kind::Lambda | Kind::Bare => return None,
    })
}

struct Seg<'a, 'b> {
    toks: Vec<Tk<'a>>,
    block: Option<&'b Block<'a>>,
}

fn segments<'a, 'b>(elems: &[&'b Elem<'a>]) -> Vec<Seg<'a, 'b>> {
    let mut segs = Vec::new();
    let mut toks = Vec::new();
    for e in elems {
        match *e {
            Elem::T(t) => toks.push(*t),
            Elem::Block(b) => segs.push(Seg {
                toks: std::mem::take(&mut toks),
                block: Some(b),
            }),
        }
    }
    segs.push(Seg { toks, block: None });
    segs
}

fn synth<'a>(kind: core_syntax::SyntaxKind, text: &'static str, blk: bool) -> Tk<'a> {
    Tk {
        kind,
        text,
        nl: 0,
        blk,
        sp: true,
    }
}

/// Whether a block can stay on the line of its header: it is empty, or holds one simple
/// statement.
fn inline_tokens<'a>(b: &Block<'a>) -> Option<Vec<Tk<'a>>> {
    let close = b.close?;
    let mut out = vec![Tk {
        blk: true,
        ..b.open
    }];
    match b.items.as_slice() {
        [] => {}
        [
            Item {
                kind: ItemKind::Comment(c),
                ..
            },
        ] if c.nl == 0 && c.text.starts_with("/*") && !c.text.contains('\n') => out.push(*c),
        [
            Item {
                kind: ItemKind::Line(l),
                ..
            },
        ] if !l.has_dir => {
            for e in &l.elems {
                match e {
                    Elem::T(t) if !t.comment() => out.push(*t),
                    _ => return None,
                }
            }
        }
        _ => return None,
    }
    out.push(Tk { blk: true, ..close });
    Some(out)
}

impl<'a> Printer<'a> {
    pub fn new(src: &'a str, style: &'a Style, dialect: Dialect) -> Self {
        Printer {
            src,
            style,
            dialect,
            rows: Vec::new(),
            blank: 0,
            first: true,
            verify: true,
        }
    }

    fn offset(&self, text: &str) -> usize {
        text.as_ptr() as usize - self.src.as_ptr() as usize
    }

    /// The text of the line a token is on, up to the token.
    fn column_of(&self, text: &str) -> usize {
        let off = self.offset(text);
        let line = self.src[..off].rfind('\n').map_or(0, |i| i + 1);
        self.src[line..off].chars().count()
    }

    fn push_row(&mut self, text: String, comment: Option<String>, fixed: bool) {
        let blank = std::mem::take(&mut self.blank);
        self.rows.push(Row {
            text,
            comment,
            blank,
            fixed,
        });
    }

    fn pad(&self, col: usize) -> String {
        " ".repeat(col)
    }

    pub fn items(&mut self, items: &[Item<'a>], base: usize, scope: Scope, ns_depth: usize) {
        let style = self.style;
        let mut pend: Vec<(usize, bool, bool)> = Vec::new(); // header column, is `if`, is `do`
        let mut in_case = false;
        let mut i = 0;
        self.first = true;
        let n = items.len();
        while i < n {
            let item = &items[i];
            let blank = item.blank.min(style.blank_lines as u32);
            if !self.first || scope != Scope::File && scope != Scope::Body {
                self.blank = blank;
            }
            if self.first && scope == Scope::File {
                self.blank = 0;
            }
            match &item.kind {
                ItemKind::Comment(c) => {
                    // A comment takes the indentation of what follows it: a directive has none.
                    let before_directive = items[i + 1..]
                        .iter()
                        .find(|x| !matches!(x.kind, ItemKind::Comment(_)))
                        .is_some_and(|x| matches!(x.kind, ItemKind::Dir(_)));
                    let col = if before_directive && scope != Scope::File {
                        0
                    } else {
                        self.item_col(base, &pend, in_case, scope, None)
                    };
                    self.comment_item(c, col);
                }
                ItemKind::Dir(d) => {
                    self.directive(d);
                }
                ItemKind::Line(l) => {
                    let toks = first_tokens(l);
                    let label = case_label(&toks);
                    let access = access_spec(&toks, scope);
                    let is_else = toks.first().is_some_and(|t| t.word("else"));
                    let mut col = self.item_col(base, &pend, in_case, scope, Some(&toks));
                    if label {
                        in_case = true;
                        col = base + if style.case_labels { style.indent } else { 0 };
                        pend.clear();
                    } else if access {
                        col = (base as i64 + style.access_offset).max(0) as usize;
                        in_case = false;
                        if !self.first {
                            self.blank = self.blank.max(1);
                        }
                    }
                    let mut stmt_done = true;
                    // The `else` (or the `while` of a `do`) goes with the statement before it.
                    if is_else && let Some(pos) = pend.iter().rposition(|p| p.1) {
                        col = pend[pos].0;
                        pend.truncate(pos);
                    }
                    let next_else = items.get(i + 1).is_some_and(|x| match &x.kind {
                        ItemKind::Line(l2) => {
                            first_tokens(l2).first().is_some_and(|t| t.word("else"))
                        }
                        _ => false,
                    });
                    let next_while = items.get(i + 1).is_some_and(|x| match &x.kind {
                        ItemKind::Line(l2) => {
                            first_tokens(l2).first().is_some_and(|t| t.word("while"))
                        }
                        _ => false,
                    });
                    let _ = next_while;
                    // `case x:` followed by a block: the brace stays on the label's line.
                    let merge_brace = label
                        && items.get(i + 1).is_some_and(|x| match &x.kind {
                            ItemKind::Line(l2) => {
                                matches!(l2.elems.first(), Some(Elem::Block(_))) && x.blank == 0
                            }
                            _ => false,
                        })
                        && !style.wraps(Wrap::Case);
                    let more = if merge_brace {
                        match items.get(i + 1).map(|x| &x.kind) {
                            Some(ItemKind::Line(l2)) => Some(l2),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    let mark = (self.rows.len(), self.blank);
                    let after_next = items.get(i + 2);
                    let mut merged = false;
                    if let Some(next) = items.get(i + 1).and_then(|x| match &x.kind {
                        ItemKind::Line(l2) if x.blank == 0 => Some(l2),
                        _ => None,
                    }) && short_pair(style, l, next, label, after_next)
                    {
                        let mut all = toks.clone();
                        all.extend(first_tokens(next));
                        self.rows_for(&all, col, false, &mut true);
                        merged = self.rows.len() == mark.0 + 1 && !self.rows[mark.0].fixed;
                        if merged {
                            i += 1;
                        } else {
                            self.rows.truncate(mark.0);
                            self.blank = mark.1;
                        }
                    }
                    let mut header = if merged {
                        None
                    } else {
                        self.line(l, more, col, scope, ns_depth, label, merge_brace)
                    };
                    if !merged
                        && !merge_brace
                        && !l.has_dir
                        && !l.raw.is_empty()
                        && self.verify
                        && matches!(scope, Scope::File | Scope::Namespace | Scope::Record)
                        && !self.item_ok(mark.0, l.raw, col, scope, ns_depth)
                    {
                        self.rows.truncate(mark.0);
                        self.blank = mark.1;
                        self.raw(l, col);
                        header = None;
                    }
                    if merge_brace {
                        i += 1;
                    }
                    let _ = merge_brace;
                    if header.is_some() {
                        stmt_done = false;
                    }
                    if let Some(h) = header {
                        pend.push((col, h == Header::If, h == Header::Do));
                    }
                    if stmt_done && !label && !access {
                        // The body of the headers waiting for one is complete.
                        while let Some((c, is_if, is_do)) = pend.pop() {
                            let followed = (next_else && is_if) || (is_do && next_while);
                            if followed {
                                pend.push((c, is_if, is_do));
                                break;
                            }
                        }
                    }
                }
            }
            self.first = false;
            i += 1;
        }
        let _ = ns_depth;
    }

    fn item_col(
        &self,
        base: usize,
        pend: &[(usize, bool, bool)],
        in_case: bool,
        scope: Scope,
        _toks: Option<&[Tk]>,
    ) -> usize {
        let extra = if in_case && scope == Scope::Switch {
            self.style.indent
                + if self.style.case_labels {
                    self.style.indent
                } else {
                    0
                }
        } else {
            0
        };
        match pend.last() {
            Some(&(c, _, _)) => c + self.style.indent,
            None => base + extra,
        }
    }

    fn comment_item(&mut self, c: &Tk<'a>, col: usize) {
        let text = self.comment_text(c, col);
        let fixed = text.contains('\n');
        self.push_row(format!("{}{}", self.pad(col), text), None, fixed);
    }

    /// A comment re-indented: the lines after the first move with the first.
    fn comment_text(&self, c: &Tk<'a>, col: usize) -> String {
        let old = self.column_of(c.text);
        let lines: Vec<&str> = c.text.split('\n').collect();
        if lines.len() == 1 {
            return c.text.trim_end().to_string();
        }
        let mut out = String::from(lines[0].trim_end());
        let star = lines[1..]
            .iter()
            .all(|l| l.trim_start().starts_with('*') || l.trim().is_empty());
        for l in &lines[1..] {
            out.push('\n');
            let body = l.trim_end();
            if body.is_empty() {
                continue;
            }
            let lead = body.len() - body.trim_start().len();
            if star {
                out.push_str(&" ".repeat(col + 1));
                out.push_str(body.trim_start());
            } else {
                let new = (lead as i64 + col as i64 - old as i64).max(0) as usize;
                out.push_str(&" ".repeat(new));
                out.push_str(body.trim_start());
            }
        }
        out
    }

    fn directive(&mut self, d: &Dir<'a>) {
        let toks = &d.toks;
        let name = toks.get(1).map_or("", |t| t.text);
        let continued = toks.iter().skip(1).any(|t| t.nl > 0);
        // A comment that ends a one-line directive is a trailing comment.
        let trailing = toks.last().filter(|t| {
            t.comment()
                    && !continued
                    && toks.len() > 2
                    && !t.text.contains('\n')
                    // `#define NAME /* c */`: the comment is the macro's body.
                    && !(name == "define" && toks.len() == 4)
        });
        let body = &toks[..toks.len() - usize::from(trailing.is_some())];
        let text = if continued || body.iter().any(|t| t.comment()) {
            self.directive_multi(d)
        } else {
            self.directive_one(body, name)
        };
        let fixed = text.contains('\n');
        let comment = if fixed {
            None
        } else {
            trailing.map(|t| t.text.to_string())
        };
        self.push_row(text, comment, fixed);
    }

    fn directive_one(&self, toks: &[Tk<'a>], name: &str) -> String {
        let mut out = String::from("#");
        if toks.len() < 2 {
            return out;
        }
        out.push_str(name);
        let rest = &toks[2..];
        if rest.is_empty() {
            return out;
        }
        match name {
            "include" | "include_next" | "import" | "pragma" | "error" | "warning" | "line"
            | "ident" | "sccs" => {
                out.push(' ');
                out.push_str(&self.src_range(rest.first().unwrap(), rest.last().unwrap()));
            }
            "define" => {
                out.push(' ');
                let mut rest = rest.to_vec();
                // The macro name and its parameter list: `(` right after the name.
                out.push_str(rest[0].text);
                let params = rest.len() > 1 && rest[1].kind == k::LPAR && !rest[1].sp;
                let mut at = 1;
                if params {
                    // Parameters: tokens up to the closing parenthesis, joined by `, `.
                    out.push('(');
                    at = 2;
                    let mut first = true;
                    while at < rest.len() && rest[at].kind != k::RPAR {
                        if rest[at].kind == k::COMMA {
                            out.push_str(", ");
                            first = true;
                        } else {
                            if !first && rest[at].kind != k::ELLIPSIS {
                                out.push(' ');
                            }
                            out.push_str(rest[at].text);
                            first = false;
                        }
                        at += 1;
                    }
                    out.push(')');
                    at += 1;
                }
                if at < rest.len() {
                    let body = rest.split_off(at);
                    out.push(' ');
                    out.push_str(&self.expr_text(&body));
                }
            }
            _ => {
                out.push(' ');
                out.push_str(&self.expr_text(rest));
            }
        }
        out
    }

    fn src_range(&self, a: &Tk, b: &Tk) -> String {
        let s = self.offset(a.text);
        let e = self.offset(b.text) + b.text.len();
        self.src[s..e].to_string()
    }

    /// Tokens of a directive's expression on one line, spaced as code.
    fn expr_text(&self, toks: &[Tk<'a>]) -> String {
        let cx = Cx {
            style: self.style,
            decl: false,
        };
        let ann = annotate(toks, &cx);
        let mut out = String::new();
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            if i > 0 && (ann[i].space || t.comment()) {
                out.push(' ');
            }
            // `FMT_HAS_INCLUDE(<bit>)`: a header name is not a comparison.
            let header = (t.kind == k::LESS && i > 0 && toks[i - 1].kind == k::LPAR)
                .then(|| {
                    (i..toks.len().saturating_sub(1))
                        .find(|&g| toks[g].kind == k::GREATER && toks[g + 1].kind == k::RPAR)
                })
                .flatten();
            if let Some(g) = header {
                out.push_str(&self.src_range(t, &toks[g]));
                i = g + 1;
                continue;
            }
            out.push_str(t.text);
            i += 1;
        }
        out
    }

    /// A directive over several lines: the lines keep their breaks and their indentation, the
    /// backslashes line up.
    fn directive_multi(&self, d: &Dir<'a>) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        let toks = &d.toks;
        let mut prev_end = 0usize;
        // The continuation lines keep their relative indentation; the least indented one is at
        // the indent width (a macro body) or the continuation width (a condition).
        let indent_at = |off: usize| {
            let line_start = self.src[..off].rfind('\n').map_or(0, |p| p + 1);
            off - line_start
        };
        let least = toks
            .iter()
            .enumerate()
            .filter(|(i, t)| t.nl > 0 && *i > 0)
            .map(|(_, t)| indent_at(self.offset(t.text)))
            .min()
            .unwrap_or(0);
        let target = if toks.get(1).is_some_and(|t| t.word("define")) {
            self.style.indent
        } else {
            self.style.continuation
        };
        for (i, t) in toks.iter().enumerate() {
            let off = self.offset(t.text);
            if t.nl > 0 && i > 0 {
                // A line break (continuation) before this token.
                lines.push(std::mem::take(&mut cur));
                cur.push_str(&" ".repeat(target + indent_at(off) - least));
                cur.push_str(t.text);
                prev_end = off + t.text.len();
                continue;
            }
            if i == 0 {
                cur.push_str(t.text);
            } else {
                let gap = &self.src[prev_end..off];
                if i == 1 {
                    // `#  define` becomes `#define`.
                } else if gap.is_empty() {
                } else {
                    cur.push(' ');
                }
                cur.push_str(t.text);
            }
            prev_end = off + t.text.len();
        }
        lines.push(cur);
        // Backslashes.
        let limit = self.style.width.max(2);
        let longest = lines[..lines.len() - 1]
            .iter()
            .map(|l| width(l))
            .max()
            .unwrap_or(0);
        let mut out = String::new();
        let last = lines.len() - 1;
        for (i, l) in lines.iter().enumerate() {
            let l = l.trim_end();
            out.push_str(l);
            if i < last {
                let col = match self.style.escaped.as_str() {
                    "left" => longest + 1,
                    "dont_align" => width(l) + 1,
                    _ => (limit - 1).max(width(l) + 1),
                };
                out.push_str(&" ".repeat(col - width(l)));
                out.push('\\');
                out.push('\n');
            }
        }
        out
    }

    /// Prints a line; returns the kind of control header it ends in, which takes the next line
    /// as its body.
    #[allow(clippy::too_many_arguments)]
    fn line(
        &mut self,
        l: &Line<'a>,
        more: Option<&Line<'a>>,
        col: usize,
        scope: Scope,
        ns_depth: usize,
        label: bool,
        merge_brace: bool,
    ) -> Option<Header> {
        if l.has_dir || more.is_some_and(|m| m.has_dir) {
            self.raw(l, col);
            return None;
        }
        let elems: Vec<&Elem<'a>> = l
            .elems
            .iter()
            .chain(more.iter().flat_map(|m| m.elems.iter()))
            .collect();
        let segs = segments(&elems);
        let decl = matches!(scope, Scope::File | Scope::Namespace | Scope::Record);
        let mut acc: Vec<Tk<'a>> = Vec::new();
        let mut depth = 0i32;
        let mut first_row = true;
        let mut header = None;
        let last = segs.len() - 1;
        for (si, seg) in segs.iter().enumerate() {
            acc.extend(seg.toks.iter().copied());
            for t in &seg.toks {
                match t.kind {
                    k::LPAR | k::LSQB | k::LBRACE => depth += 1,
                    k::RPAR | k::RSQB | k::RBRACE => depth -= 1,
                    _ => {}
                }
            }
            let Some(block) = seg.block else {
                // The end of the line.
                if si == last {
                    header = self.end_header(&acc, label);
                }
                if let Some(at) = empty_body(&acc) {
                    let body = acc.split_off(at);
                    self.rows_for(&acc, col, decl, &mut first_row);
                    self.rows_for(&body, col + self.style.indent, decl, &mut first_row);
                    break;
                }
                let own = &mut first_row;
                self.rows_for(&acc, col, decl, own);
                break;
            };
            let kind = block_kind(&acc, depth);
            let switch = kind == Kind::Control && acc.iter().any(|t| t.word("switch"));
            // Short blocks stay on the line.
            let wrap = wrap_for(kind).is_some_and(|w| self.style.wraps(w));
            // The empty body of a construct whose brace goes on its own line is split too.
            let split_empty = wrap
                && block.items.is_empty()
                && matches!(
                    kind,
                    Kind::Function | Kind::Class | Kind::Struct | Kind::Union | Kind::Namespace
                );
            if !split_empty
                && let Some(inline) =
                    self.inline_ok(block, kind, scope, &acc, &segs[si + 1..], col, decl)
            {
                acc.extend(inline);
                continue;
            }
            let case_brace = label && !merge_brace && self.style.wraps(Wrap::Case);
            let brace_own = (wrap || case_brace)
                && !acc.is_empty()
                && !(acc.len() == 1 && acc[0].kind == k::RBRACE);
            let own = &mut first_row;
            let mut skip = 0;
            if brace_own {
                self.rows_for(&acc, col, decl, own);
                acc.clear();
                let bcol = col
                    + if self.style.indent_braces {
                        self.style.indent
                    } else {
                        0
                    };
                self.push_row(format!("{}{{", self.pad(bcol)), None, false);
            } else {
                acc.push(synth(k::LBRACE, "{", true));
                // A comment after `{` stays on its line.
                if let Some(Item {
                    kind: ItemKind::Comment(c),
                    ..
                }) = block.items.first()
                    && c.nl == 0
                {
                    acc.push(*c);
                    skip = 1;
                }
                self.rows_for(&acc, col, decl, own);
                acc.clear();
            }
            first_row = false;
            // The contents.
            let (inner, scope2, ns2) = self.inner(kind, col, scope, ns_depth, block, switch);
            if kind == Kind::Enum
                && !block
                    .items
                    .iter()
                    .any(|i| matches!(i.kind, ItemKind::Dir(_)))
            {
                self.enum_body(block, inner, col);
            } else {
                let saved = (self.first, self.blank);
                self.items(&block.items[skip..], inner, scope2, ns2);
                let _ = saved;
            }
            // The closing brace.
            // Blank lines before a closing brace stay only for a namespace.
            self.blank = match (kind, block.close) {
                (Kind::Namespace, Some(c)) => {
                    c.nl.saturating_sub(1).min(self.style.blank_lines as u32)
                }
                _ => 0,
            };
            acc.push(synth(k::RBRACE, "}", false));
            // `else`, `catch` and the `while` of a `do` go on their own line when the style says.
            let next_tok = segs.get(si + 1).and_then(|s| s.toks.first());
            let split = next_tok.is_some_and(|t| {
                (t.word("else") && self.style.wraps(Wrap::Else))
                    || (t.word("catch") && self.style.wraps(Wrap::Catch))
                    || (t.word("while") && self.style.wraps(Wrap::While))
            });
            let close_col = col
                + if self.style.indent_braces && wrap {
                    self.style.indent
                } else {
                    0
                };
            if split {
                self.push_row(format!("{}}}", self.pad(close_col)), None, false);
                acc.clear();
            }
            if si == last {
                break;
            }
        }
        header
    }

    fn end_header(&self, acc: &[Tk<'a>], label: bool) -> Option<Header> {
        if label {
            return None;
        }
        let toks: Vec<&Tk> = acc.iter().filter(|t| !t.comment()).collect();
        let first = toks.iter().find(|t| t.kind != k::RBRACE)?;
        let last = toks.last()?;
        if first.word("else") && last.word("else") {
            return Some(Header::Else);
        }
        if first.word("do") && last.word("do") {
            return Some(Header::Do);
        }
        if (control_word(first) || first.word("else") && toks.get(1).is_some_and(|t| t.word("if")))
            && last.kind == k::RPAR
        {
            let is_if = first.word("if") || first.word("else");
            return Some(if is_if { Header::If } else { Header::Other });
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn inline_ok(
        &self,
        block: &Block<'a>,
        kind: Kind,
        scope: Scope,
        acc: &[Tk<'a>],
        rest: &[Seg<'a, '_>],
        col: usize,
        decl: bool,
    ) -> Option<Vec<Tk<'a>>> {
        let style = self.style;
        let empty = block.items.is_empty();
        let allowed = match kind {
            Kind::Function => match style.short_functions.as_str() {
                "all" => true,
                "inline" => empty || scope == Scope::Record,
                "empty" => empty,
                _ => false,
            },
            Kind::Lambda => match style.short_lambdas.as_str() {
                "all" => true,
                "inline" => true,
                "empty" => empty,
                _ => false,
            },
            Kind::Namespace | Kind::Extern => empty && block.close.is_some(),
            // `if (x) {}` is split; only an empty `switch` stays.
            Kind::Control => empty && block.close.is_some() && acc.iter().any(|t| t.word("switch")),
            Kind::Bare => false,
            Kind::Class | Kind::Struct | Kind::Union => empty,
            Kind::Enum => {
                self.style.short_enums && !block.items.is_empty() && !enum_trailing_comma(block)
            }
        };
        if !allowed {
            return None;
        }
        // A function under a `template <...>` line of its own is not joined, unless empty.
        if kind == Kind::Function
            && !empty
            && style.templates == "yes"
            && acc.first().is_some_and(|t| t.word("template"))
        {
            return None;
        }
        let tokens = inline_tokens(block)?;
        if empty && block.close.is_some() {
            return Some(tokens);
        }
        // The whole line, as far as the next block, has to fit.
        let mut all: Vec<Tk<'a>> = acc.to_vec();
        all.extend(tokens.iter().copied());
        if let Some(next) = rest.first() {
            // A comment after the block is the line's trailing comment: it does not count.
            all.extend(next.toks.iter().copied().filter(|t| !t.comment()));
        }
        let cx = Cx { style, decl };
        let ann = annotate(&all, &cx);
        if all.iter().any(|t| t.comment()) {
            return None;
        }
        let wrap = Wrapper::new(&all, &ann, style, decl, col, style.width);
        let fits = style.width == 0 || wrap.one_line_end(col) <= style.width;
        fits.then_some(tokens)
    }

    fn inner(
        &self,
        kind: Kind,
        col: usize,
        scope: Scope,
        ns_depth: usize,
        block: &Block<'a>,
        switch: bool,
    ) -> (usize, Scope, usize) {
        let style = self.style;
        let _ = block;
        let brace_extra = if style.indent_braces { style.indent } else { 0 };
        match kind {
            Kind::Namespace => {
                let indent = match style.namespace_indent.as_str() {
                    "all" => style.indent,
                    "inner" if ns_depth > 0 => style.indent,
                    _ => 0,
                };
                (col + indent, Scope::Namespace, ns_depth + 1)
            }
            Kind::Extern => (col, scope, ns_depth),
            Kind::Class | Kind::Struct | Kind::Union => {
                (col + style.indent + brace_extra, Scope::Record, ns_depth)
            }
            Kind::Control if switch => (col + brace_extra, Scope::Switch, ns_depth),
            _ => (col + style.indent + brace_extra, Scope::Body, ns_depth),
        }
    }

    fn enum_body(&mut self, block: &Block<'a>, inner: usize, col: usize) {
        let _ = col;
        let mut toks: Vec<Tk<'a>> = Vec::new();
        for it in &block.items {
            match &it.kind {
                ItemKind::Line(l) => {
                    for e in &l.elems {
                        if let Elem::T(t) = e {
                            toks.push(*t);
                        }
                    }
                }
                ItemKind::Comment(c) => toks.push(*c),
                _ => {}
            }
        }
        let trailing = toks
            .iter()
            .rev()
            .find(|t| !t.comment())
            .is_some_and(|t| t.kind == k::COMMA);
        // One enumerator per line.
        let mut line: Vec<Tk<'a>> = Vec::new();
        let mut depth = 0i32;
        let mut groups: Vec<Vec<Tk<'a>>> = Vec::new();
        for t in toks {
            match t.kind {
                k::LPAR | k::LSQB | k::LBRACE => depth += 1,
                k::RPAR | k::RSQB | k::RBRACE => depth -= 1,
                _ => {}
            }
            let comma = t.kind == k::COMMA && depth == 0;
            // A comment on a line of its own is a row of its own.
            if t.comment() && t.nl > 0 && depth == 0 {
                if !line.is_empty() {
                    groups.push(std::mem::take(&mut line));
                }
                groups.push(vec![t]);
                continue;
            }
            if t.comment()
                && t.nl == 0
                && line.is_empty()
                && let Some(prev) = groups.last_mut()
            {
                prev.push(t);
                continue;
            }
            line.push(t);
            if comma {
                groups.push(std::mem::take(&mut line));
            }
        }
        if !line.is_empty() {
            groups.push(line);
        }
        let _ = trailing;
        for g in groups {
            let mut first = true;
            self.blank = 0;
            self.rows_for(&g, inner, true, &mut first);
        }
    }

    /// Whether the rows since `from` mean what the source text `raw` did and are what formatting
    /// them again gives.
    fn item_ok(&self, from: usize, raw: &str, col: usize, scope: Scope, ns_depth: usize) -> bool {
        let out = self.render_rows(&self.rows[from..]);
        let out = out.trim_start_matches('\n');
        if !super::verify::same(raw, out, self.dialect) {
            return false;
        }
        if scope == Scope::Record {
            return true;
        }
        let tree = crate::parse(out, self.dialect).tree;
        let items = super::model::build(&tree, out);
        let mut again = Printer::new(out, self.style, self.dialect);
        again.verify = false;
        again.items(&items, col, scope, ns_depth);
        let second = again.render();
        second.trim_start_matches('\n') == out
    }

    fn raw(&mut self, l: &Line<'a>, col: usize) {
        let text = format!("{}{}", self.pad(col), l.raw);
        self.push_row(text, None, true);
    }

    /// Lays out the tokens as rows; the first starts at `col`.
    fn rows_for(&mut self, toks: &[Tk<'a>], col: usize, decl: bool, _first: &mut bool) {
        if toks.is_empty() {
            return;
        }
        let style = self.style;
        let cx = Cx { style, decl };
        let mut ann = annotate(toks, &cx);
        if !style.wrap_lines {
            for (i, a) in ann.iter_mut().enumerate().skip(1) {
                if toks[i].nl > 0 {
                    a.must = true;
                    a.can = true;
                }
            }
        }
        let limit = if style.wrap_lines { style.width } else { 0 };
        let mut fits = Wrapper::new(toks, &ann, style, decl, col, limit).one_line_end(col) <= limit
            || limit == 0;
        let mut head = 0;
        // The declaration after `template <...>`.
        if toks[0].word("template")
            && toks.len() > 2
            && ann[1].role == super::annotate::Role::TplOpen
        {
            let mut depth = 0i32;
            let at = (1..toks.len()).find(|&j| {
                match (ann[j].role, toks[j].kind) {
                    (super::annotate::Role::TplOpen, _) => depth += 1,
                    (super::annotate::Role::TplClose, k::GREATER_GREATER) => depth -= 2,
                    (super::annotate::Role::TplClose, _) => depth -= 1,
                    _ => {}
                }
                depth <= 0
            });
            if let Some(at) = at.map(|j| j + 1).filter(|&at| at < toks.len()) {
                if style.templates == "yes" || (style.templates == "multiline" && !fits) {
                    ann[at].must = true;
                    head = at;
                }
                ann[at].can = style.templates != "no" || ann[at].can;
            }
        }
        if head > 0 {
            fits = Wrapper::new(&toks[head..], &ann[head..], style, decl, col, limit)
                .one_line_end(col)
                <= limit;
        }
        if !fits {
            for (i, a) in ann.iter_mut().enumerate() {
                if a.role == super::annotate::Role::CtorColon && i > 0 {
                    a.must = true;
                }
            }
            if style.init == super::style::Init::BeforeComma {
                let mut after = false;
                for i in 1..toks.len() {
                    if ann[i].role == super::annotate::Role::CtorColon {
                        after = true;
                    }
                    if after && ann[i].level == 0 && toks[i].kind == k::COMMA {
                        ann[i].must = true;
                        ann[i].can = true;
                        ann[i].base = true;
                        ann[i].space = false;
                    }
                }
            }
        }
        for r in Wrapper::new(toks, &ann, style, decl, col, limit).rows() {
            let text = format!("{}{}", self.pad(col), r.text);
            self.push_row(text, r.comment, r.fixed);
        }
    }

    /// The output text: trailing comments aligned, blank lines in.
    pub fn render(&self) -> String {
        self.render_rows(&self.rows)
    }

    fn render_rows(&self, rows: &[Row]) -> String {
        let style = self.style;
        let mut out = String::new();
        let mut i = 0;
        while i < rows.len() {
            // A group of consecutive rows that end in a comment.
            let mut j = i;
            while j < rows.len() && rows[j].comment.is_some() && (j == i || rows[j].blank == 0) {
                j += 1;
            }
            let target = if style.align_comments && j > i {
                rows[i..j]
                    .iter()
                    .map(|r| width(&r.text) + style.comment_spaces.max(1))
                    .max()
                    .unwrap_or(0)
            } else {
                0
            };
            let end = j.max(i + 1);
            for (n, r) in rows[i..end].iter().enumerate() {
                for _ in 0..r.blank {
                    out.push('\n');
                }
                let _ = n;
                if style.tabs && !r.fixed {
                    let lead = r.text.len() - r.text.trim_start_matches(' ').len();
                    out.push_str(&"\t".repeat(lead / style.indent.max(1)));
                    out.push_str(&" ".repeat(lead % style.indent.max(1)));
                    out.push_str(&r.text[lead..]);
                } else {
                    out.push_str(&r.text);
                }
                if let Some(c) = &r.comment {
                    let w = width(&r.text);
                    let gap = if style.align_comments {
                        target.saturating_sub(w)
                    } else {
                        style.comment_spaces.max(1)
                    };
                    if !r.text.trim().is_empty() {
                        out.push_str(&" ".repeat(gap.max(style.comment_spaces)));
                    }
                    out.push_str(c);
                }
                out.push('\n');
            }
            i = end;
        }
        out
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Header {
    If,
    Else,
    Do,
    Other,
}

fn enum_trailing_comma(block: &Block) -> bool {
    block.items.iter().any(|it| match &it.kind {
        ItemKind::Line(l) => l
            .elems
            .iter()
            .rev()
            .find_map(|e| match e {
                Elem::T(t) if !t.comment() => Some(t.kind == k::COMMA),
                _ => None,
            })
            .unwrap_or(false),
        _ => false,
    })
}

fn first_tokens<'a>(l: &Line<'a>) -> Vec<Tk<'a>> {
    l.elems
        .iter()
        .map_while(|e| match e {
            Elem::T(t) if !t.comment() => Some(*t),
            _ => None,
        })
        .collect()
}

/// `while (x);`: where the `;` of an empty body starts.
fn empty_body(toks: &[Tk]) -> Option<usize> {
    let real: Vec<usize> = (0..toks.len()).filter(|&i| !toks[i].comment()).collect();
    let first = *real.first()?;
    if !(control_word(&toks[first]) && !toks[first].word("catch")) {
        return None;
    }
    let semi = *real.last()?;
    if toks[semi].kind != k::SEMI || real.len() < 3 {
        return None;
    }
    let before = real[real.len() - 2];
    if toks[before].kind != k::RPAR {
        return None;
    }
    // The parenthesis that `before` closes must be the one after the keyword.
    let mut depth = 0;
    for &i in &real[1..real.len() - 1] {
        match toks[i].kind {
            k::LPAR => depth += 1,
            k::RPAR => {
                depth -= 1;
                if depth == 0 && i != before {
                    return None;
                }
            }
            _ => {}
        }
    }
    Some(semi)
}

fn case_label(toks: &[Tk]) -> bool {
    toks.first()
        .is_some_and(|t| t.word("case") || t.word("default"))
        && toks.last().is_some_and(|t| t.kind == k::COLON)
}

fn access_spec(toks: &[Tk], scope: Scope) -> bool {
    scope == Scope::Record
        && toks.len() == 2
        && toks[0].kind == k::KEYWORD
        && matches!(toks[0].text, "public" | "private" | "protected")
        && toks[1].kind == k::COLON
}

#[allow(dead_code)]
fn unused(_: &[Tk]) -> bool {
    is_ident(&synth(k::IDENT, "", false))
}
