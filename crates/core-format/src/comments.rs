//! Comment placement, shared by every language and driven by the trivia attachment rule
//! (`core_syntax::trivia`): a token owns the rest of its line as *trailing* trivia and
//! everything between that and the next token as that token's *leading* trivia. Comments
//! therefore always have an owner token, and formatting code that prints tokens through
//! [`crate::FormatCtx::token`] can never lose one. A comment is *dangling* when its owner is a
//! closing delimiter of an otherwise empty (or fully consumed) container.
//!
//! Everything here reads the tree locally (token neighbours), so it works for any subtree.
//!
//! concept: comment-placement
//! what: Leading, trailing and dangling comment placement from the trivia attachment rule.
//! tools: Prettier comments, Black comments, ktlint comment-spacing
//! reads: grammar:language

use core_syntax::trivia::is_line_break;
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use std::borrow::Cow;
use std::ops::ControlFlow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    Block,
    /// An own-line comment that is a structured trivia node (a C# doc comment) or a one-line
    /// directive tagged `#indented`: printed line by line at the current indentation.
    Lines,
    /// Own-line text kept as written at column 0 (tag `#verbatim-trivia`): a preprocessor
    /// directive, disabled code. It ends its own line.
    Directive,
}

#[derive(Clone, Debug)]
pub struct Comment {
    /// A trivia token, or a structured trivia node (one element of the run, see
    /// [`core_syntax::trivia`]).
    pub element: SyntaxElement,
    pub kind: CommentKind,
    /// Line breaks between this comment and whatever follows it (the next comment or the owner
    /// token), counting the comment's own line end. Only meaningful for leading comments.
    pub breaks_after: usize,
}

impl Comment {
    /// The token of a comment that is one (every comment of a language without structured
    /// trivia).
    pub fn token(&self) -> &SyntaxToken {
        match &self.element {
            SyntaxElement::Token(t) => t,
            SyntaxElement::Node(_) => panic!("a structured trivia is not a token"),
        }
    }

    /// The comment as written, without trailing whitespace. For [`CommentKind::Directive`] only
    /// the line break that ends it is removed: the lines of disabled code are exact.
    pub fn text(&self) -> Cow<'_, str> {
        self.text_trimmed(char::is_whitespace)
    }

    /// [`Comment::text`] as `st` writes it: with `keep_unicode_blanks` only ASCII whitespace ends
    /// the text.
    pub fn text_in(&self, st: &CommentStyle) -> Cow<'_, str> {
        if st.keep_unicode_blanks {
            self.text_trimmed(|c| c.is_ascii_whitespace() || c == '\x0b')
        } else {
            self.text()
        }
    }

    fn text_trimmed(&self, blank: fn(char) -> bool) -> Cow<'_, str> {
        let raw = element_text(&self.element);
        let strip = |t: &str| -> String {
            if self.kind == CommentKind::Directive {
                t.strip_suffix("\r\n")
                    .or_else(|| t.strip_suffix(['\n', '\r']))
                    .unwrap_or(t)
                    .to_string()
            } else {
                t.trim_end_matches(blank).to_string()
            }
        };
        match raw {
            Cow::Borrowed(t) if self.kind != CommentKind::Directive => {
                Cow::Borrowed(t.trim_end_matches(blank))
            }
            other => Cow::Owned(strip(&other)),
        }
    }
}

/// How comments are printed, set by the language through `NodeFormatter::comment_style`.
#[derive(Clone, Copy, Debug)]
pub struct CommentStyle {
    /// Spaces between code and a comment after it on the same line.
    pub gap: usize,
    /// No blank line between a line comment (or a documentation comment) and what it comments:
    /// the blank lines after the last comment in front of a token are dropped, except in front
    /// of the first token of a file and after a `////` comment.
    pub attach_comments: bool,
    /// A `/** */` comment attaches as under `attach_comments`, without line comments doing so.
    pub attach_docs: bool,
    /// A comment after code keeps the blanks the source has before it when they are more than
    /// `gap` (comments aligned in a column stay aligned).
    pub keep_gap: bool,
    /// A block comment written against the code before it (`f(/*name=*/x)`) stays against it.
    pub keep_attached: bool,
    /// A comment after code starts one blank after it, or in the column of the trailing comments
    /// of the lines around it (`comment_column`); the source's blanks do not count.
    pub align_lines: bool,
    /// Aligned comments stay within the line width: a line that would push a comment of the run
    /// past it ends the run (`align_trailing_comments`); without it the width does not matter.
    pub align_within_width: bool,
    /// Blank lines kept after a comment in front of code.
    pub max_blank_after: usize,
    /// The width at which an own-line `//` comment wraps onto more `//` lines; every `//x` comment
    /// but `///x` is written `// x` (google-java-format). `None`: comments keep their text.
    pub reflow_width: Option<usize>,
    /// Tokens after which the width of a comment at the end of the line counts when a group on that
    /// line is measured (a token that cannot break afterwards, like `;`).
    pub trailing_counts_after: &'static [&'static str],
    /// The lines of a block comment lose their trailing blanks (`comment_trim_lines`).
    pub trim_lines: bool,
    /// A comment loses only trailing ASCII whitespace; a no-break space stays
    /// (`comment_keep_unicode_blanks`).
    pub keep_unicode_blanks: bool,
    /// The lines of a block comment keep their indentation relative to the comment's own line
    /// (`comment_reindent_lines`).
    pub reindent: bool,
    /// A one-line block comment that ends a line of the output is written as a line comment
    /// (`single_block_comment_as_line`).
    pub block_as_line: bool,
    /// A block comment written tight against the token after it stays tight
    /// (`block_comment_keep_tight`: `f(/*name=*/x)`).
    pub keep_tight: bool,
    /// Block comments that end with one of these (`=*/`) have no blank before the token after them.
    pub glued: &'static [&'static str],
}

