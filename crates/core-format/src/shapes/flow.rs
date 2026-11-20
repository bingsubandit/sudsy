//! The children of a node one after the other, for the syntax no other shape lays out: the
//! spacing between two tokens is the language's rule (`Flow::want`), a line break the source has
//! stays one, and everything after the first one hangs below what stands before it.
//!
//! concept: token-flow-layout
//! what: Children one after another with the spacing rows and the line breaks the source has.
//! tools: none (internal)
//! reads: rules:flow, rules:space

use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::seq::{el, first, kids, last};
use crate::spacing::SpaceRules;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// The language's part of [`Flow::seq`].
pub struct Flow<'a> {
    /// Should a blank separate two tokens of one line? `None`: as the source has it.
    pub want: fn(&SyntaxToken, &SyntaxToken) -> Option<bool>,
    /// A line break after this element does not start a hanging part (modifiers, attributes).
    pub flat_after: fn(&SyntaxElement) -> bool,
    /// A line break before a token of one of these kinds ends the hanging part and the token
    /// stands at the level of the line that opened it (a closing parenthesis on a line of its own).
    pub closers: &'a [SyntaxKind],
    /// A token of one of these kinds ends the hanging part even without a line break before it
    /// (the `{` of a body after a condition that spans lines).
    pub unhang: &'a [SyntaxKind],
    /// Two tokens that stay on one line although the source breaks between them (`} else`).
    pub joined: fn(&SyntaxToken, &SyntaxToken) -> bool,
    /// Two tokens that need a blank between them when `want` has no opinion and the source has none.
    pub at_least_one: fn(&SyntaxToken, &SyntaxToken) -> bool,
    /// An operator that starts the next line when the source ends a line with it.
    pub op_first: fn(&SyntaxToken) -> bool,
    /// Where `want` has no opinion the blanks of the source stay as they are (else they become one).
    pub exact: bool,
    /// A line after a break is indented by as many columns as the source indents it below the
    /// line the node starts on (else one level for all of them).
    pub source_hang: bool,
    /// Blank lines of the source stay.
    pub keep_blank_lines: bool,
    /// The blank between a block comment and the token after it is as the source has it
    /// (`f(/*name=*/x)`), also where `exact` is off.
    pub keep_comment_gap: bool,
    /// A line break of the source is a breakable line (the group around decides), not a hard one:
    /// the items of a list that is filled to the line width.
    pub reflow: bool,
    /// Rows that decide `want`, `joined`, `at_least_one`, `op_first`, `flat_after`, `closers`
    /// and `unhang` instead of the fields of those names.
    pub rules: Option<&'a SpaceRules>,
}

