//! What the formatter reads from the tree: items (lines, directives, own-line comments), where
//! a line is the tokens of one unwrapped line with its comments and its blocks (`{ ... }` that
//! hold statements) in place.

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, SyntaxNode};
use rowan::WalkEvent;

#[derive(Clone, Copy, Debug)]
pub struct Tk<'a> {
    pub kind: SyntaxKind,
    pub text: &'a str,
    /// Line breaks between the previous token or comment and this one.
    pub nl: u32,
    /// The brace of a block that is printed on one line with its statement.
    pub blk: bool,
    /// Blanks or a line break before it in the source.
    pub sp: bool,
}

impl Tk<'_> {
    pub fn word(&self, w: &str) -> bool {
        matches!(self.kind, k::IDENT | k::KEYWORD) && self.text == w
    }

    pub fn comment(&self) -> bool {
        self.kind == k::COMMENT
    }
}

pub struct Item<'a> {
    /// Blank lines before it in the source.
    pub blank: u32,
    pub kind: ItemKind<'a>,
}

pub enum ItemKind<'a> {
    Line(Line<'a>),
    Dir(Dir<'a>),
    /// A comment on lines of its own.
    Comment(Tk<'a>),
}

pub struct Line<'a> {
    pub elems: Vec<Elem<'a>>,
    /// The source text of the line, for a line that is kept as written.
    pub raw: &'a str,
    /// A preprocessor directive inside the line.
    pub has_dir: bool,
}

pub enum Elem<'a> {
    T(Tk<'a>),
    Block(Block<'a>),
}

pub struct Block<'a> {
    pub open: Tk<'a>,
    pub items: Vec<Item<'a>>,
    pub close: Option<Tk<'a>>,
}

pub struct Dir<'a> {
    /// All tokens and comments of the directive; a line break in it is a continuation.
    pub toks: Vec<Tk<'a>>,
}

enum Ctx<'a> {
    Root(Vec<Item<'a>>),
    Block {
        open: Option<Tk<'a>>,
        items: Vec<Item<'a>>,
        close: Option<Tk<'a>>,
    },
    Line {
        elems: Vec<Elem<'a>>,
        first_nl: u32,
        start: usize,
        end: usize,
        has_dir: bool,
    },
    /// A group or a wrapper node whose tokens belong to the line around it; `init`: the braces
    /// of an initializer list.
    Flat {
        init: bool,
    },
    Dir {
        toks: Vec<Tk<'a>>,
        start: usize,
        end: usize,
    },
}

struct Builder<'a> {
    src: &'a str,
    ctx: Vec<Ctx<'a>>,
    pending: Vec<Tk<'a>>,
    nl: u32,
    sp: bool,
    done: Vec<Item<'a>>,
}

pub fn build<'a>(root: &SyntaxNode, src: &'a str) -> Vec<Item<'a>> {
    let mut b = Builder {
        src,
        ctx: Vec::new(),
        pending: Vec::new(),
        nl: 0,
        sp: false,
        done: Vec::new(),
    };
    for ev in root.preorder_with_tokens() {
        match ev {
            WalkEvent::Enter(e) => match e {
                rowan::NodeOrToken::Node(n) => {
                    let semi = n.kind() == k::BRACE_GROUP
                        && n.children()
                            .any(|l| l.children_with_tokens().any(|c| c.kind() == k::SEMI));
                    b.enter(n.kind(), semi)
                }
                rowan::NodeOrToken::Token(t) => {
                    let r = t.text_range();
                    b.token(t.kind(), usize::from(r.start()), usize::from(r.end()));
                }
            },
            WalkEvent::Leave(rowan::NodeOrToken::Node(n)) => b.leave(n.kind()),
            WalkEvent::Leave(_) => {}
        }
    }
    b.done
}

impl<'a> Builder<'a> {
    /// The innermost context that is not a wrapper.
    fn home(&self) -> Option<usize> {
        self.ctx
            .iter()
            .rposition(|c| !matches!(c, Ctx::Flat { .. }))
    }