impl CommentStyle {
    /// Is the block comment `text` glued to the token after it?
    pub fn is_glued(&self, text: &str) -> bool {
        self.glued.iter().any(|g| text.ends_with(g))
    }
}

impl Default for CommentStyle {
    fn default() -> Self {
        CommentStyle {
            gap: 1,
            attach_comments: false,
            attach_docs: false,
            keep_gap: false,
            keep_attached: false,
            align_lines: false,
            align_within_width: false,
            max_blank_after: 1,
            reflow_width: None,
            trailing_counts_after: &[],
            trim_lines: false,
            keep_unicode_blanks: false,
            reindent: false,
            block_as_line: false,
            keep_tight: false,
            glued: &[],
        }
    }
}

impl CommentStyle {
    /// The default style, with the trailing comments aligned in columns when the string option
    /// `option` is `align` and keeping the source's blanks before them otherwise.
    pub fn column_option(
        options: &core_config::ResolvedOptions,
        option: core_config::OptId,
    ) -> Self {
        let align = options.is(option, "align");
        CommentStyle {
            keep_gap: !align,
            align_lines: align,
            ..Default::default()
        }
    }

    pub fn gap_doc(&self) -> crate::Doc {
        match self.gap {
            1 => crate::doc::lit(" "),
            n => crate::doc::text(" ".repeat(n)),
        }
    }

    /// The text of the line comment `c` as the style writes it.
    pub fn line_text<'a>(&self, c: &'a Comment) -> Cow<'a, str> {
        let text = c.text_in(self);
        if self.block_as_line && c.kind == CommentKind::Block && !text.contains(['\n', '\r']) {
            return Cow::Owned(format!("{BLOCK_START}{text}{BLOCK_END}"));
        }
        match text.strip_prefix("//") {
            Some(rest)
                if self.reflow_width.is_some()
                    && c.kind == CommentKind::Line
                    && !rest.is_empty()
                    && !rest.starts_with([' ', '/']) =>
            {
                Cow::Owned(format!("// {rest}"))
            }
            _ => text,
        }
    }

    /// Is `c` preceded by a blank (or a line start) in the source?
    fn blank_before(&self, c: &Comment) -> bool {
        match &c.element {
            SyntaxElement::Token(t) => t
                .prev_token()
                .is_none_or(|p| p.text().ends_with([' ', '\t', '\n', '\r'])),
            SyntaxElement::Node(_) => true,
        }
    }

    /// The gap before `c`: `gap`, or the blanks the source has when `keep_gap` and they are more.
    pub fn gap_before(&self, c: &Comment) -> crate::Doc {
        // `if (/*c*/` at a line end: a block comment written against an opening bracket stays there.
        let after_open = c.kind == CommentKind::Block
            && !self.blank_before(c)
            && matches!(&c.element, SyntaxElement::Token(t) if t.prev_token().is_some_and(|p| matches!(p.text(), "(" | "[")));
        if after_open {
            return crate::doc::Doc::Nil;
        }
        if self.align_lines && ends_its_line(c) {
            return match self.gap {
                1 => crate::doc::lit(GAP_MARK),
                n => crate::doc::text(format!("{}\u{1}", " ".repeat(n))),
            };
        }
        let source = match (&c.element, self.keep_gap) {
            (SyntaxElement::Token(t), true) => t
                .prev_token()
                .filter(|p| p.text().bytes().all(|b| b == b' ' || b == b'\t'))
                .map_or(0, |p| p.text().len()),
            _ => 0,
        };
        if (self.keep_gap || self.keep_attached)
            && c.kind == CommentKind::Block
            && !self.blank_before(c)
        {
            // `f(/*name=*/x)`: a block comment written against the code stays against it.
            crate::doc::Doc::Nil
        } else if source > self.gap {
            crate::doc::text(" ".repeat(source))
        } else {
            self.gap_doc()
        }
    }
}

/// Does a line break (or the end of the file) follow `c`? A comment with code after it on its line
/// (`f(a, /* b */ c)`) is not a trailing comment: it stays in its line's text.
fn ends_its_line(c: &Comment) -> bool {
    let SyntaxElement::Token(t) = &c.element else {
        return true;
    };
    if t.text().contains(['\n', '\r']) {
        return false;
    }
    let mut next = t.next_token();
    while let Some(n) = next {
        let text = n.text();
        if text.contains(['\n', '\r']) {
            return true;
        }
        if !text.bytes().all(|b| b == b' ' || b == b'\t') {
            return false;
        }
        next = n.next_token();
    }
    true
}

/// Around a one-line block comment in the printed text under `block_as_line`: [`settle_block_comments`]
/// writes the ones that end their line as line comments and drops the marks.
const BLOCK_START: char = '\u{2}';
const BLOCK_END: char = '\u{3}';