thread_local! {
    static REFLOW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `f` with the flows it creates breaking at the source's line breaks as soft lines (`on`) or
/// as hard ones.
pub fn with_reflow<T>(on: bool, f: impl FnOnce() -> T) -> T {
    let before = REFLOW.with(|c| c.replace(on));
    let out = f();
    REFLOW.with(|c| c.set(before));
    out
}

/// Do the flows created now reflow ([`with_reflow`])?
pub fn reflowing() -> bool {
    REFLOW.with(|c| c.get())
}

/// Columns of blanks in front of the first token on the source line of `t` (a tab is `tab`).
fn line_indent(t: &SyntaxToken, tab: usize) -> usize {
    line_prefix(t)
        .chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .map(|c| if c == '\t' { tab } else { 1 })
        .sum()
}

/// Columns in front of `t` on its source line (a tab is `tab`).
fn column_of(t: &SyntaxToken, tab: usize) -> usize {
    line_prefix(t)
        .chars()
        .map(|c| if c == '\t' { tab } else { 1 })
        .sum()
}

fn line_prefix(t: &SyntaxToken) -> String {
    let mut line = String::new();
    let mut cur = t.prev_token();
    while let Some(p) = cur {
        let text = p.text();
        // A comment of several lines counts from the line it starts on.
        if text.starts_with("/*")
            && let Some(i) = text.find('\n')
        {
            line.insert_str(0, &text[..i]);
            cur = p.prev_token();
            continue;
        }
        if let Some(i) = text.rfind('\n') {
            line.insert_str(0, &text[i + 1..]);
            break;
        }
        line.insert_str(0, text);
        cur = p.prev_token();
    }
    line
}

/// The next token that is not trivia.
fn next_code(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Option<SyntaxToken> {
    let mut cur = t.next_token();
    while let Some(n) = cur {
        if !cx.grammar.is_trivia(n.kind()) {
            return Some(n);
        }
        cur = n.next_token();
    }
    None
}

/// A part of a node that hangs below what stands before it: `delta` columns, or one level.
struct Hung {
    delta: Option<usize>,
    docs: Vec<Doc>,
}

fn flush(hung: &mut Vec<Hung>, parts: &mut Vec<Doc>) {
    for h in hung.drain(..) {
        let body = doc::concat(h.docs);
        parts.push(match h.delta {
            Some(n) => doc::align(n, body),
            None => doc::indent(body),
        });
    }
}

impl Flow<'_> {
    fn want(&self, a: &SyntaxToken, b: &SyntaxToken) -> Option<bool> {
        self.rules
            .map_or_else(|| (self.want)(a, b), |r| r.want(a, b))
    }

    fn joined(&self, a: &SyntaxToken, b: &SyntaxToken) -> bool {
        self.rules
            .map_or_else(|| (self.joined)(a, b), |r| r.joined(a, b))
    }

    fn at_least_one(&self, a: &SyntaxToken, b: &SyntaxToken) -> bool {
        self.rules
            .map_or_else(|| (self.at_least_one)(a, b), |r| r.at_least_one(a, b))
    }

    fn op_first(&self, t: &SyntaxToken) -> bool {
        self.rules
            .map_or_else(|| (self.op_first)(t), |r| r.op_first(t))
    }

    fn flat_after(&self, e: &SyntaxElement) -> bool {
        self.rules
            .map_or_else(|| (self.flat_after)(e), |r| r.flat_after(e.kind()))
    }

    fn closer(&self, kind: SyntaxKind) -> bool {
        self.closers.contains(&kind) || self.rules.is_some_and(|r| r.closer(kind))
    }

    fn unhangs(&self, t: &SyntaxToken) -> bool {
        self.unhang.contains(&t.kind()) || self.rules.is_some_and(|r| r.unhangs(t))
    }

    /// An opening brace goes on its own line (`Some(false)`) or on the line of its construct
    /// (`Some(true)`) as the language's option says, whatever the source has; `None` where no
    /// row decides or a line comment ends the line.
    pub fn attach(&self, cx: &FormatCtx<'_>, a: &SyntaxToken, b: &SyntaxToken) -> Option<bool> {
        let rules = self.rules.filter(|r| r.has_attach())?;
        let (name, word) = rules.attach_word(b)?;
        if comments::trailing_comments(cx.grammar, a)
            .iter()
            .any(|c| c.kind == comments::CommentKind::Line)
            || !comments::leading_comments(cx.grammar, b).is_empty()
        {
            return None;
        }
        Some(!cx.options.strings(name).iter().any(|w| w == word))
    }

    /// What stands between two neighbouring tokens: a line break the source has, else a blank
    /// where the spacing rule or the source wants one.
    pub fn between(&self, cx: &FormatCtx<'_>, a: &SyntaxToken, b: &SyntaxToken) -> Option<Doc> {
        if let Some(attached) = self.attach(cx, a, b) {
            return Some(if attached {
                doc::lit(" ")
            } else {
                doc::hardline()
            });
        }
        let broken = comments::break_after(cx.grammar, a);
        if broken && self.op_first(a) {
            return Some(doc::lit(" "));
        }
        if !broken && self.op_first(b) && comments::break_after(cx.grammar, b) {
            return Some(self.line_break(cx, a, b));
        }
        if broken && !self.joined(a, b) {
            return Some(self.line_break(cx, a, b));
        }
        let gap = a.text_range().end() != b.text_range().start();
        // `f(a, /*name=*/b)`: the blank between a block comment and `b` is as the source has it.
        if (self.exact || self.keep_comment_gap)
            && gap
            && let Some(p) = b.prev_token()
        {
            let space = p.text().ends_with([' ', '\t']);
            let comment = if space {
                p.prev_token()
            } else {
                Some(p.clone())
            };
            if let Some(c) = comment
                .filter(|c| c != a && c.text().starts_with("/*") && cx.grammar.is_trivia(c.kind()))
            {
                let glued = cx.comment_style.is_glued(c.text().as_ref());
                return (space && !glued).then(|| doc::lit(" "));
            }
        }
        let pointer = self.rules.and_then(|r| {
            let (before, after) = match cx.options.get("pointer_alignment") {
                Some(core_config::OptionValue::String(s)) => match s.as_str() {
                    "left" => (false, true),
                    "middle" => (true, true),
                    _ => (true, false),
                },
                _ => return None,
            };
            r.pointer_gap(a, b, before, after)
        });
        match pointer.or_else(|| self.want(a, b)) {
            Some(blank) => blank.then(|| doc::lit(" ")),
            None if broken => Some(doc::lit(" ")),
            None if !gap => self.at_least_one(a, b).then(|| doc::lit(" ")),
            None if self.exact => Some(blanks(cx, a, b)),
            None => Some(doc::lit(" ")),
        }
    }

    /// A line break, and the blank lines the source has in front of `b`.
    fn line_break(&self, cx: &FormatCtx<'_>, a: &SyntaxToken, b: &SyntaxToken) -> Doc {
        let blank = if self.keep_blank_lines {
            comments::blank_lines_before(cx.grammar, b)
        } else {
            0
        };
        if blank == 0 {
            let ends_line = comments::trailing_comments(cx.grammar, a)
                .iter()
                .any(|c| c.kind != comments::CommentKind::Block)
                || !comments::leading_comments(cx.grammar, b).is_empty()
                // (adjacent string literals: each stays on its own line)
                || (cx.grammar.has_tag(a.kind(), "string") && cx.grammar.has_tag(b.kind(), "string"));
            return if self.reflow && !ends_line {
                // (where the spacing rows want no blank, a flat break is none: `{` `.x`)
                if self.want(a, b) == Some(false) {
                    doc::softline()
                } else {
                    doc::line()
                }
            } else {
                doc::hardline()
            };
        }
        doc::concat((0..=blank).map(|_| doc::hardline()).collect())
    }

    /// The children of `node` one after the other with the spacing of [`Flow::between`] and no
    /// hanging part: a line break of the source stays, a declaration's annotation has a line
    /// of its own (modifiers, attributes).
    pub fn flat(&self, cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
        let els = kids(cx, node);
        let mut parts = Vec::with_capacity(els.len() * 2);
        for (i, e) in els.iter().enumerate() {
            if i > 0
                && let (Some(a), Some(b)) = (last(cx, &els[i - 1]), first(cx, e))
            {
                if super::annotation_line(cx, &els[i - 1]) {
                    parts.push(doc::hardline());
                } else {
                    parts.extend(self.between(cx, &a, &b));
                }
            }
            parts.push(el(cx, e)?);
        }
        Ok(doc::concat(parts))
    }

    /// The elements one after the other, each laid out by [`el`].
    pub fn seq(&self, cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> Result<Doc, FormatError> {
        self.seq_with(cx, els, |e| el(cx, e))
    }

    /// [`Flow::seq`] with `doc` laying out the elements.
    pub fn seq_with(
        &self,
        cx: &FormatCtx<'_>,
        els: &[SyntaxElement],
        doc: impl Fn(&SyntaxElement) -> Result<Doc, FormatError>,
    ) -> Result<Doc, FormatError> {
        let tab = if self.source_hang {
            cx.options.int("indent_size") as usize
        } else {
            0
        };
        let mut parts: Vec<Doc> = Vec::with_capacity(els.len() * 2);
        let mut hung: Vec<Hung> = Vec::new();
        let mut prev: Option<(&SyntaxElement, SyntaxToken)> = None;
        // (the hangs of a reflowed list count from the column its first item starts in)
        let base = |first_el: &SyntaxElement| {
            first(cx, first_el).map(|t| {
                if self.reflow {
                    column_of(&t, tab)
                } else {
                    line_indent(&t, tab)
                }
            })
        };
        for e in els {
            let mut d = doc(e)?;
            if let (Some((p, a)), Some(b)) = (prev.take(), first(cx, e)) {
                // A declaration's annotation stands on a line of its own.
                let mut sep = if super::annotation_line(cx, p) {
                    Some(doc::hardline())
                } else {
                    self.between(cx, &a, &b)
                };
                // An operator that moves to the start of the next line leaves its comment on the
                // line it comes from.
                if let SyntaxElement::Token(t) = e
                    && self.op_first(t)
                    && !comments::trailing_comments(cx.grammar, t).is_empty()
                    && sep.as_ref().is_some_and(Doc::will_break)
                    && !comments::break_after(cx.grammar, &a)
                {
                    sep = sep.map(|s| doc::concat(vec![cx.trailing_comments_doc(t), s]));
                    d = cx.token_text(t);
                }
                let broken = sep.as_ref().is_some_and(Doc::will_break);
                let closing = broken && self.closer(b.kind());
                let comments_before = match e {
                    SyntaxElement::Token(t) if closing && !hung.is_empty() => {
                        comments::leading_comments(cx.grammar, t)
                    }
                    _ => Vec::new(),
                };
                if closing && !comments_before.is_empty() {
                    // Comments in front of the closing token hang like the lines above them.
                    if let Some(h) = hung.last_mut() {
                        h.docs
                            .push(super::dangling_comments(&comments_before, false));
                    }
                    flush(&mut hung, &mut parts);
                    parts.push(doc::hardline());
                    parts.push(cx.token_text(&b));
                    parts.push(cx.trailing_comments_doc(&b));
                } else if closing || (!hung.is_empty() && self.unhangs(&b)) {
                    flush(&mut hung, &mut parts);
                    parts.extend(sep);
                    parts.push(d);
                } else if (broken && !self.flat_after(p)) || !hung.is_empty() {
                    if broken && (self.source_hang || hung.is_empty()) {
                        let line = if self.op_first(&b) {
                            next_code(cx, &b)
                        } else {
                            Some(b.clone())
                        };
                        let delta = self
                            .source_hang
                            .then(|| {
                                // A directive stands at column 0 whatever it is written at.
                                if line.as_ref().and_then(|t| t.parent()).is_some_and(|n| {
                                    n.descendants_with_tokens()
                                        .filter_map(|e| e.into_token())
                                        .find(|t| {
                                            let s = t.text().trim_start();
                                            !s.is_empty()
                                                && !s.starts_with("//")
                                                && !s.starts_with("/*")
                                        })
                                        .is_some_and(|f| f.text().starts_with('#'))
                                        && cx.grammar.has_tag(n.kind(), "directive")
                                }) {
                                    return Some(0);
                                }
                                let at = line.as_ref().map(|t| line_indent(t, tab))?;
                                Some(at.saturating_sub(base(&els[0])?))
                            })
                            .flatten();
                        hung.push(Hung {
                            delta,
                            docs: Vec::new(),
                        });
                    }
                    let h = hung.last_mut().expect("a part is open");
                    h.docs.extend(sep);
                    h.docs.push(d);
                } else {
                    parts.extend(sep);
                    parts.push(d);
                }
            } else if let Some(h) = hung.last_mut() {
                h.docs.push(d);
            } else {
                parts.push(d);
            }
            prev = last(cx, e).map(|t| (e, t));
        }
        flush(&mut hung, &mut parts);
        // The hangs count from the line the flow starts on, which is not always the indentation
        // it is laid out in (the arguments of `.f(` on a continuation line).
        let whole = doc::concat(parts);
        let bracketed = els
            .first()
            .and_then(|e| e.as_token())
            .is_some_and(|t| matches!(t.text(), "(" | "[" | "{"));
        Ok(if self.source_hang && bracketed {
            doc::align(doc::ALIGN_FROM_LINE, whole)
        } else {
            whole
        })
    }
}

/// The blanks between two tokens as written: one blank when a comment or a line break is between.
fn blanks(cx: &FormatCtx<'_>, a: &SyntaxToken, b: &SyntaxToken) -> Doc {
    let mut text = String::new();
    let mut cur = a.next_token();
    while let Some(t) = cur.filter(|t| t != b) {
        if !cx.grammar.has_tag(t.kind(), "whitespace") || t.text().contains(['\n', '\r']) {
            return doc::lit(" ");
        }
        text.push_str(t.text());
        cur = t.next_token();
    }
    doc::text(text)
}