    fn enter(&mut self, kind: SyntaxKind, semi: bool) {
        let home = self.home();
        let at_items = matches!(
            home.map(|h| &self.ctx[h]),
            None | Some(Ctx::Root(_)) | Some(Ctx::Block { .. })
        );
        let c = match kind {
            k::TRANSLATION_UNIT => Ctx::Root(Vec::new()),
            k::LINE if at_items => Ctx::Line {
                elems: Vec::new(),
                first_nl: 0,
                start: usize::MAX,
                end: 0,
                has_dir: false,
            },
            k::BRACE_GROUP => match home.map(|h| &self.ctx[h]) {
                Some(Ctx::Line { elems, .. }) if semi || self.is_block(elems) => Ctx::Block {
                    open: None,
                    items: Vec::new(),
                    close: None,
                },
                _ => Ctx::Flat { init: true },
            },
            k::DIRECTIVE if at_items => Ctx::Dir {
                toks: Vec::new(),
                start: usize::MAX,
                end: 0,
            },
            k::DIRECTIVE => match home {
                Some(h) if matches!(self.ctx[h], Ctx::Line { .. }) => {
                    self.split_line(h);
                    Ctx::Dir {
                        toks: Vec::new(),
                        start: usize::MAX,
                        end: 0,
                    }
                }
                _ => Ctx::Flat { init: false },
            },
            _ => Ctx::Flat { init: false },
        };
        self.ctx.push(c);
    }

    fn leave(&mut self, kind: SyntaxKind) {
        let Some(c) = self.ctx.pop() else { return };
        match c {
            Ctx::Root(items) => {
                let mut items = items;
                items.append(&mut self.done);
                self.done = items;
            }
            Ctx::Line {
                mut elems,
                first_nl,
                start,
                mut end,
                has_dir,
            } => {
                while self.pending.first().is_some_and(|c| c.nl == 0) {
                    let c = self.pending.remove(0);
                    end = end.max(self.offset(c.text) + c.text.len());
                    elems.push(Elem::T(c));
                }
                if start == usize::MAX {
                    return;
                }
                let raw = &self.src[start..end.max(start)];
                let line = Line {
                    elems,
                    raw,
                    has_dir,
                };
                self.item(Item {
                    blank: first_nl.saturating_sub(1),
                    kind: ItemKind::Line(line),
                });
            }
            Ctx::Block { open, items, close } => {
                let Some(open) = open else { return };
                let block = Block { open, items, close };
                let at = close.map_or(0, |c| self.offset(c.text) + c.text.len());
                let from = self.offset(open.text);
                if let Some(h) = self.home()
                    && let Ctx::Line {
                        elems, start, end, ..
                    } = &mut self.ctx[h]
                {
                    *start = (*start).min(from);
                    *end = (*end).max(at);
                    elems.push(Elem::Block(block));
                }
            }
            Ctx::Dir { toks, start, .. } => {
                if start != usize::MAX {
                    let first = toks.first().map_or(0, |t| t.nl);
                    self.item(Item {
                        blank: first.saturating_sub(1),
                        kind: ItemKind::Dir(Dir { toks }),
                    });
                }
            }
            Ctx::Flat { .. } => {
                let _ = kind;
            }
        }
    }