/// Settles the one-line block comments (`block_as_line`) of the printed `out`: one that ends its
/// line becomes the line comment `line_form` makes of it, the others lose the marks. `keep` (see
/// [`align_marked_gaps`]) follows the changes.
pub fn settle_block_comments(g: &Grammar, out: &mut String, keep: &mut usize) {
    if !out.contains(BLOCK_START) {
        return;
    }
    let mut settled = String::with_capacity(out.len());
    let mut kept = *keep;
    let mut rest = out.as_str();
    let mut at = 0;
    while let Some(start) = rest.find(BLOCK_START) {
        let Some(len) = rest[start..].find(BLOCK_END) else {
            break;
        };
        let text = &rest[start + BLOCK_START.len_utf8()..start + len];
        let after = rest[start + len + BLOCK_END.len_utf8()..].trim_start_matches([' ', '\t']);
        let ends = after.is_empty() || after.starts_with(['\n', '\r']);
        let line = if ends {
            crate::safety::line_form(g, text)
        } else {
            None
        };
        settled.push_str(&rest[..start]);
        // (a line comment that was glued to the token before it, `(/* a */`, gets its blank)
        let blank = line.is_some() && !settled.is_empty() && !settled.ends_with([' ', '\t', '\n']);
        let written = match &line {
            Some(l) if blank => format!(" {l}"),
            Some(l) => l.clone(),
            None => text.to_string(),
        };
        settled.push_str(&written);
        let consumed = start + len + BLOCK_END.len_utf8();
        if at + start < *keep {
            kept = (kept + written.len()).saturating_sub(consumed - start);
        }
        at += consumed;
        rest = &rest[consumed..];
    }
    settled.push_str(rest);
    *out = settled;
    *keep = kept;
}

/// Written in place of the blank before a trailing comment under `align_lines`: a blank and a
/// zero-width mark, which [`align_marked_gaps`] replaces by the blanks that align the comments.
pub const GAP_MARK: &str = " \u{1}";

/// Pads the [`GAP_MARK`]s of `out` so that the trailing comments of consecutive lines start in one
/// column, one past the widest text before a comment of the run (characters, a tab counts one); a
/// line without a trailing comment ends the run, and so does a line that would push a comment of
/// the run past `limit` columns, or a change between a preprocessor line (starting with `#`) and a
/// code line. `keep` (the output length up to which trailing
/// blanks are significant) follows the padding.
pub fn align_marked_gaps(out: &mut String, keep: &mut usize, limit: usize) {
    const MARK: char = '\u{1}';
    if !out.contains(MARK) && !out.contains(CONTINUATION) {
        return;
    }
    // (offset of the mark, line number, characters before the mark, characters after it,
    // whether the line is a preprocessor line)
    let mut cells: Vec<(usize, usize, usize, usize, bool)> = Vec::new();
    let (mut line, mut seen) = (0, 0);
    for (at, _) in out.match_indices(MARK) {
        line += out[seen..at].matches('\n').count();
        seen = at;
        if cells.last().is_some_and(|c| c.1 == line) {
            continue;
        }
        let start = out[..at].rfind('\n').map_or(0, |i| i + 1);
        let end = out[at..].find('\n').map_or(out.len(), |i| at + i);
        let directive = out[start..].trim_start().starts_with('#');
        cells.push((
            at,
            line,
            out[start..at].chars().count(),
            out[at + MARK.len_utf8()..end].chars().count(),
            directive,
        ));
    }
    let mut pads = vec![0usize; cells.len()];
    let mut i = 0;
    while i < cells.len() {
        let mut j = i + 1;
        let mut widest = cells[i].2;
        while j < cells.len() && cells[j].1 == cells[j - 1].1 + 1 && cells[j].4 == cells[i].4 {
            let wider = widest.max(cells[j].2);
            if cells[i..=j].iter().any(|c| wider + c.3 > limit) {
                break;
            }
            widest = wider;
            j += 1;
        }
        let widest = cells[i..j].iter().map(|c| c.2).max().unwrap_or(0);
        for k in i..j {
            pads[k] = widest - cells[k].2;
        }
        i = j;
    }
    // A mark that no cell took (a second one on a line) is dropped.
    let mut result = String::with_capacity(out.len());
    let (mut last, mut cell, mut delta) = (0, 0, 0isize);
    for (at, _) in out.match_indices(MARK) {
        result.push_str(&out[last..at]);
        last = at + MARK.len_utf8();
        let pad = match cells.get(cell) {
            Some(c) if c.0 == at => {
                cell += 1;
                pads[cell - 1]
            }
            _ => 0,
        };
        result.push_str(&" ".repeat(pad));
        if at < *keep {
            delta += pad as isize - MARK.len_utf8() as isize;
        }
    }
    result.push_str(&out[last..]);
    // A continuation line starts under the comment of the line above it.
    if result.contains(CONTINUATION) {
        let column_of: std::collections::HashMap<usize, usize> = cells
            .iter()
            .zip(&pads)
            .map(|(c, pad)| (c.1, c.2 + pad))
            .collect();
        let mut lines: Vec<String> = result.split('\n').map(str::to_string).collect();
        let (mut at, mut above) = (0, None);
        for k in 0..lines.len() {
            let column = column_of.get(&k).copied();
            if lines[k].starts_with(CONTINUATION) {
                let col = above.or_else(|| lines[k - 1].find("//")).unwrap_or(0);
                let pad = " ".repeat(col);
                if at < *keep {
                    delta += col as isize - CONTINUATION.len_utf8() as isize;
                }
                lines[k].replace_range(..CONTINUATION.len_utf8(), &pad);
                above = Some(col);
            } else {
                above = column;
            }
            at += lines[k].len() + 1;
        }
        result = lines.join("\n");
    }
    *keep = keep.saturating_add_signed(delta);
    *out = result;
}

/// Written at the start of a line that continues the trailing comment above it: replaced by the
/// blanks that put it in the column of that comment.
pub const CONTINUATION: char = '\u{2}';

