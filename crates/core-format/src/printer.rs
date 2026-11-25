//! The greedy, language-unaware printer. It follows Prettier's algorithm: a group is printed flat
//! if the rest of its line (up to the next possible break) fits in the line width, else broken.
//!
//! concept: doc-printer
//! what: The greedy, language-unaware printer that fits groups to the line width.
//! tools: Prettier printDocToString
//! reads: grammar:language

use crate::doc::{self, Doc, GroupId, Outdent};
use std::collections::HashMap;
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintOptions {
    pub line_width: usize,
    /// Columns per indentation level.
    pub indent_width: usize,
    pub use_tabs: bool,
    /// With tabs, an alignment is tabs for each whole indent width and spaces for the rest.
    pub align_with_tabs: bool,
    /// Every line break of the document is a line continuation (the code of a Dockerfile `RUN`).
    pub continuation: Option<Continuation>,
}

/// The continuation character of the hosts whose code is read as continued lines.
pub const CONTINUATION_ESCAPE: char = '\\';

/// The line breaks of code that is one logical line: a line that holds code ends in ` <escape>`, a
/// line of blanks gets the escape character after them, a line that is a whole comment (the
/// character `comment` starts it) gets nothing, and neither does a line break inside text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Continuation {
    pub escape: char,
    pub comment: char,
}

