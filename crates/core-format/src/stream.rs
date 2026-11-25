//! The token stream of a line-preserving formatter: the tokens of a file with what the source has
//! around each (line breaks, blanks, a line continuation, the comments in front), the matching
//! brackets, and the loop that writes them: every output line starts at a token the language
//! marked as starting one, the blank lines in front of it stay (up to `max_blank_lines`), the
//! blanks between tokens of a line come from the language, and the indentation is that of the
//! open frames or, with the fact `source_indent`, the source's own. The `[lines]` section of
//! `format.rules` holds `open` and `close` (see [`crate::frames`]) and `continuation` (the character
//! that ends a continued line, written in the blanks before the line break). `pipe`: the token
//! kinds after which the next line continues, given by the language.
//!
//! concept: token-line-stream
//! what: The tokens of a file with their line breaks, blanks and comments, written line by line with frame or source indentation.
//! tools: PSScriptAnalyzer Invoke-Formatter, RuboCop Layout
//! reads: rules:lines

use crate::comments::{self, Comment};
use crate::frames::{Brackets, Frames};
use crate::{Doc, FormatCtx, doc};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken};

/// A token with what the source has around it.
pub struct Tk {
    pub tok: SyntaxToken,
    pub kind: SyntaxKind,
    /// Source lines of the token's first and last character.
    pub first_line: usize,
    pub last_line: usize,
    /// Line breaks between the previous token and this one.
    pub breaks: usize,
    /// A continuation character and a line break come before the token.
    pub cont: bool,
    /// The blanks before the token on its line (a comment between counts as one blank).
    pub gap: String,
    /// The blanks in front of the continuation character.
    pub cont_gap: String,
    /// The token starts an output line.
    pub newline: bool,
    /// The comments on lines of their own in front of the token.
    pub leading: Vec<Comment>,
    /// A comment follows the token on its line.
    pub trailing: bool,
    /// The matching bracket of an opener or closer.
    pub pair: Option<usize>,
}

impl Tk {
    pub fn text(&self) -> &str {
        self.tok.text()
    }

    pub fn is(&self, kinds: &[SyntaxKind]) -> bool {
        kinds.contains(&self.kind)
    }
}

/// The `[lines]` settings of a language.
#[derive(Default)]
pub struct StreamRules {
    pub brackets: Brackets,
    continuation: Option<char>,
    /// Each line keeps the indentation it has, and the language is written by its rows alone
    /// ([`format`]): the fact `source_indent` of the language's rules.
    pub(crate) source_indent: bool,
    pub(crate) blanks: crate::stream_blank::Blanks,
    pub(crate) indents: crate::stream_indent::Indents,
}

impl StreamRules {
    pub(crate) fn source_indent(mut self, on: bool) -> StreamRules {
        self.source_indent = on;
        self
    }

    pub fn parse(g: &Grammar, text: &str) -> StreamRules {
        let settings = crate::rules::settings(text, "lines");
        let value = |key: &str| settings.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
        StreamRules {
            brackets: Brackets::parse(g, text),
            continuation: value("continuation").and_then(|v| v.chars().next()),
            source_indent: false,
            blanks: Default::default(),
            indents: Default::default(),
        }
    }
}

/// The tokens of `root` (not the trivia) in order.
pub fn collect(g: &Grammar, root: &SyntaxNode, rules: &StreamRules) -> Vec<Tk> {
    let mut line = 0;
    let mut out: Vec<Tk> = Vec::new();
    for t in root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let n = comments::count_breaks(t.text());
        if !g.is_trivia(t.kind()) {
            let mut tk = Tk {
                kind: t.kind(),
                first_line: line,
                last_line: line + n,
                breaks: 0,
                cont: false,
                gap: String::new(),
                cont_gap: String::new(),
                newline: false,
                leading: comments::leading_comments(g, &t),
                trailing: comments::has_trailing_comment(g, &t),
                pair: None,
                tok: t.clone(),
            };
            gap(g, &t, &mut tk, rules.continuation);
            out.push(tk);
        }
        line += n;
    }
    out
}

/// Fills in what the trivia between the previous token and `t` say.
fn gap(g: &Grammar, t: &SyntaxToken, tk: &mut Tk, continuation: Option<char>) {
    let mut trivia = Vec::new();
    let mut cur = t.prev_token();
    while let Some(p) = cur.filter(|p| g.is_trivia(p.kind())) {
        cur = p.prev_token();
        trivia.push(p);
    }
    let (mut commented, mut blanks) = (false, false);
    for p in trivia.iter().rev() {
        let kind = p.kind();
        if g.has_tag(kind, "newline") {
            tk.breaks += comments::count_breaks(p.text());
            tk.gap.clear();
        } else if g.has_tag(kind, "whitespace") {
            if continuation.is_some_and(|c| p.text().contains(c)) {
                tk.cont = true;
                tk.cont_gap = std::mem::take(&mut tk.gap);
            } else {
                tk.gap.push_str(p.text());
                blanks = true;
            }
        } else {
            commented = true;
            tk.gap.clear();
        }
    }
    if commented && tk.breaks == 0 {
        tk.gap = if blanks { " " } else { "" }.into();
    }
}