/// Number of line breaks in `text` (`\r\n` counts once).
pub fn count_breaks(text: &str) -> usize {
    let mut n = 0;
    let mut prev_cr = false;
    for c in text.chars() {
        if is_line_break(c) && !(c == '\n' && prev_cr) {
            n += 1;
        }
        prev_cr = c == '\r';
    }
    n
}

fn element_text(e: &SyntaxElement) -> Cow<'_, str> {
    match e {
        SyntaxElement::Token(t) => Cow::Borrowed(t.text()),
        SyntaxElement::Node(n) => Cow::Owned(n.text().to_string()),
    }
}

fn ends_line(e: &SyntaxElement) -> bool {
    match e {
        SyntaxElement::Token(t) => t.text().chars().next_back().is_some_and(is_line_break),
        // A structured trivia ends with a zero-width token; its text decides.
        SyntaxElement::Node(n) => n
            .text()
            .to_string()
            .chars()
            .next_back()
            .is_some_and(is_line_break),
    }
}

fn is_trivia(g: &Grammar, k: SyntaxKind) -> bool {
    g.is_trivia(k)
}

/// Is this kind a comment, or text that is kept as written like one?
fn is_comment_like(g: &Grammar, k: SyntaxKind) -> bool {
    g.has_tag(k, "comment") || g.has_tag(k, "verbatim-trivia")
}

/// The outermost structured trivia node a token is inside of, if any.
fn trivia_ancestor(g: &Grammar, t: &SyntaxToken) -> Option<SyntaxNode> {
    t.parent_ancestors()
        .filter(|a| is_trivia(g, a.kind()))
        .last()
}

/// A token as one element of a trivia run: the structured trivia it is in, else itself.
fn element_of(g: &Grammar, t: SyntaxToken) -> SyntaxElement {
    match trivia_ancestor(g, &t) {
        Some(n) => SyntaxElement::Node(n),
        None => SyntaxElement::Token(t),
    }
}

fn before_element(g: &Grammar, e: &SyntaxElement) -> Option<SyntaxElement> {
    let first = match e {
        SyntaxElement::Token(t) => t.clone(),
        SyntaxElement::Node(n) => n.first_token()?,
    };
    prev_token_over_empty(first).map(|t| element_of(g, t))
}

/// `prev_token`, which gives up at an empty node (the parser leaves zero-width ones).
fn prev_token_over_empty(t: SyntaxToken) -> Option<SyntaxToken> {
    let mut e = SyntaxElement::Token(t);
    loop {
        match e.prev_sibling_or_token() {
            Some(s) => match &s {
                SyntaxElement::Token(t) => return Some(t.clone()),
                SyntaxElement::Node(n) => match n.last_token() {
                    Some(t) => return Some(t),
                    None => e = s,
                },
            },
            None => e = SyntaxElement::Node(e.parent()?),
        }
    }
}

fn after_element(g: &Grammar, e: &SyntaxElement) -> Option<SyntaxElement> {
    let last = match e {
        SyntaxElement::Token(t) => t.clone(),
        SyntaxElement::Node(n) => n.last_token()?,
    };
    next_token_over_empty(last).map(|t| element_of(g, t))
}

fn next_token_over_empty(t: SyntaxToken) -> Option<SyntaxToken> {
    let mut e = SyntaxElement::Token(t);
    loop {
        match e.next_sibling_or_token() {
            Some(s) => match &s {
                SyntaxElement::Token(t) => return Some(t.clone()),
                SyntaxElement::Node(n) => match n.first_token() {
                    Some(t) => return Some(t),
                    None => e = s,
                },
            },
            None => e = SyntaxElement::Node(e.parent()?),
        }
    }
}

fn is_trivia_element(g: &Grammar, e: &SyntaxElement) -> bool {
    is_trivia(g, e.kind())
}

/// The trivia run directly before `tok` and the non-trivia element that precedes it.
fn run_before(g: &Grammar, tok: &SyntaxToken) -> (Vec<SyntaxElement>, Option<SyntaxElement>) {
    let mut run = Vec::new();
    let mut cur = before_element(g, &SyntaxElement::Token(tok.clone()));
    while let Some(e) = cur {
        if !is_trivia_element(g, &e) {
            run.reverse();
            return (run, Some(e));
        }
        cur = before_element(g, &e);
        run.push(e);
    }
    run.reverse();
    (run, None)
}

fn run_after(g: &Grammar, tok: &SyntaxToken) -> Vec<SyntaxElement> {
    let mut run = Vec::new();
    let mut cur = after_element(g, &SyntaxElement::Token(tok.clone()));
    while let Some(e) = cur {
        if !is_trivia_element(g, &e) {
            break;
        }
        cur = after_element(g, &e);
        run.push(e);
    }
    run
}

fn trailing_cut(run: &[SyntaxElement]) -> usize {
    run.iter().position(ends_line).map_or(run.len(), |i| i + 1)
}

/// Does the run between `prev` and the next token belong to that next token as a whole? So it
/// does when `prev` is tagged `#glue` (`(`, `,`: a comment after it leads what follows) and the
/// run starts with a block comment on the line of `prev`.
fn glued(g: &Grammar, prev: &SyntaxToken, run: &[SyntaxElement]) -> bool {
    if !g.has_tag(prev.kind(), "glue") {
        return false;
    }
    for e in run {
        match e {
            SyntaxElement::Token(t) if g.has_tag(t.kind(), "comment") => {
                return t.text().starts_with("/*");
            }
            SyntaxElement::Token(t) if count_breaks(t.text()) > 0 => return false,
            _ => {}
        }
    }
    false
}