impl Default for PrintOptions {
    fn default() -> Self {
        PrintOptions {
            line_width: 80,
            indent_width: 2,
            use_tabs: false,
            align_with_tabs: false,
            continuation: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Flat,
    Break,
}

/// What is left to print: one document or a run of them (a concat, or the rest of a fill).
#[derive(Clone, Copy)]
enum Item<'a> {
    Doc(&'a Doc),
    Seq(&'a [Doc]),
    Fill(&'a [Doc]),
}

#[derive(Clone, Copy)]
struct Cmd<'a> {
    indent: usize,
    mode: Mode,
    item: Item<'a>,
}

/// One interned indentation level.
struct Level {
    /// The level this one was made from (the root is its own parent).
    parent: usize,
    text: String,
    width: usize,
    /// The level one indent deeper, once it exists.
    indented: Option<usize>,
    /// The levels `align` made from this one, as `(spaces, level)`.
    aligned: Vec<(usize, usize)>,
    /// The levels an outdent by columns made from this one, as `(columns, level)`.
    dedented: Vec<(usize, usize)>,
}

/// Indentation levels are interned: a command carries an index into this arena, and a level is
/// built once however often the document indents by it.
struct Indents {
    levels: Vec<Level>,
    opts: PrintOptions,
}

impl Indents {
    fn new(opts: PrintOptions) -> Self {
        let mut indents = Indents {
            levels: Vec::new(),
            opts,
        };
        indents.push(0, String::new(), 0);
        indents
    }

    fn push(&mut self, parent: usize, text: String, width: usize) -> usize {
        self.levels.push(Level {
            parent,
            text,
            width,
            indented: None,
            aligned: Vec::new(),
            dedented: Vec::new(),
        });
        self.levels.len() - 1
    }

    fn indent(&mut self, parent: usize) -> usize {
        if let Some(i) = self.levels[parent].indented {
            return i;
        }
        let Level { text, width, .. } = &self.levels[parent];
        let w = self.opts.indent_width;
        let (text, width) = if self.opts.use_tabs {
            (format!("{text}\t"), width + w)
        } else {
            (format!("{text}{}", " ".repeat(w)), width + w)
        };
        let i = self.push(parent, text, width);
        self.levels[parent].indented = Some(i);
        i
    }

    fn outdent(&self, mut level: usize, levels: usize) -> usize {
        for _ in 0..levels {
            if level == 0 {
                break;
            }
            level = self.levels[level].parent;
        }
        level
    }

    /// The level `columns` columns narrower than `level` (tabs for whole indents when tabs are
    /// used, spaces for the rest).
    fn dedent(&mut self, level: usize, columns: usize) -> usize {
        if let Some((_, i)) = self.levels[level]
            .dedented
            .iter()
            .find(|(n, _)| *n == columns)
        {
            return *i;
        }
        let width = self.levels[level].width.saturating_sub(columns);
        let w = self.opts.indent_width.max(1);
        let text = if self.opts.use_tabs {
            format!("{}{}", "\t".repeat(width / w), " ".repeat(width % w))
        } else {
            " ".repeat(width)
        };
        let parent = self.levels[level].parent;
        let i = self.push(parent, text, width);
        self.levels[level].dedented.push((columns, i));
        i
    }

    fn align(&mut self, parent: usize, spaces: usize) -> usize {
        if let Some((_, i)) = self.levels[parent]
            .aligned
            .iter()
            .find(|(n, _)| *n == spaces)
        {
            return *i;
        }
        let Level { text, width, .. } = &self.levels[parent];
        let width = width + spaces;
        let w = self.opts.indent_width.max(1);
        let text = if self.opts.use_tabs && self.opts.align_with_tabs {
            format!("{}{}", "\t".repeat(width / w), " ".repeat(width % w))
        } else {
            format!("{text}{}", " ".repeat(spaces))
        };
        let i = self.push(parent, text, width);
        self.levels[parent].aligned.push((spaces, i));
        i
    }
}

fn str_width(s: &str, tab: usize) -> usize {
    // Printable ASCII is one column per byte.
    if s.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return s.len();
    }
    s.chars()
        .map(|c| {
            if c == '\t' {
                tab
            } else {
                c.width().unwrap_or(0)
            }
        })
        .sum()
}

/// Prints `doc`. Trailing whitespace before a newline is trimmed.
pub fn print(doc: &mut Doc, opts: &PrintOptions) -> String {
    print_protected(doc, opts).0
}

/// Like [`print`]; also returns the length of the output up to which trailing whitespace is
/// significant because it was written by [`Doc::Verbatim`].
pub fn print_protected(doc: &mut Doc, opts: &PrintOptions) -> (String, usize) {
    let (out, keep, _) = print_counting(doc, opts);
    (out, keep)
}

/// Work done by the printer besides writing: the number of steps `fits` took in total. It grows
/// linearly with the document when each group's measurement stops at its own line; a
/// measurement that rescans what follows would make it grow faster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintStats {
    pub fits_steps: usize,
}

/// Like [`print_protected`], with the printer's work counted.
pub fn print_counting(doc: &mut Doc, opts: &PrintOptions) -> (String, usize, PrintStats) {
    doc.propagate_breaks();
    let mut p = Printer {
        opts: *opts,
        indents: Indents::new(*opts),
        out: String::new(),
        pos: 0,
        suffixes: Vec::new(),
        suffix_at: None,
        line_level: 0,
        remeasure: false,
        group_modes: HashMap::new(),
        keep: 0,
        measure: Measure::default(),
    };
    p.run(doc);
    let stats = PrintStats {
        fits_steps: p.measure.steps,
    };
    (p.out, p.keep, stats)
}

struct Printer<'a> {
    opts: PrintOptions,
    indents: Indents,
    out: String,
    pos: usize,
    suffixes: Vec<Cmd<'a>>,
    /// Where the output stood when the last pending comments were queued.
    suffix_at: Option<usize>,
    /// The indentation level of the current line.
    line_level: usize,
    /// Set after a hard line in flat mode: the next group must be measured again.
    remeasure: bool,
    /// How each group with an id was printed, for `IndentIfBreak`.
    group_modes: HashMap<GroupId, Mode>,
    /// Output before this length ends with significant blanks (see [`Doc::Verbatim`]).
    keep: usize,
    /// Work list of [`fits`], kept between calls.
    measure: Measure<'a>,
}

static HARD_LINE: Doc = Doc::HardLine;

/// Does the pending suffix end in text that starts with `//` or `#`?
fn ends_in_line_comment(d: &Doc) -> bool {
    match d {
        Doc::Text(t) => t.starts_with("//") || t.starts_with('#'),
        Doc::Concat(ps) => ps
            .iter()
            .rev()
            .find(|p| !matches!(p, Doc::Nil | Doc::BreakParent))
            .is_some_and(ends_in_line_comment),
        _ => false,
    }
}