/// Pairs each opener with its closer (a mismatch is a parse error and cannot get here).
pub fn pair_brackets(tks: &mut [Tk], brackets: &Brackets) {
    let mut stack: Vec<usize> = Vec::new();
    for i in 0..tks.len() {
        if brackets.is_open(tks[i].kind) {
            stack.push(i);
        } else if brackets.is_close(tks[i].kind)
            && let Some(o) = stack.pop()
        {
            tks[o].pair = Some(i);
            tks[i].pair = Some(o);
        }
    }
}

/// Is the bracket pair of token `i` spread over several source lines?
pub fn multiline(tks: &[Tk], i: usize) -> bool {
    tks[i].pair.is_some_and(|p| {
        let (a, b) = (i.min(p), i.max(p));
        tks[a].last_line != tks[b].first_line
    })
}

/// The blanks in front of a comment on a line of its own, as the source has them.
fn source_indent(c: &Comment) -> String {
    match c.token().prev_token() {
        Some(p) if !p.text().contains('\n') && p.text().chars().all(char::is_whitespace) => {
            p.text().to_string()
        }
        _ => String::new(),
    }
}

/// The line breaks at the start of the file, before its first comment or token.
fn file_start_breaks(g: &Grammar, tok: &SyntaxToken) -> usize {
    let mut run = Vec::new();
    let mut cur = tok.prev_token();
    while let Some(p) = cur.filter(|p| g.is_trivia(p.kind())) {
        cur = p.prev_token();
        run.push(p);
    }
    (run.iter().rev())
        .take_while(|p| g.has_tag(p.kind(), "newline") || g.has_tag(p.kind(), "whitespace"))
        .map(|p| comments::count_breaks(p.text()))
        .sum()
}

fn indented(levels: usize, mut d: Doc) -> Doc {
    for _ in 0..levels {
        d = doc::indent(d);
    }
    d
}