    /// A directive in the middle of a line: what the line has so far is an item (kept as written
    /// unless it is a whole statement), and the rest of the line, after the directive, is one too.
    fn split_line(&mut self, h: usize) {
        while self.pending.first().is_some_and(|c| c.nl == 0) {
            let c = self.pending.remove(0);
            let at = self.offset(c.text) + c.text.len();
            if let Ctx::Line { elems, end, .. } = &mut self.ctx[h] {
                *end = (*end).max(at);
                elems.push(Elem::T(c));
            }
        }
        let Ctx::Line {
            elems,
            first_nl,
            start,
            end,
            has_dir,
        } = &mut self.ctx[h]
        else {
            return;
        };
        let elems = std::mem::take(elems);
        let (first, s, e, rest) = (*first_nl, *start, *end, *has_dir);
        *first_nl = 0;
        *start = usize::MAX;
        *end = 0;
        *has_dir = true;
        if s == usize::MAX {
            return;
        }
        let complete = match elems
            .iter()
            .rev()
            .find(|e| !matches!(e, Elem::T(t) if t.comment()))
        {
            Some(Elem::T(t)) => t.kind == k::SEMI,
            Some(Elem::Block(b)) => b.close.is_some(),
            None => false,
        };
        let line = Line {
            elems,
            raw: &self.src[s..e.max(s)],
            has_dir: !complete || rest,
        };
        self.item(Item {
            blank: first.saturating_sub(1),
            kind: ItemKind::Line(line),
        });
    }

    /// Appends an item to the innermost block or the file.
    fn item(&mut self, item: Item<'a>) {
        let at = self
            .ctx
            .iter()
            .rposition(|c| matches!(c, Ctx::Root(_) | Ctx::Block { .. }));
        match at.map(|h| &mut self.ctx[h]) {
            Some(Ctx::Root(items)) | Some(Ctx::Block { items, .. }) => items.push(item),
            _ => self.done.push(item),
        }
    }

    fn offset(&self, text: &str) -> usize {
        text.as_ptr() as usize - self.src.as_ptr() as usize
    }

    fn token(&mut self, kind: SyntaxKind, start: usize, end: usize) {
        let text = &self.src[start..end];
        match kind {
            k::NEWLINE => {
                self.sp = true;
                self.nl += text.matches('\n').count() as u32;
                return;
            }
            k::WHITESPACE => {
                self.sp = true;
                return;
            }
            k::CONTINUATION => {
                if let Some(Ctx::Dir { .. }) = self.home().map(|h| &self.ctx[h]) {
                    self.nl += 1;
                }
                return;
            }
            _ => {}
        }
        let tk = Tk {
            kind,
            text,
            nl: self.nl,
            blk: false,
            sp: self.sp,
        };
        self.nl = 0;
        self.sp = false;
        let Some(h) = self.home() else { return };
        if let Ctx::Dir { toks, .. } = &self.ctx[h] {
            if toks.is_empty() && kind == k::COMMENT {
                self.pending.push(tk);
                return;
            }
            if toks.is_empty() {
                // Comments above the directive are items of their own.
                self.flush_standalone(h);
            }
        }
        if let Ctx::Dir {
            toks,
            start: s,
            end: e,
        } = &mut self.ctx[h]
        {
            *s = (*s).min(start);
            *e = (*e).max(end);
            toks.push(tk);
            return;
        }
        if kind == k::COMMENT {
            self.pending.push(tk);
            return;
        }
        if kind == k::END_OF_FILE {
            self.flush_standalone(h);
            return;
        }
        match &self.ctx[h] {
            Ctx::Block { open, .. } => {
                if open.is_none() {
                    // Comments before the `{` belong to the line that holds the block.
                    let before = std::mem::take(&mut self.pending);
                    if let Some(p) = self.ctx[..h]
                        .iter_mut()
                        .rposition(|c| matches!(c, Ctx::Line { .. }))
                        && let Ctx::Line { elems, .. } = &mut self.ctx[p]
                    {
                        elems.extend(before.into_iter().map(Elem::T));
                    }
                    if let Ctx::Block { open, .. } = &mut self.ctx[h] {
                        *open = Some(tk);
                    }
                } else {
                    self.flush_standalone(h);
                    if let Ctx::Block { close, .. } = &mut self.ctx[h] {
                        *close = Some(tk);
                    }
                }
            }
            Ctx::Line { .. } => {
                let first = matches!(&self.ctx[h], Ctx::Line { elems, .. } if elems.is_empty());
                if first {
                    // Own-line comments above the line are items of their own.
                    let comments = std::mem::take(&mut self.pending);
                    for c in comments {
                        self.item(Item {
                            blank: c.nl.saturating_sub(1),
                            kind: ItemKind::Comment(c),
                        });
                    }
                }
                let pending = std::mem::take(&mut self.pending);
                if let Ctx::Line {
                    elems,
                    first_nl,
                    start: s,
                    end: e,
                    ..
                } = &mut self.ctx[h]
                {
                    if first {
                        *first_nl = tk.nl;
                    }
                    elems.extend(pending.into_iter().map(Elem::T));
                    elems.push(Elem::T(tk));
                    *s = (*s).min(start);
                    *e = (*e).max(end);
                }
            }
            _ => {}
        }
    }