impl<'a> Printer<'a> {
    /// Queues the pending suffixes in front of the line break that follows; the line break a
    /// second comment of a line brought along takes the deeper indent of the line it follows and the line break that follows (formatting the
    /// output again reads it as a comment on one of them).
    fn flush_suffixes(&mut self, cmds: &mut Vec<Cmd<'a>>, indent: usize) {
        self.suffix_at = Some(self.out.len());
        for mut c in self.suffixes.drain(..).rev() {
            if matches!(c.item, Item::Doc(d) if std::ptr::eq(d, &HARD_LINE)) {
                c.indent = [indent, self.line_level]
                    .into_iter()
                    .max_by_key(|&l| self.indents.levels[l].width)
                    .unwrap_or(indent);
            }
            cmds.push(c);
        }
    }

    fn run(&mut self, root: &'a Doc) {
        let mut cmds = vec![Cmd {
            indent: 0,
            mode: Mode::Break,
            item: Item::Doc(root),
        }];
        loop {
            let Some(cmd) = cmds.pop() else {
                if self.suffixes.is_empty() {
                    break;
                }
                self.flush_suffixes(&mut cmds, 0);
                continue;
            };
            self.step(cmd, &mut cmds);
        }
    }

    /// How the group `id` was printed; a group not printed yet counts as flat.
    fn group_mode(&self, id: GroupId) -> Mode {
        self.group_modes.get(&id).copied().unwrap_or(Mode::Flat)
    }

    fn width_left(&self) -> isize {
        self.opts.line_width as isize - self.pos as isize
    }

    /// The line that ends here is continued: see [`Continuation`].
    fn continue_line(&mut self, c: Continuation) {
        let line = self.out.rfind('\n').map_or(0, |i| i + 1);
        let code = self.out[line..].trim();
        if self.out.is_empty() {
        } else if code.is_empty() {
            self.out.push(c.escape);
        } else if !code.starts_with(c.comment) {
            self.out.push(' ');
            self.out.push(c.escape);
        }
    }

    fn newline(&mut self, indent: usize) {
        if let Some(c) = self.opts.continuation {
            self.continue_line(c);
        }
        let trimmed = self.out.trim_end_matches([' ', '\t']).len().max(self.keep);
        self.out.truncate(trimmed);
        self.out.push('\n');
        self.line_level = indent;
        let level = &self.indents.levels[indent];
        self.out.push_str(&level.text);
        self.pos = level.width;
    }

    /// The column where the comment that ended the previous line starts (see [`Outdent::Suffix`]).
    fn suffix_column(&self) -> Option<usize> {
        let at = self.suffix_at?;
        let tail = self.out.get(at..)?;
        if tail.matches('\n').count() != usize::from(self.line_is_blank())
            || !tail.trim_start().starts_with("//")
        {
            return None;
        }
        let start = at + (tail.len() - tail.trim_start().len());
        let line = self.out[..start].rfind('\n').map_or(0, |i| i + 1);
        Some(str_width(&self.out[line..start], self.opts.indent_width))
    }

    /// The column of the first `//` on the line above the current blank one (a directive that
    /// was printed as written has no queued comment).
    fn previous_line_comment_column(&self) -> Option<usize> {
        if !self.line_is_blank() {
            return None;
        }
        let end = self.current_line_start().checked_sub(1)?;
        let start = self.out[..end].rfind('\n').map_or(0, |i| i + 1);
        let line = &self.out[start..end];
        let at = line.find("//")?;
        Some(str_width(&line[..at], self.opts.indent_width))
    }

    fn current_line_start(&self) -> usize {
        self.out.rfind('\n').map_or(0, |i| i + 1).max(self.keep)
    }

    /// Does the current line hold nothing but indentation?
    fn line_is_blank(&self) -> bool {
        self.out[self.current_line_start()..]
            .bytes()
            .all(|b| b == b' ' || b == b'\t')
    }