/// Writes the tokens: a token with `newline` starts a line. `spacing` gives the blanks between
/// token `i - 1` and `i` on one line.
pub fn emit(
    cx: &FormatCtx<'_>,
    rules: &StreamRules,
    mut tks: Vec<Tk>,
    pipe: &[SyntaxKind],
    spacing: impl Fn(&[Tk], usize) -> String,
) -> Doc {
    let g = cx.grammar;
    let max_blank = cx.options.int(core_config::shared::MAX_BLANK_LINES) as usize;
    let brackets = &rules.brackets;
    let mut lines: Vec<Doc> = Vec::new();
    // The current line's parts and its indentation level.
    let mut cur: Vec<Doc> = Vec::new();
    let mut level = 0;
    let mut frames = Frames::default();
    let flush = |lines: &mut Vec<Doc>, cur: &mut Vec<Doc>, level: usize| {
        if !cur.is_empty() {
            lines.push(indented(level, doc::concat(std::mem::take(cur))));
        }
    };
    let blank_lines = |n: usize| (0..n.min(max_blank)).map(|_| doc::hardline());
    let eof = g.kind_by_name("Eof");
    let leads = if rules.source_indent {
        crate::stream_indent::Leads::new(cx, &tks)
    } else {
        Default::default()
    };
    let shifts = if rules.source_indent {
        let width = cx.options.int(core_config::shared::INDENT_SIZE) as usize;
        rules.indents.plan(cx, &tks, &leads, width)
    } else {
        Vec::new()
    };
    let shifted = |gap: &str, i: usize| -> String {
        match shifts.get(i) {
            Some(&d) if d != 0 => " ".repeat((gap.len() as isize + d).max(0) as usize),
            _ => gap.to_string(),
        }
    };

    let heredocs = if rules.source_indent {
        let width = cx.options.int(core_config::shared::INDENT_SIZE) as usize;
        rules.indents.heredocs(&tks, &shifts, width)
    } else {
        Default::default()
    };
    for i in 0..tks.len() {
        let is_eof = Some(tks[i].kind) == eof;
        if is_eof && tks[i].leading.is_empty() {
            if i == 0 && rules.source_indent && file_start_breaks(g, &tks[i].tok) > 0 {
                // A file of only blank lines keeps one line break, which the end trim leaves.
                cur.push(doc::verbatim("\n"));
            }
            continue;
        }
        let starts = i > 0 && (tks[i].newline || is_eof);
        if starts {
            if tks[i].cont {
                cur.push(doc::text(format!(
                    "{}{}",
                    tks[i].cont_gap,
                    rules.continuation.unwrap_or_default()
                )));
            }
            flush(&mut lines, &mut cur, level);
            // A pipeline or continuation line goes on in the next line.
            let (pipe, tick) = (tks[i - 1].is(pipe), tks[i].cont);
            frames.continue_line(pipe, tick);
            // Comments stand where the code in front of the closing bracket stands.
            let mut blank = if tks[i].breaks > 0 {
                comments::blank_lines_before(g, &tks[i].tok)
            } else {
                0
            };
            let (want, closes) = if rules.source_indent {
                (
                    rules.blanks.wanted(cx, &tks, leads.of(i)),
                    rules.blanks.closes(cx, &tks, i),
                )
            } else {
                (None, false)
            };
            if closes && tks[i].leading.is_empty() {
                blank = 0;
            }
            // Directives of a tool right after the code in front stay with it.
            let directive = blank == 0
                && (tks[i].leading.iter())
                    .take_while(|c| rules.blanks.is_directive(c))
                    .last()
                    .is_some_and(|c| c.breaks_after > 1);
            match want {
                Some(true) if directive => {}
                Some(true) => blank = blank.max(1),
                Some(false) => blank = 0,
                None => {}
            }
            let leading = std::mem::take(&mut tks[i].leading);
            let mut inline: Vec<Doc> = Vec::new();
            for (j, c) in leading.iter().enumerate() {
                if c.kind == comments::CommentKind::Block && c.breaks_after == 0 {
                    inline.push(crate::context::comment_doc_in(c, &cx.comment_style));
                    inline.push(doc::lit(" "));
                    continue;
                }
                let mut parts: Vec<Doc> = blank_lines(blank).collect();
                parts.push(doc::hardline());
                if rules.source_indent {
                    parts.push(doc::text(shifted(&source_indent(c), i)));
                }
                parts.push(crate::context::comment_doc_in(c, &cx.comment_style));
                let depth = if rules.source_indent {
                    0
                } else {
                    frames.depth()
                };
                lines.push(indented(depth, doc::concat(parts)));
                blank = c.breaks_after.saturating_sub(1);
                if closes && j + 1 == leading.len() {
                    blank = 0;
                }
            }
            if brackets.is_close(tks[i].kind) {
                frames.close(1);
            }
            level = if rules.source_indent {
                0
            } else {
                frames.depth()
            };
            cur.extend(blank_lines(blank));
            cur.push(doc::hardline());
            if rules.source_indent {
                cur.push(doc::text(shifted(&tks[i].gap, i)));
            }
            cur.extend(inline);
        } else if i > 0 {
            let gap = spacing(&tks, i);
            if !gap.is_empty() {
                cur.push(doc::text(gap));
            }
            if brackets.is_close(tks[i].kind) {
                frames.close(1);
            }
        } else {
            // The first token: its comments stand on lines of their own.
            if rules.source_indent {
                cur.extend((0..file_start_breaks(g, &tks[i].tok)).map(|_| doc::hardline()));
            }
            let leading = std::mem::take(&mut tks[i].leading);
            for (j, c) in leading.iter().enumerate() {
                cur.push(crate::context::comment_doc_in(c, &cx.comment_style));
                cur.push(doc::hardline());
                // A blank line closes the magic comments at the top of the file.
                let last_magic = rules.blanks.is_magic(c)
                    && leading.get(j + 1).is_none_or(|n| !rules.blanks.is_magic(n));
                let blank = c.breaks_after.saturating_sub(1);
                cur.extend(blank_lines(if last_magic { blank.max(1) } else { blank }));
            }
            if rules.source_indent {
                cur.push(doc::text(shifted(&tks[i].gap, i)));
            }
        }
        if !is_eof {
            // A token that spans lines is text of its own: its lines are not touched, except the
            // heredocs that stand in from their opener's line.
            let body = heredocs.get(&i);
            cur.push(if let Some(body) = body {
                doc::verbatim(body.as_str())
            } else if rules.source_indent && tks[i].first_line != tks[i].last_line {
                doc::verbatim(tks[i].text())
            } else {
                cx.token_text(&tks[i].tok)
            });
            cur.push(cx.trailing_comments_doc(&tks[i].tok));
        }
        if brackets.is_open(tks[i].kind) {
            frames.open(1);
        }
    }
    flush(&mut lines, &mut cur, level);
    doc::concat(lines)
}

/// A file written by the rows alone: every source line stays a line, the `[space]` rows give the
/// blanks between tokens on a line (no row keeps the source's; an `[at_least_one]` row adds a
/// blank where there is none).
pub fn format(cx: &FormatCtx<'_>, rows: &crate::rules::RuleFormatter, root: &SyntaxNode) -> Doc {
    let (rules, space) = (rows.stream(), rows.space());
    let mut tks = collect(cx.grammar, root, rules);
    pair_brackets(&mut tks, &rules.brackets);
    for tk in &mut tks {
        tk.newline = tk.breaks > 0 || tk.cont;
    }
    let spacing = |tks: &[Tk], i: usize| {
        let (prev, cur) = (&tks[i - 1].tok, &tks[i].tok);
        match space.want(prev, cur) {
            Some(true) => " ".into(),
            Some(false) => String::new(),
            None if tks[i].gap.is_empty() && space.at_least_one(prev, cur) => " ".into(),
            None => tks[i].gap.clone(),
        }
    };
    emit(cx, rules, tks, &[], spacing)
}