/// Trivia owned by `tok` that follows it on its line.
pub fn trailing_trivia(g: &Grammar, tok: &SyntaxToken) -> Vec<SyntaxElement> {
    let mut run = run_after(g, tok);
    if glued(g, tok, &run) {
        return Vec::new();
    }
    run.truncate(trailing_cut(&run));
    run
}

/// Trivia owned by `tok` that precedes it (own-line comments, indentation, blank lines).
pub fn leading_trivia(g: &Grammar, tok: &SyntaxToken) -> Vec<SyntaxElement> {
    let (mut run, prev) = run_before(g, tok);
    let cut = match prev {
        Some(SyntaxElement::Token(prev)) if glued(g, &prev, &run) => 0,
        Some(_) => trailing_cut(&run),
        None => 0,
    };
    run.drain(..cut);
    run
}

/// A block comment of several lines that each start with `*` (after blanks) is re-indented as a
/// whole: tag `#indentable` (`#indentable-doc`: only a `/**` one).
fn indentable(text: &str) -> bool {
    let mut lines = text.lines();
    lines.next();
    let mut any = false;
    for l in lines {
        if !l.trim_start().starts_with('*') {
            return false;
        }
        any = true;
    }
    any
}

fn kind_of(g: &Grammar, e: &SyntaxElement) -> Option<CommentKind> {
    let kind = e.kind();
    if g.has_tag(kind, "verbatim-trivia") {
        return Some(if g.has_tag(kind, "indented") {
            CommentKind::Lines
        } else {
            CommentKind::Directive
        });
    }
    if !g.has_tag(kind, "comment") {
        return None;
    }
    let text = element_text(e);
    if (g.has_tag(kind, "indentable")
        || (g.has_tag(kind, "indentable-doc") && text.starts_with("/**")))
        && indentable(&text)
    {
        return Some(CommentKind::Lines);
    }
    let is_line = g
        .comments
        .line
        .as_deref()
        .is_some_and(|l| text.starts_with(l));
    Some(match e {
        SyntaxElement::Node(_) if text.starts_with("///") => CommentKind::Lines,
        _ if is_line => CommentKind::Line,
        _ => CommentKind::Block,
    })
}

/// What the trivia run before a token holds, found without collecting it. Most tokens have no
/// comment, and then every question about their trivia is answered by this one backward walk.
struct ScanBefore {
    /// Does a non-trivia token precede the run?
    has_prev: bool,
    /// Is there a comment in the run? Then the other fields are not computed.
    has_comment: bool,
    /// Line breaks in the whole run.
    breaks: usize,
    /// Line breaks in the tokens after the first one that ends a line.
    after_first_line_end: Option<usize>,
}

impl ScanBefore {
    /// Line breaks in the part of the run the token owns (see [`leading_trivia`]).
    fn leading_breaks(&self) -> usize {
        if self.has_prev {
            self.after_first_line_end.unwrap_or(0)
        } else {
            self.breaks
        }
    }
}

/// Calls `visit` with each token before the child at `idx` of `parent` (`None`: nothing
/// before), nearest first, until it returns `Break`. Walks the green tree: the neighbours of a
/// token are mostly trivia, and creating a red token for each of them would cost far more than
/// looking at them.
fn walk_back<B>(
    g: &Grammar,
    parent: Option<SyntaxNode>,
    mut idx: usize,
    mut visit: impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
) -> Option<B> {
    fn down<B>(
        g: &Grammar,
        node: &rowan::GreenNodeData,
        visit: &mut impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for child in node.children().rev() {
            match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text())?,
                rowan::NodeOrToken::Node(n) => visit_node(g, n, visit, down)?,
            }
        }
        ControlFlow::Continue(())
    }
    let mut node = parent?;
    loop {
        for child in node.green().children().take(idx).rev() {
            let flow = match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text()),
                rowan::NodeOrToken::Node(n) => visit_node(g, n, &mut visit, down),
            };
            if let ControlFlow::Break(b) = flow {
                return Some(b);
            }
        }
        idx = node.index();
        node = node.parent()?;
    }
}

/// Visits a green node: a structured trivia is one element (its kind and whole text), any other
/// node is entered with `down`.
fn visit_node<B, V: FnMut(SyntaxKind, &str) -> ControlFlow<B>>(
    g: &Grammar,
    node: &rowan::GreenNodeData,
    visit: &mut V,
    down: fn(&Grammar, &rowan::GreenNodeData, &mut V) -> ControlFlow<B>,
) -> ControlFlow<B> {
    let kind = SyntaxKind(node.kind().0);
    if is_trivia(g, kind) {
        visit(kind, &node.to_string())
    } else {
        down(g, node, visit)
    }
}

/// Like [`walk_back`], for the tokens after `tok`.
fn walk_forward<B>(
    g: &Grammar,
    tok: &SyntaxToken,
    mut visit: impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
) -> Option<B> {
    fn down<B>(
        g: &Grammar,
        node: &rowan::GreenNodeData,
        visit: &mut impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for child in node.children() {
            match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text())?,
                rowan::NodeOrToken::Node(n) => visit_node(g, n, visit, down)?,
            }
        }
        ControlFlow::Continue(())
    }
    let mut node = tok.parent()?;
    let mut idx = tok.index() + 1;
    loop {
        for child in node.green().children().skip(idx) {
            let flow = match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text()),
                rowan::NodeOrToken::Node(n) => visit_node(g, n, &mut visit, down),
            };
            if let ControlFlow::Break(b) = flow {
                return Some(b);
            }
        }
        idx = node.index() + 1;
        node = node.parent()?;
    }
}