    /// See [`Doc::LineStart`].
    fn line_start(&mut self, s: &str) {
        let line_start = self.current_line_start();
        if self.line_is_blank() {
            self.out.truncate(line_start);
        } else {
            self.newline(0);
        }
        self.write(s);
    }

    /// See [`Doc::Outdent`]: moves the current line (or a new one) to indentation level `indent`.
    fn restart_line(&mut self, indent: usize) {
        if self.line_is_blank() {
            let line_start = self.current_line_start();
            self.out.truncate(line_start);
            let level = &self.indents.levels[indent];
            self.out.push_str(&level.text);
            self.pos = level.width;
            self.line_level = indent;
        } else {
            self.newline(indent);
        }
    }

    fn write(&mut self, s: &str) {
        self.out.push_str(s);
        match s.rfind('\n') {
            Some(i) => self.pos = str_width(&s[i + 1..], self.opts.indent_width),
            None => self.pos += str_width(s, self.opts.indent_width),
        }
    }

    fn step(&mut self, cmd: Cmd<'a>, cmds: &mut Vec<Cmd<'a>>) {
        let Cmd { indent, mode, item } = cmd;
        let doc = match item {
            Item::Doc(d) => d,
            Item::Seq(parts) => {
                cmds.extend(parts.iter().rev().map(|d| Cmd {
                    indent,
                    mode,
                    item: Item::Doc(d),
                }));
                return;
            }
            Item::Fill(parts) => return self.fill(indent, mode, parts, cmds),
        };
        let push = |cmds: &mut Vec<Cmd<'a>>, indent, mode, doc| {
            cmds.push(Cmd {
                indent,
                mode,
                item: Item::Doc(doc),
            })
        };
        match doc {
            Doc::Nil | Doc::BreakParent => {}
            Doc::Shared(s) => push(cmds, indent, mode, &s.doc),
            Doc::Label(_, d) => push(cmds, indent, mode, d),
            Doc::LineSuffixBoundary => {
                if !self.suffixes.is_empty() {
                    push(cmds, indent, mode, &Doc::HardLine);
                }
            }
            Doc::IndentIfBreak(id, d) => {
                let broken = self.group_modes.get(id) == Some(&Mode::Break);
                let i = if broken {
                    self.indents.indent(indent)
                } else {
                    indent
                };
                push(cmds, i, mode, d);
            }
            Doc::Text(s) => self.write(s),
            Doc::Reserve(_) => {}
            Doc::FitsBelow(f) => {
                let mut below = indent;
                for _ in 0..f.levels {
                    below = self.indents.indent(below);
                }
                let room =
                    self.opts.line_width as isize - self.indents.levels[below].width as isize;
                let probe = Cmd {
                    indent: below,
                    mode: Mode::Flat,
                    item: Item::Doc(&f.probe),
                };
                let fits_below = fits(
                    &mut self.measure,
                    probe,
                    cmds,
                    room,
                    false,
                    &self.group_modes,
                    !self.suffixes.is_empty(),
                );
                push(
                    cmds,
                    indent,
                    mode,
                    if fits_below { &f.fits } else { &f.otherwise },
                );
            }
            Doc::Reflow(r) => {
                let (s, limit) = &**r;
                for (i, line) in wrap_line_comment(s, self.pos, *limit).iter().enumerate() {
                    if i > 0 {
                        self.newline(indent);
                    }
                    self.write(line);
                }
            }
            Doc::Verbatim(s) => {
                self.write(s);
                self.keep = self.out.len();
            }
            Doc::LineStart(s) => self.line_start(s),
            Doc::NewlineIfNeeded => {
                if self.line_is_blank() {
                    return;
                }
                if mode == Mode::Flat {
                    self.remeasure = true;
                }
                if !self.suffixes.is_empty() {
                    cmds.push(cmd);
                    self.flush_suffixes(cmds, indent);
                    return;
                }
                self.newline(indent);
            }
            Doc::Concat(parts) => cmds.push(Cmd {
                indent,
                mode,
                item: Item::Seq(parts),
            }),
            Doc::Indent(d) => {
                let i = self.indents.indent(indent);
                push(cmds, i, mode, d);
            }
            Doc::Outdent(by, d) => {
                if !self.suffixes.is_empty() {
                    cmds.push(cmd);
                    self.flush_suffixes(cmds, indent);
                    return;
                }
                let i = match *by {
                    Outdent::Levels(levels) => self.indents.outdent(indent, levels),
                    Outdent::Columns(columns) => self.indents.dedent(indent, columns),
                    Outdent::Suffix(source) => {
                        let col = (self.suffix_column())
                            .or_else(|| self.previous_line_comment_column())
                            .unwrap_or(source);
                        self.indents.align(0, col)
                    }
                };
                self.restart_line(i);
                push(cmds, i, mode, d);
            }
            Doc::Align(n, d) => {
                // `doc::ALIGN_TO_COLUMN`: the lines of the contents start at the column the
                // contents start at.
                let (base, n) = if *n == doc::ALIGN_TO_COLUMN {
                    (
                        indent,
                        self.pos.saturating_sub(self.indents.levels[indent].width),
                    )
                } else if *n & doc::ALIGN_FROM_LINE != 0 {
                    (self.line_level, *n & !doc::ALIGN_FROM_LINE)
                } else {
                    (indent, *n)
                };
                let i = self.indents.align(base, n);
                push(cmds, i, mode, d);
            }
            Doc::IfBreak {
                broken,
                flat,
                group,
            } => {
                let m = group.map_or(mode, |id| self.group_mode(id));
                push(
                    cmds,
                    indent,
                    mode,
                    if m == Mode::Break { broken } else { flat },
                );
            }
            Doc::Fill(parts) => cmds.push(Cmd {
                indent,
                mode,
                item: Item::Fill(parts),
            }),
            Doc::LineSuffix(d) => {
                // A second comment on a line that ends in a line comment goes on a line of its
                // own, or it would become part of the first.
                let after_line_comment = self
                    .suffixes
                    .last()
                    .is_some_and(|c| matches!(c.item, Item::Doc(p) if ends_in_line_comment(p)));
                let mut parts: &'a [Doc] = std::slice::from_ref(&**d);
                if after_line_comment {
                    self.suffixes.push(Cmd {
                        indent,
                        mode,
                        item: Item::Doc(&HARD_LINE),
                    });
                    if let Doc::Concat(ps) = &**d
                        && matches!(ps.first(), Some(Doc::Text(t)) if t.trim().is_empty())
                    {
                        parts = &ps[1..];
                    }
                }
                for part in parts {
                    self.suffixes.push(Cmd {
                        indent,
                        mode,
                        item: Item::Doc(part),
                    });
                }
            }
            Doc::Line | Doc::SoftLine | Doc::HardLine => {
                if mode == Mode::Flat && !matches!(doc, Doc::HardLine) {
                    if matches!(doc, Doc::Line) {
                        self.write(" ");
                    }
                    return;
                }
                if mode == Mode::Flat {
                    self.remeasure = true;
                }
                if !self.suffixes.is_empty() {
                    // Trailing comments go out before the newline.
                    cmds.push(cmd);
                    self.flush_suffixes(cmds, indent);
                    return;
                }
                self.newline(indent);
            }
            Doc::Group(g) => {
                let flat_cmd = |doc| Cmd {
                    indent,
                    mode: Mode::Flat,
                    item: Item::Doc(doc),
                };
                // A candidate with a forced break of its own is printed in break mode (its groups
                // are measured one by one) once it was found to fit; a flat run would measure
                // only the first group after the break.
                let chosen_cmd = |c: Cmd<'a>| match c.item {
                    Item::Doc(Doc::Shared(s)) if s.forced => Cmd {
                        mode: Mode::Break,
                        ..c
                    },
                    _ => c,
                };
                if mode == Mode::Flat && !self.remeasure {
                    let (m, d) = match (g.broken, g.states.last()) {
                        (true, Some(last)) => (Mode::Break, last),
                        (true, None) => (Mode::Break, &g.contents),
                        (false, _) => (Mode::Flat, &g.contents),
                    };
                    if let Some(id) = g.id {
                        self.group_modes.insert(id, m);
                    }
                    push(cmds, indent, m, d);
                    return;
                }
                self.remeasure = false;
                let next = flat_cmd(&g.contents);
                let width = self.width_left();
                if !g.broken
                    && fits(
                        &mut self.measure,
                        next,
                        cmds,
                        width,
                        false,
                        &self.group_modes,
                        !self.suffixes.is_empty(),
                    )
                {
                    if let Some(id) = g.id {
                        self.group_modes.insert(id, Mode::Flat);
                    }
                    cmds.push(if g.states.is_empty() {
                        next
                    } else {
                        chosen_cmd(next)
                    });
                } else if let Some(most_expanded) = g.states.last() {
                    if g.broken {
                        push(cmds, indent, Mode::Break, most_expanded);
                        return;
                    }
                    let chosen = g.states[1..].iter().map(flat_cmd).find(|c| {
                        fits(
                            &mut self.measure,
                            *c,
                            cmds,
                            width,
                            false,
                            &self.group_modes,
                            !self.suffixes.is_empty(),
                        )
                    });
                    match chosen {
                        Some(c) => cmds.push(chosen_cmd(c)),
                        None => push(cmds, indent, Mode::Break, most_expanded),
                    }
                } else {
                    if let Some(id) = g.id {
                        self.group_modes.insert(id, Mode::Break);
                    }
                    push(cmds, indent, Mode::Break, &g.contents);
                }
            }
        }
    }

    fn fill(&mut self, indent: usize, mode: Mode, parts: &'a [Doc], cmds: &mut Vec<Cmd<'a>>) {
        let cmd = |mode, item| Cmd { indent, mode, item };
        let width = self.width_left();
        let [content, rest @ ..] = parts else { return };
        let content_flat = cmd(Mode::Flat, Item::Doc(content));
        let content_fits = fits(
            &mut self.measure,
            content_flat,
            &[],
            width,
            true,
            &self.group_modes,
            !self.suffixes.is_empty(),
        );
        let [whitespace, rest @ ..] = rest else {
            let m = if content_fits {
                Mode::Flat
            } else {
                Mode::Break
            };
            cmds.push(cmd(m, Item::Doc(content)));
            return;
        };
        let ws = |m| cmd(m, Item::Doc(whitespace));
        let content_cmd = |m| cmd(m, Item::Doc(content));
        if rest.is_empty() {
            if content_fits {
                cmds.extend([ws(Mode::Flat), content_cmd(Mode::Flat)]);
            } else {
                cmds.extend([ws(Mode::Break), content_cmd(Mode::Break)]);
            }
            return;
        }
        let remaining = cmd(mode, Item::Fill(rest));
        let first_two = cmd(Mode::Flat, Item::Seq(&parts[..3]));
        let first_two_fit = fits(
            &mut self.measure,
            first_two,
            &[],
            width,
            true,
            &self.group_modes,
            !self.suffixes.is_empty(),
        );
        if first_two_fit {
            cmds.extend([remaining, ws(Mode::Flat), content_cmd(Mode::Flat)]);
        } else if content_fits {
            cmds.extend([remaining, ws(Mode::Break), content_cmd(Mode::Flat)]);
        } else {
            cmds.extend([remaining, ws(Mode::Break), content_cmd(Mode::Break)]);
        }
    }
}