    /// The comments before the closing brace (or the end of the file) are items of their own.
    fn flush_standalone(&mut self, _h: usize) {
        for c in std::mem::take(&mut self.pending) {
            self.item(Item {
                blank: c.nl.saturating_sub(1),
                kind: ItemKind::Comment(c),
            });
        }
    }

    /// Whether a `{` after these tokens opens a block of statements (or members), not an
    /// initializer list.
    fn is_block(&self, elems: &[Elem<'a>]) -> bool {
        let init_around = matches!(
            self.ctx
                .iter()
                .rev()
                .find(|c| !matches!(c, Ctx::Flat { init: false })),
            Some(Ctx::Flat { init: true })
        );
        let toks: Vec<&Tk> = elems
            .iter()
            .filter_map(|e| match e {
                Elem::T(t) if !t.comment() => Some(t),
                _ => None,
            })
            .collect();
        let Some(p) = toks.last() else { return true };
        if init_around && matches!(p.kind, k::COMMA | k::LBRACE) {
            return false;
        }
        match p.kind {
            k::RPAR | k::RSQB | k::SEMI | k::RBRACE | k::LBRACE | k::COLON => true,
            k::STRING => toks.first().is_some_and(|t| t.word("extern")),
            k::IDENT | k::KEYWORD => {
                if matches!(
                    p.text,
                    "else"
                        | "do"
                        | "try"
                        | "const"
                        | "volatile"
                        | "noexcept"
                        | "override"
                        | "final"
                        | "mutable"
                        | "namespace"
                        | "struct"
                        | "class"
                        | "union"
                        | "enum"
                ) {
                    return true;
                }
                record_body(&toks) || arrow_return(&toks)
            }
            k::GREATER | k::GREATER_GREATER => arrow_return(&toks),
            _ => false,
        }
    }
}

/// `struct S : B {`, `class A final {`, `enum class E : int {`, `namespace a::b {`: the tokens
/// after the keyword hold one (qualified) name, template arguments, attributes and a base list.
fn record_body(toks: &[&Tk]) -> bool {
    let Some(at) = toks.iter().rposition(|t| {
        matches!(t.text, "struct" | "class" | "union" | "enum" | "namespace")
            && t.kind == k::KEYWORD
    }) else {
        return false;
    };
    let mut names = 0;
    let mut after_name = false;
    let mut depth = 0i32;
    for t in &toks[at + 1..] {
        match t.kind {
            k::LPAR | k::LSQB | k::LESS => depth += 1,
            k::RPAR | k::RSQB | k::GREATER => depth -= 1,
            k::GREATER_GREATER => depth -= 2,
            k::COLON if depth == 0 => return true,
            k::COLON_COLON => after_name = false,
            k::IDENT | k::KEYWORD if depth == 0 => {
                if matches!(t.text, "final" | "class" | "struct" | "alignas")
                    || t.text.starts_with("__")
                {
                    continue;
                }
                if after_name {
                    names += 1;
                }
                if names > 0 {
                    return false;
                }
                after_name = true;
            }
            _ => {}
        }
    }
    true
}

/// `auto f() -> T {` and `[] () -> T {`: a trailing return type.
fn arrow_return(toks: &[&Tk]) -> bool {
    toks.iter().any(|t| t.kind == k::ARROW)
        && toks.iter().any(|t| t.kind == k::RPAR)
        && !toks.iter().any(|t| t.kind == k::EQUAL)
}