fn text_ends_line(text: &str) -> bool {
    text.chars().next_back().is_some_and(is_line_break)
}

fn scan_back(g: &Grammar, parent: Option<SyntaxNode>, idx: usize) -> ScanBefore {
    let mut breaks = 0;
    let mut after_first_line_end = None;
    let stopped = walk_back(g, parent, idx, |kind, text| {
        if !is_trivia(g, kind) {
            return ControlFlow::Break(ScanBefore {
                has_prev: true,
                has_comment: false,
                breaks,
                after_first_line_end,
            });
        }
        if is_comment_like(g, kind) {
            return ControlFlow::Break(ScanBefore {
                has_prev: false,
                has_comment: true,
                breaks: 0,
                after_first_line_end: None,
            });
        }
        if text_ends_line(text) {
            after_first_line_end = Some(breaks);
        }
        breaks += count_breaks(text);
        ControlFlow::Continue(())
    });
    stopped.unwrap_or(ScanBefore {
        has_prev: false,
        has_comment: false,
        breaks,
        after_first_line_end,
    })
}

fn scan_before(g: &Grammar, tok: &SyntaxToken) -> ScanBefore {
    scan_back(g, tok.parent(), tok.index())
}

/// What precedes the first non-trivia token of a node: see [`node_leading`].
pub struct NodeLeading {
    /// Does the node hold a non-trivia token at all?
    pub has_tokens: bool,
    /// Is there a comment among the leading trivia of its first token? When there is, the
    /// other fields are not computed and the caller has to look at the token itself.
    pub has_comment: bool,
    /// [`blank_lines_before`] for the node's first token.
    pub blank_lines: usize,
}

/// Answers what [`blank_lines_before`] and [`leading_comments`] would for the first non-trivia
/// token of `node`, without creating that token or any of the trivia around it.
pub fn node_leading(g: &Grammar, node: &SyntaxNode) -> NodeLeading {
    // The trivia inside the node, in front of its first token.
    let (mut total, mut after_first, mut seen_line_end) = (0, 0, false);
    let mut comment = false;
    let found = walk_node(g, node, |kind, text| {
        if !is_trivia(g, kind) {
            return ControlFlow::Break(true);
        }
        if is_comment_like(g, kind) {
            comment = true;
            return ControlFlow::Break(true);
        }
        let n = count_breaks(text);
        total += n;
        if seen_line_end {
            after_first += n;
        } else if text_ends_line(text) {
            seen_line_end = true;
        }
        ControlFlow::Continue(())
    })
    .unwrap_or(false);
    if !found {
        return NodeLeading {
            has_tokens: false,
            has_comment: false,
            blank_lines: 0,
        };
    }
    // The trivia before the node, joined to the above.
    let outer = scan_back(g, node.parent(), node.index());
    if comment || outer.has_comment {
        return NodeLeading {
            has_tokens: true,
            has_comment: true,
            blank_lines: 0,
        };
    }
    let blank_lines = match (outer.has_prev, outer.after_first_line_end, seen_line_end) {
        (false, ..) => 0,
        (true, Some(after), _) => after + total,
        (true, None, true) => after_first,
        (true, None, false) => 0,
    };
    NodeLeading {
        has_tokens: true,
        has_comment: false,
        blank_lines,
    }
}

/// Is there a comment among the trivia at the end of `node`, after its last significant token?
/// Walks the green tree backwards without creating tokens, so it can ask this of every node.
pub fn comment_at_end(g: &Grammar, node: &SyntaxNode) -> bool {
    fn down(g: &Grammar, node: &rowan::GreenNodeData) -> ControlFlow<bool> {
        for child in node.children().rev() {
            let (kind, flow) = match child {
                rowan::NodeOrToken::Token(t) => (SyntaxKind(t.kind().0), None),
                rowan::NodeOrToken::Node(n) if !is_trivia(g, SyntaxKind(n.kind().0)) => {
                    (SyntaxKind(n.kind().0), Some(down(g, n)))
                }
                rowan::NodeOrToken::Node(n) => (SyntaxKind(n.kind().0), None),
            };
            if let Some(flow) = flow {
                flow?;
            } else if !is_trivia(g, kind) {
                return ControlFlow::Break(false);
            } else if is_comment_like(g, kind) {
                return ControlFlow::Break(true);
            }
        }
        ControlFlow::Continue(())
    }
    matches!(down(g, &node.green()), ControlFlow::Break(true))
}

/// Is the last token of `node` that is not trivia of kind `kind`, one of its own children?
/// Reads the green tree, so it can be asked of every node.
pub fn ends_with_token(g: &Grammar, node: &SyntaxNode, kind: SyntaxKind) -> bool {
    for child in node.green().children().rev() {
        match child {
            rowan::NodeOrToken::Token(t) => {
                let k = SyntaxKind(t.kind().0);
                if k == kind {
                    return true;
                }
                if !is_trivia(g, k) {
                    return false;
                }
            }
            rowan::NodeOrToken::Node(n) => {
                if !is_trivia(g, SyntaxKind(n.kind().0)) {
                    return false;
                }
            }
        }
    }
    false
}