/// The lines of the line comment `text` that starts at column `col`: a line wider than `limit`
/// breaks at the last blank that keeps it within the limit (not within the `//` and the first
/// character after it) and goes on in a line that starts with `//` (google-java-format).
fn wrap_line_comment(text: &str, col: usize, limit: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut chars: Vec<char> = text.chars().collect();
    while chars.len() + col > limit {
        let mut at = limit.saturating_sub(col).min(chars.len() - 1);
        while at >= 2 && !chars[at].is_whitespace() {
            at -= 1;
        }
        if at <= 2 {
            break;
        }
        lines.push(
            chars[..at]
                .iter()
                .collect::<String>()
                .trim_end()
                .to_string(),
        );
        chars = "//".chars().chain(chars[at..].iter().copied()).collect();
    }
    lines.push(chars.into_iter().collect());
    lines
}

/// The work list of [`fits`] and a count of the steps it took.
#[derive(Default)]
struct Measure<'a> {
    stack: Vec<(Mode, Item<'a>)>,
    steps: usize,
}

/// Takes the width of `s` off `width`. Only the first line of text with a line break counts
/// against the current line: then the answer to "does it fit" is known (`Some`).
fn measure(s: &str, width: &mut isize) -> Option<bool> {
    match s.find('\n') {
        Some(i) => Some(*width - (str_width(&s[..i], 1) as isize) >= 0),
        None => {
            *width -= str_width(s, 1) as isize;
            None
        }
    }
}