/// The spans (start of the trivia in front of a token, start of the token) of the tokens that
/// have a comment among the trivia in front of them, in order, found in one walk of the green
/// tree of `root` (no tokens are created), so that asking this of every node costs a search.
pub fn spans_after_comments(g: &Grammar, root: &SyntaxNode) -> Vec<(u32, u32)> {
    struct Walk {
        at: u32,
        /// Where the trivia in front of the next token start.
        run: u32,
        pending: bool,
        out: Vec<(u32, u32)>,
    }
    fn down(g: &Grammar, node: &rowan::GreenNodeData, w: &mut Walk) {
        for child in node.children() {
            match child {
                rowan::NodeOrToken::Node(n) => down(g, n, w),
                rowan::NodeOrToken::Token(t) => {
                    let kind = SyntaxKind(t.kind().0);
                    if !is_trivia(g, kind) {
                        if std::mem::take(&mut w.pending) {
                            w.out.push((w.run, w.at));
                        }
                        w.run = w.at + u32::from(t.text_len());
                    } else if is_comment_like(g, kind) {
                        w.pending = true;
                    }
                    w.at += u32::from(t.text_len());
                }
            }
        }
    }
    let mut w = Walk {
        at: 0,
        run: 0,
        pending: false,
        out: Vec::new(),
    };
    down(g, &root.green(), &mut w);
    w.out
}

/// Calls `visit` with the tokens of `node` in document order until it returns `Break`.
fn walk_node<B>(
    g: &Grammar,
    node: &SyntaxNode,
    mut visit: impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
) -> Option<B> {
    fn down<B>(
        g: &Grammar,
        node: &rowan::GreenNodeData,
        visit: &mut impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for child in node.children() {
            match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text())?,
                rowan::NodeOrToken::Node(n) => visit_node(g, n, visit, down)?,
            }
        }
        ControlFlow::Continue(())
    }
    match down(g, &node.green(), &mut visit) {
        ControlFlow::Break(b) => Some(b),
        ControlFlow::Continue(()) => None,
    }
}

/// Is there a comment in the trivia directly after `tok`?
fn comment_follows(g: &Grammar, tok: &SyntaxToken) -> bool {
    walk_forward(g, tok, |kind, _| {
        if !is_trivia(g, kind) {
            ControlFlow::Break(false)
        } else if is_comment_like(g, kind) {
            ControlFlow::Break(true)
        } else {
            ControlFlow::Continue(())
        }
    })
    .unwrap_or(false)
}

pub fn leading_comments(g: &Grammar, tok: &SyntaxToken) -> Vec<Comment> {
    if !scan_before(g, tok).has_comment {
        return Vec::new();
    }
    let trivia = leading_trivia(g, tok);
    let mut out: Vec<Comment> = Vec::new();
    for t in trivia {
        match kind_of(g, &t) {
            Some(kind) => out.push(Comment {
                // A structured trivia ends its own line.
                breaks_after: usize::from(ends_line(&t) && kind != CommentKind::Line),
                element: t,
                kind,
            }),
            None => {
                if let Some(last) = out.last_mut() {
                    last.breaks_after += count_breaks(&element_text(&t));
                }
            }
        }
    }
    out
}

pub fn has_trailing_comment(g: &Grammar, tok: &SyntaxToken) -> bool {
    comment_follows(g, tok)
}

/// A comment in front of `tok` or after it.
pub fn token_has_comment(g: &Grammar, tok: &SyntaxToken) -> bool {
    !leading_comments(g, tok).is_empty() || has_trailing_comment(g, tok)
}

pub fn trailing_comments(g: &Grammar, tok: &SyntaxToken) -> Vec<Comment> {
    if !comment_follows(g, tok) {
        return Vec::new();
    }
    trailing_trivia(g, tok)
        .into_iter()
        .filter_map(|t| {
            kind_of(g, &t).map(|kind| Comment {
                element: t,
                kind,
                breaks_after: 0,
            })
        })
        .collect()
}

/// Blank lines between the previous line and `tok` (or its first leading comment), 0 at the
/// start of a file.
pub fn blank_lines_before(g: &Grammar, tok: &SyntaxToken) -> usize {
    let scan = scan_before(g, tok);
    if !scan.has_comment {
        return if scan.has_prev {
            scan.leading_breaks()
        } else {
            0
        };
    }
    let has_prev = run_before(g, tok).1.is_some();
    if !has_prev {
        return 0;
    }
    leading_trivia(g, tok)
        .iter()
        .take_while(|t| kind_of(g, t).is_none())
        .map(|t| count_breaks(&element_text(t)))
        .sum()
}

/// The column of a token in its line, in characters.
pub fn column(tok: &SyntaxToken) -> usize {
    let mut col = 0;
    let mut prev = tok.prev_token();
    while let Some(t) = prev {
        let text = t.text();
        if let Some(i) = text.rfind(['\n', '\r']) {
            return col + text[i + 1..].chars().count();
        }
        col += text.chars().count();
        prev = t.prev_token();
    }
    col
}

/// The first token after `tok` that is not trivia.
pub fn next_significant(g: &Grammar, tok: &SyntaxToken) -> Option<SyntaxToken> {
    std::iter::successors(tok.next_token(), SyntaxToken::next_token)
        .find(|t| !g.is_trivia(t.kind()))
}

/// How many of the leading comments `cs` stand at `min_col` or deeper before the first that
/// does not: those belong to the block that ends before the token, the others to the token.
pub fn indented_run(cs: &[Comment], min_col: usize) -> usize {
    cs.iter()
        .take_while(|c| column(c.token()) >= min_col)
        .count()
}

/// The significant token before `tok`.
pub fn prev_significant(g: &Grammar, tok: &SyntaxToken) -> Option<SyntaxToken> {
    std::iter::successors(tok.prev_token(), SyntaxToken::prev_token)
        .find(|t| !g.is_trivia(t.kind()))
}