/// Does `next`, followed by whatever is already queued in `rest`, fit in `width` columns up to
/// the first line break in break mode?
fn fits<'a>(
    m: &mut Measure<'a>,
    next: Cmd<'a>,
    rest: &[Cmd<'a>],
    width: isize,
    must_be_flat: bool,
    groups: &HashMap<GroupId, Mode>,
    mut suffix: bool,
) -> bool {
    let mut width = width;
    let mut rest_idx = rest.len();
    m.stack.clear();
    m.stack.push((next.mode, next.item));
    while width >= 0 {
        m.steps += 1;
        let Some((mode, item)) = m.stack.pop() else {
            if rest_idx == 0 {
                return true;
            }
            rest_idx -= 1;
            m.stack.push((rest[rest_idx].mode, rest[rest_idx].item));
            continue;
        };
        let doc = match item {
            Item::Doc(d) => d,
            Item::Seq(parts) | Item::Fill(parts) => {
                m.stack
                    .extend(parts.iter().rev().map(|d| (mode, Item::Doc(d))));
                continue;
            }
        };
        match doc {
            Doc::Nil | Doc::BreakParent => {}
            Doc::LineSuffix(_) => suffix = true,
            Doc::Shared(s) => m.stack.push((mode, Item::Doc(&s.doc))),
            Doc::IndentIfBreak(_, d) | Doc::Label(_, d) => m.stack.push((mode, Item::Doc(d))),
            Doc::LineSuffixBoundary => {
                if suffix {
                    return true;
                }
            }
            Doc::Text(s) => {
                if let Some(answer) = measure(s, &mut width) {
                    return answer;
                }
            }
            Doc::Reflow(r) => {
                let s = &r.0;
                if let Some(answer) = measure(s, &mut width) {
                    return answer;
                }
            }
            Doc::Reserve(n) => width -= *n as isize,
            Doc::FitsBelow(f) => m.stack.push((mode, Item::Doc(&f.fits))),
            Doc::Verbatim(s) => {
                if let Some(answer) = measure(s, &mut width) {
                    return answer;
                }
            }
            Doc::LineStart(_) | Doc::Outdent(..) | Doc::NewlineIfNeeded => return true,
            Doc::Concat(parts) => m.stack.push((mode, Item::Seq(parts))),
            Doc::Indent(d) | Doc::Align(_, d) => m.stack.push((mode, Item::Doc(d))),
            Doc::IfBreak {
                broken,
                flat,
                group,
            } => {
                let gm = group.map_or(mode, |id| groups.get(&id).copied().unwrap_or(Mode::Flat));
                m.stack.push((
                    mode,
                    Item::Doc(if gm == Mode::Break { broken } else { flat }),
                ));
            }
            Doc::Fill(parts) => m.stack.push((mode, Item::Fill(parts))),
            Doc::Group(g) => {
                if must_be_flat && g.broken {
                    return false;
                }
                let group_mode = if g.broken { Mode::Break } else { mode };
                let contents = match (group_mode, g.states.last()) {
                    (Mode::Break, Some(last)) => last,
                    _ => &g.contents,
                };
                m.stack.push((group_mode, Item::Doc(contents)));
            }
            Doc::Line | Doc::SoftLine | Doc::HardLine => {
                if mode == Mode::Break || matches!(doc, Doc::HardLine) {
                    return true;
                }
                if matches!(doc, Doc::Line) {
                    width -= 1;
                }
            }
        }
    }
    false
}