/// The first significant token of a node.
fn first_significant(g: &Grammar, node: &SyntaxNode) -> Option<SyntaxToken> {
    std::iter::successors(node.first_token(), SyntaxToken::next_token)
        .find(|t| !g.is_trivia(t.kind()))
}

/// The last significant token of a node.
fn last_significant(g: &Grammar, node: &SyntaxNode) -> Option<SyntaxToken> {
    std::iter::successors(node.last_token(), SyntaxToken::prev_token)
        .find(|t| !g.is_trivia(t.kind()))
}

/// The block collections (nodes of the kinds `blocks`) that end right before `tok`, innermost
/// first, with the range of the leading comments `cs` of `tok` each one keeps; a collection
/// keeps the comments indented at least as far as itself, so a comment after a nested block
/// stays with it. With the option `comment_indent: content` only the innermost collection
/// keeps comments, those deeper than `tok` (at the end of a file, deeper than column 0): each
/// is then indented like the line before it or like `tok`.
pub fn block_takers(
    g: &Grammar,
    options: &core_config::ResolvedOptions,
    cs: &[Comment],
    tok: &SyntaxToken,
    blocks: &[SyntaxKind],
) -> Vec<(SyntaxNode, usize, usize)> {
    let Some(prev) = prev_significant(g, tok) else {
        return Vec::new();
    };
    let mut content = None;
    let (mut out, mut start) = (Vec::new(), 0);
    for a in prev.parent_ancestors() {
        if last_significant(g, &a).as_ref() != Some(&prev) {
            break;
        }
        if blocks.contains(&a.kind()) {
            let content = *content
                .get_or_insert_with(|| options.is(core_config::opt::COMMENT_INDENT, "content"));
            let col = if content {
                1 + if tok.text_range().is_empty() {
                    0
                } else {
                    column(tok)
                }
            } else {
                first_significant(g, &a).as_ref().map_or(0, column)
            };
            let take = indented_run(&cs[start..], col);
            out.push((a, start, start + take));
            start += take;
            if content {
                break;
            }
        }
    }
    out
}

/// Does no non-trivia token come before `tok`?
pub fn starts_file(g: &Grammar, tok: &SyntaxToken) -> bool {
    run_before(g, tok).1.is_none()
}

/// Is there a line break in the trivia between `tok` and the next non-trivia token?
pub fn break_after(g: &Grammar, tok: &SyntaxToken) -> bool {
    walk_forward(g, tok, |kind, text| {
        if !is_trivia(g, kind) {
            ControlFlow::Break(false)
        } else if count_breaks(text) > 0 {
            ControlFlow::Break(true)
        } else {
            ControlFlow::Continue(())
        }
    })
    .unwrap_or(false)
}

/// The value of the parser directive `<marker> name=value` that stands among the comments before
/// the first token of `root`'s text (names compare ignoring case, blanks around the value go).
pub fn parser_directive(g: &Grammar, root: &SyntaxNode, name: &str) -> Option<String> {
    let marker = g.comments.line.as_deref()?;
    let first = root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !g.is_trivia(t.kind()))?;
    leading_comments(g, &first).iter().find_map(|c| {
        let text = c.text();
        let body = text.trim_start_matches(marker);
        let (key, value) = body.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.replace(' ', ""))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_syntax::{SyntaxNode, TreeBuilder, grammars};

    /// Tokens of `src` as a flat JSON-grammar tree: lexed by splitting on a tiny alphabet.
    fn tree(parts: &[(&str, &str)]) -> SyntaxNode {
        let g = grammars::json();
        let mut b = TreeBuilder::new();
        b.start_node(g.kind_by_name("Document").unwrap());
        for (kind, text) in parts {
            b.token(g.kind_by_name(kind).unwrap(), text);
        }
        b.token(g.kind_by_name("Eof").unwrap(), "");
        b.finish_node();
        SyntaxNode::new_root(b.finish())
    }

    fn token(root: &SyntaxNode, text: &str) -> SyntaxToken {
        root.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.text() == text)
            .unwrap()
    }

    #[test]
    fn splits_trivia_between_trailing_and_leading() {
        let g = grammars::json();
        let root = tree(&[
            ("Number", "1"),
            ("Whitespace", " "),
            ("LineComment", "// a"),
            ("Newline", "\n"),
            ("Newline", "\n"),
            ("LineComment", "// b"),
            ("Newline", "\n"),
            ("Number", "2"),
        ]);
        let one = token(&root, "1");
        let two = token(&root, "2");
        let trailing: Vec<_> = trailing_comments(g, &one)
            .iter()
            .map(|c| c.text().to_string())
            .collect();
        assert_eq!(trailing, ["// a"]);
        let leading = leading_comments(g, &two);
        assert_eq!(leading.len(), 1);
        assert_eq!(leading[0].text(), "// b");
        assert_eq!(leading[0].breaks_after, 1);
        assert_eq!(blank_lines_before(g, &two), 1);
        assert_eq!(blank_lines_before(g, &one), 0);
    }

    #[test]
    fn same_line_block_comment_is_trailing() {
        let g = grammars::json();
        let root = tree(&[
            ("Number", "1"),
            ("BlockComment", "/* a */"),
            ("Number", "2"),
        ]);
        assert_eq!(trailing_comments(g, &token(&root, "1")).len(), 1);
        assert!(leading_comments(g, &token(&root, "2")).is_empty());
        assert!(!break_after(g, &token(&root, "1")));
    }

    #[test]
    fn counts_crlf_once() {
        assert_eq!(count_breaks("\r\n"), 1);
        assert_eq!(count_breaks("\n\r\n\r"), 3);
    }
}
