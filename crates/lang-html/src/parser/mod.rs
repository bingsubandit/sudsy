//! One hand-written parser for HTML and the template dialects built on it (Vue, Svelte, Angular):
//! a single pass over the bytes that emits the tokens and the node spans of `grammars/html.grammar`;
//! `provider_api::adapter::replay` builds the lossless tree (blanks and comments between tokens are
//! trivia, anything else it finds there is an error token).
//!
//! The tree is the source as written, not the browser's repaired tree: void elements and elements
//! closed by an implied end tag have no `EndTag`. Errors are reported and parsing goes on, so every
//! file has a tree. The dialects differ in what they add to the same shapes: Vue's `{{ }}` and
//! directive names, Svelte's `{ }` tags and blocks, Angular's control flow (`angular.rs`).
//!
//! concept: recursive-descent-parser

mod actions;
mod angular;
mod svelte;
mod tags;

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, grammars};
use provider_api::adapter::{Node, Tok, replay_grammar};
use provider_api::{ParseError, ParseResult};
use std::borrow::Cow;

/// Deeper nesting of elements is an error: the elements below are not nested any more, which
/// bounds the recursion of the formatter and of tree destruction.
pub const MAX_DEPTH: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Html,
    Vue,
    Svelte,
    Angular,
}

pub fn parse(src: &str, mode: Mode) -> ParseResult {
    parse_with(src, mode, false, true)
}

/// `templates`: `{{ }}` actions of a template language stay as written (see `actions.rs`); `balanced`: their blocks must open and close in one element.
pub fn parse_with(src: &str, mode: Mode, templates: bool, balanced: bool) -> ParseResult {
    let mut p = P {
        templates,
        balanced,
        blocks: core_syntax::template_actions::Blocks::default(),
        src,
        b: src.as_bytes(),
        mode,
        pos: 0,
        toks: Vec::with_capacity(src.len() / 6),
        nodes: Vec::with_capacity(src.len() / 12),
        errors: Vec::new(),
        stack: Vec::new(),
        deep: false,
        page: false,
        last_void: None,
        cdata: 0,
    };
    p.run();
    let mut parsed = replay_grammar(src, grammars::HTML.get(), &p.toks, &p.nodes, |t, _| t);
    parsed.errors.append(&mut p.errors);
    parsed.errors.sort_by_key(|e| e.start);
    parsed
}

/// The length of the start-tag comment at the start of `rest`: `//` up to the line break, its
/// trailing blanks left out; `/*` through `*/`; `None` for none or one that is not closed.
fn ng_comment_len(rest: &str) -> Option<usize> {
    if rest.starts_with("//") {
        Some(
            rest.find(['\n', '\r'])
                .map_or(rest, |i| &rest[..i])
                .trim_end()
                .len(),
        )
    } else if let Some(body) = rest.strip_prefix("/*") {
        body.find("*/").map(|i| i + 4)
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Open {
    /// An element: the range of its name.
    Element(u32, u32),
    /// A Svelte or Angular block.
    Block,
    /// An Angular ICU expression and one of its cases.
    Icu,
    IcuCase,
}

struct P<'a> {
    src: &'a str,
    b: &'a [u8],
    mode: Mode,
    /// HTML with `{{ }}` actions (see `actions.rs`).
    templates: bool,
    balanced: bool,
    blocks: core_syntax::template_actions::Blocks,
    pos: usize,
    toks: Vec<Tok>,
    nodes: Vec<Node>,
    errors: Vec<ParseError>,
    /// The open elements and blocks, with the index of their node.
    stack: Vec<(Open, usize)>,
    deep: bool,
    /// A Vue file that is an HTML page (a doctype or an `html` root): no blocks.
    page: bool,
    /// The last void element: its node, the token count after it and its lowercase name.
    last_void: Option<(usize, usize, Cow<'a, str>)>,
    /// Angular: the end of the CDATA section being read; 0 outside one.
    cdata: usize,
}

/// The ASCII-lowercase name; borrowed unless it has upper-case letters.
fn lowercase(s: &str) -> Cow<'_, str> {
    if s.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(s.to_ascii_lowercase())
    } else {
        Cow::Borrowed(s)
    }
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c)
}

impl<'a> P<'a> {
    fn at(&self, i: usize) -> u8 {
        self.b.get(i).copied().unwrap_or(0)
    }

    fn error(&mut self, start: usize, end: usize, message: &str) {
        let len = end.saturating_sub(start).max(1);
        self.errors
            .push(ParseError::syntax(start as u32, len as u32, message));
    }

    fn tok(&mut self, kind: SyntaxKind, start: usize, end: usize) {
        if start < end {
            self.toks.push(Tok {
                kind,
                start: start as u32,
                end: end as u32,
            });
        }
    }

    fn open(&mut self, kind: SyntaxKind, start: usize) -> usize {
        self.nodes.push(Node {
            start: start as u32,
            end: start as u32,
            kind,
        });
        self.nodes.len() - 1
    }

    /// Ends node `i` after the last token.
    fn close(&mut self, i: usize) {
        let end = self.toks.last().map_or(0, |t| t.end);
        let n = &mut self.nodes[i];
        n.end = end.max(n.start);
    }

    /// A node of one token (`Chunk` of a `Text`).
    fn leaf(&mut self, node: SyntaxKind, kind: SyntaxKind, start: usize, end: usize) {
        let n = self.open(node, start);
        self.tok(kind, start, end);
        self.close(n);
    }

    fn name_end(&self, mut i: usize) -> usize {
        while i < self.b.len()
            && !is_ws(self.b[i])
            && !matches!(self.b[i], b'>' | b'/' | b'<')
            && !(self.templates && self.action_end(i).is_some())
        {
            i += 1;
        }
        i
    }

    fn skip_ws(&self, mut i: usize) -> usize {
        while i < self.b.len() && is_ws(self.b[i]) {
            i += 1;
        }
        i
    }

    fn name_eq(&self, range: (u32, u32), at: usize, end: usize) -> bool {
        let open = &self.src[range.0 as usize..range.1 as usize];
        let name = &self.src[at..end];
        if self.mode == Mode::Angular {
            open == name
        } else {
            open.eq_ignore_ascii_case(name)
        }
    }

    fn run(&mut self) {
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            match c {
                _ if self.pos < self.cdata => {
                    if self.src[self.pos..].starts_with("{{") && self.interpolation() {
                    } else {
                        self.text_from(1);
                    }
                }
                b'<' if self.mode == Mode::Angular
                    && self.src[self.pos..].starts_with("<![CDATA[") =>
                {
                    self.cdata = self.src[self.pos..]
                        .find("]]>")
                        .map_or(self.b.len(), |n| self.pos + n + 3);
                    self.text_from(9);
                }
                b'<' if self.markup() => {}
                b'<' => self.text(true),
                b'&' if self.entities() => match self.entity(self.pos) {
                    Some(end) => {
                        self.leaf(k::CHUNK, k::ENTITY, self.pos, end);
                        self.pos = end;
                    }
                    None => self.text(true),
                },
                b'{' if self.mode == Mode::Vue || self.templates => {
                    if self.at(self.pos + 1) == b'{' && self.interpolation() {
                    } else {
                        self.text(true);
                    }
                }
                b'{' if self.mode == Mode::Svelte => self.svelte_brace(),
                b'{' if self.mode == Mode::Angular => self.ng_brace(),
                b'}' if self.mode == Mode::Angular => self.ng_close(),
                b'@' if self.mode == Mode::Angular && self.ng_at() => {}
                b'}' if self.mode == Mode::Svelte => {
                    self.error(self.pos, self.pos + 1, "unexpected }");
                    self.text(true);
                }
                _ if is_ws(c) => self.pos += 1,
                _ => self.text(false),
            }
        }
        if self.templates {
            self.template_blocks_closed();
        }
        while let Some((open, node)) = self.stack.pop() {
            if !matches!(open, Open::Element(..)) {
                let at = self.nodes[node].start as usize;
                self.error(at, at + 1, "block is not closed");
            }
            self.close(node);
        }
    }

    /// `&name;`, `&#10;`, `&#x1F;` (the `;` is optional): the end, if there is one.
    /// Entities are tokens of their own in HTML and Svelte; Vue and Angular keep them in the text.
    fn entities(&self) -> bool {
        matches!(self.mode, Mode::Html | Mode::Svelte)
    }

    /// A raw `<`, `>` or `&` in text is an error (HTML and Svelte).
    fn strict(&self) -> bool {
        self.entities()
    }

    fn entity(&self, at: usize) -> Option<usize> {
        let mut i = at + 1;
        let (numeric, hex) = (self.at(i) == b'#', matches!(self.at(i + 1), b'x' | b'X'));
        let (digits, max) = if numeric {
            i += 1 + usize::from(hex);
            (
                if hex {
                    u8::is_ascii_hexdigit as fn(&u8) -> bool
                } else {
                    u8::is_ascii_digit
                },
                if hex { 6 } else { 5 },
            )
        } else {
            (u8::is_ascii_alphabetic as fn(&u8) -> bool, 30)
        };
        let from = i;
        while i < self.b.len() && digits(&self.b[i]) {
            i += 1;
        }
        if i == from || i - from > max {
            return None;
        }
        Some(i + usize::from(self.at(i) == b';'))
    }

    /// Text up to the next markup, trimmed of blanks. `first`: the character at `pos` ends no
    /// text (a lone `<`, `&`, `>` or `}`), so it is in this one.
    fn text(&mut self, first: bool) {
        self.text_from(usize::from(first));
    }

    /// As [`text`](Self::text); the first `lead` characters end no text.
    fn text_from(&mut self, lead: usize) {
        let start = self.pos;
        let mut i = start;
        // Angular reads a CDATA section as text of its own, tags inside it included, but for `{{ }}`.
        let limit = if self.pos < self.cdata {
            self.cdata
        } else {
            self.b.len()
        };
        while i < limit {
            let lead = i < start + lead;
            if limit == self.cdata {
                if !lead && self.src[i..].starts_with("{{") {
                    break;
                }
                i += 1;
                continue;
            }
            match self.b[i] {
                b'<' if !lead && self.tag_start(i) => break,
                b'&' if !lead && self.entities() && self.entity(i).is_some() => break,
                b'<' | b'>' | b'&' if self.strict() => {
                    self.error(i, i + 1, "raw character in text");
                }
                b'{' | b'}' if !lead && self.mode == Mode::Svelte => break,
                b'{' if !lead
                    && (self.mode == Mode::Vue || self.templates)
                    && self.at(i + 1) == b'{' =>
                {
                    break;
                }
                b'{' | b'}' if !lead && self.mode == Mode::Angular => break,
                b'@' if !lead && self.mode == Mode::Angular && self.ng_candidate(i) => break,
                _ => {}
            }
            i += 1;
        }
        if i >= self.cdata {
            self.cdata = 0;
        }
        let mut end = i;
        while end > start && is_ws(self.b[end - 1]) {
            end -= 1;
        }
        self.leaf(k::CHUNK, k::TEXT, start, end);
        self.pos = i;
    }

    /// `<` at `i` opens a tag, an end tag, a comment or a declaration.
    fn tag_start(&self, i: usize) -> bool {
        let c = self.at(i + 1);
        c.is_ascii_alphabetic()
            || c == b'!'
            || c == b'?'
            || (c == b'/' && self.at(i + 2).is_ascii_alphabetic())
    }

    /// Markup at `pos`; false when the `<` is text.
    fn markup(&mut self) -> bool {
        if !self.tag_start(self.pos) {
            return false;
        }
        let rest = &self.src[self.pos..];
        if self.at(self.pos + 1).is_ascii_alphabetic() {
            self.start_tag();
        } else if self.at(self.pos + 1) == b'/' {
            self.end_tag();
        } else if rest.starts_with("<!--") {
            self.skip_comment(rest.find("-->").map(|i| i + 3), "comment is not closed");
        } else if rest.len() >= 9 && rest[..9].eq_ignore_ascii_case("<!doctype") {
            self.doctype();
        } else if self.mode == Mode::Html && rest.starts_with("<![CDATA[") {
            match rest.find("]]>") {
                Some(i) => {
                    self.leaf(k::CHUNK, k::C_DATA, self.pos, self.pos + i + 3);
                    self.pos += i + 3;
                }
                None => self.skip_comment(None, "CDATA section is not closed"),
            }
        } else {
            self.skip_comment(rest.find('>').map(|i| i + 1), "declaration is not closed");
        }
        true
    }

    /// A comment is trivia: no token, the gap between tokens holds it.
    fn skip_comment(&mut self, len: Option<usize>, message: &str) {
        match len {
            Some(n) => self.pos += n,
            None => {
                self.error(self.pos, self.b.len(), message);
                self.pos = self.b.len();
            }
        }
    }

    fn doctype(&mut self) {
        let start = self.pos;
        self.page = true;
        let n = self.open(k::DOCTYPE, start);
        self.tok(k::LT_BANG, start, start + 2);
        self.tok(k::DOCTYPE_KW, start + 2, start + 9);
        let body = if self.mode == Mode::Angular {
            self.skip_ws(start + 9)
        } else {
            start + 9
        };
        let end = self.src[body..]
            .find('>')
            .map_or(self.b.len(), |i| body + i);
        self.tok(k::DOCTYPE_BODY, body, end);
        if end < self.b.len() {
            self.tok(k::GT, end, end + 1);
            self.pos = end + 1;
        } else {
            self.error(start, end, "doctype is not closed");
            self.pos = end;
        }
        self.close(n);
    }

    fn start_tag(&mut self) {
        let start = self.pos;
        let name_end = self.name_end(start + 1);
        let src = self.src;
        let lower = lowercase(&src[start + 1..name_end]);
        // The start tag closes what it cannot be inside.
        while let Some(&(Open::Element(s, e), node)) = self.stack.last() {
            let parent = lowercase(&src[s as usize..e as usize]);
            if !tags::closes(&parent, &lower) {
                break;
            }
            self.stack.pop();
            self.close(node);
        }
        let element = self.open(k::ELEMENT, start);
        let tag = self.open(k::START_TAG, start);
        self.tok(k::LT, start, start + 1);
        self.tok(k::TAG_NAME, start + 1, name_end);
        self.pos = name_end;
        let self_closing = self.attributes();
        self.close(tag);
        self.page |= lower == "html";
        let root = self.stack.is_empty() && !self.page;
        let nested_too_deep = self.stack.len() >= MAX_DEPTH;
        if nested_too_deep && !self.deep {
            self.deep = true;
            self.error(start, name_end, "elements are nested too deeply");
        }
        let raw = match self.mode {
            Mode::Html => matches!(&*lower, "script" | "style" | "textarea" | "title"),
            _ => matches!(&*lower, "script" | "style"),
        } || (self.mode == Mode::Vue && root && lower != "template");
        if self_closing
            || nested_too_deep
            || (tags::is_void(&lower)
                && !raw
                && (self.mode != Mode::Svelte && self.mode != Mode::Vue
                    || self.src[start + 1..name_end] == lower))
        {
            if self_closing && raw && matches!(&*lower, "script" | "style") {
                self.error(
                    start,
                    self.pos,
                    "a script or style element cannot be self-closing",
                );
            }
            self.close(element);
            // (only a void element owns a stray end tag: `<br></br>`, not `<slot /></slot>`)
            self.last_void = tags::is_void(&lower).then_some((element, self.toks.len(), lower));
            return;
        }
        self.stack
            .push((Open::Element(start as u32 + 1, name_end as u32), element));
        if raw {
            let custom = self.mode == Mode::Vue && root && !matches!(&*lower, "script" | "style");
            self.raw_text(&lower, custom);
        }
    }

    /// The content of `script`, `style`, `textarea` and `title` up to their end tag.
    fn raw_text(&mut self, name: &str, custom: bool) {
        let start = self.pos;
        let mut end = self.b.len();
        let mut from = start;
        while let Some(i) = self.src[from..].find("</") {
            let at = from + i;
            let name_end = at + 2 + name.len();
            if self
                .src
                .get(at + 2..name_end)
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
                && (is_ws(self.at(name_end))
                    || matches!(self.at(name_end), b'>' | b'/')
                    || name_end >= self.b.len())
            {
                end = at;
                break;
            }
            from = at + 2;
        }
        if end == self.b.len() {
            self.error(start.saturating_sub(1), start, "element is not closed");
        }
        if !self.b[start..end].iter().all(|&c| is_ws(c)) {
            // A custom block of a Vue file is text between its blanks, as in any other element; Angular
            // keeps raw text between them too.
            let (mut s, mut e) = (start, end);
            if custom || self.mode == Mode::Angular {
                while is_ws(self.b[s]) {
                    s += 1;
                }
                while is_ws(self.b[e - 1]) {
                    e -= 1;
                }
            }
            self.leaf(k::CHUNK, if custom { k::TEXT } else { k::RAW_TEXT }, s, e);
        }
        self.pos = end;
    }

    fn end_tag(&mut self) {
        let start = self.pos;
        let name = start + 2;
        let name_end = self.name_end(name);
        let found = self
            .stack
            .iter()
            .rposition(
                |&(o, _)| matches!(o, Open::Element(s, e) if self.name_eq((s, e), name, name_end)),
            )
            .filter(|&i| {
                self.stack[i..]
                    .iter()
                    .all(|&(o, _)| matches!(o, Open::Element(..)))
            });
        // `<br></br>`: the end tag right after a void element is its own.
        let void_own = match &self.last_void {
            Some((el, len, v))
                if *len == self.toks.len() && self.src[name..name_end].eq_ignore_ascii_case(v) =>
            {
                Some(*el)
            }
            _ => None,
        };
        let kind = if found.is_some() || void_own.is_some() {
            k::END_TAG
        } else {
            k::STRAY_END_TAG
        };
        if let Some(i) = found {
            while self.stack.len() > i + 1 {
                let (_, node) = self.stack.pop().unwrap();
                self.close(node);
            }
        }
        let n = self.open(kind, start);
        self.tok(k::LT_SLASH, start, start + 2);
        self.tok(k::TAG_NAME, name, name_end);
        let gt = self.skip_ws(name_end);
        // Anything between the name and the `>` is not tokenized: the replay reports it.
        let close = self.src[gt..].find('>').map(|i| gt + i);
        match close {
            Some(c) => {
                self.tok(k::GT, c, c + 1);
                self.pos = c + 1;
            }
            None => {
                self.error(start, name_end, "end tag is not closed");
                self.pos = name_end;
            }
        }
        self.close(n);
        if found.is_none() && void_own.is_none() {
            self.error(start, self.pos, "unexpected end tag");
        }
        // (a self-closed Angular element is complete: its end tag stands beside it)
        if let Some(el) = void_own.filter(|_| {
            self.mode != Mode::Angular || tags::is_void(&lowercase(&self.src[name..name_end]))
        }) {
            self.close(el);
        }
        if found.is_some() {
            let (_, node) = self.stack.pop().unwrap();
            self.close(node);
        }
    }

    /// The attributes and the end of a start tag; true for `/>`.
    fn attributes(&mut self) -> bool {
        loop {
            self.pos = self.skip_ws(self.pos);
            let c = self.at(self.pos);
            match c {
                b'>' => {
                    self.tok(k::GT, self.pos, self.pos + 1);
                    self.pos += 1;
                    return false;
                }
                b'/' if self.mode == Mode::Angular
                    && matches!(self.at(self.pos + 1), b'/' | b'*') =>
                {
                    // A start-tag comment, up to the line break or through `*/`: not part of the tree
                    // (the replay reports it as text).
                    match ng_comment_len(&self.src[self.pos..]) {
                        Some(n) => self.pos += n,
                        None => {
                            self.error(self.pos, self.b.len(), "start tag is not closed");
                            self.pos = self.b.len();
                            return false;
                        }
                    }
                }
                b'/' if self.at(self.pos + 1) == b'>' => {
                    self.tok(k::SLASH_GT, self.pos, self.pos + 2);
                    self.pos += 2;
                    return true;
                }
                _ if self.pos >= self.b.len() || c == b'<' => {
                    self.error(
                        self.pos.saturating_sub(1),
                        self.pos,
                        "start tag is not closed",
                    );
                    return false;
                }
                b'/' | b'"' | b'\'' | b'=' => {
                    // Not tokenized: the replay reports it.
                    self.pos += 1;
                }
                _ => self.attribute(),
            }
        }
    }

    fn attribute(&mut self) {
        let start = self.pos;
        if let Some(end) = self.action_end(start) {
            // An action among the attributes is one attribute, kept as written.
            let n = self.open(k::ATTRIBUTE, start);
            self.template_action(start, end - 2);
            self.tok(k::ATTR_NAME, start, end);
            self.pos = end;
            self.close(n);
            return;
        }
        let n = self.open(k::ATTRIBUTE, start);
        if self.mode == Mode::Svelte && self.at(start) == b'{' {
            let end = self.brace_end(start).unwrap_or(self.b.len());
            self.tok(k::ATTR_NAME, start, end);
            self.pos = end;
        } else {
            let mut end = start;
            // Angular: `[class.[&>svg]:w-4]` runs while its brackets are open.
            let mut depth = 0usize;
            while end < self.b.len() {
                let c = self.b[end];
                if self.mode == Mode::Angular && c == b'[' {
                    depth += 1;
                } else if self.mode == Mode::Angular && c == b']' {
                    depth = depth.saturating_sub(1);
                }
                let stop = if depth > 0 {
                    matches!(c, b'\n' | b'\r')
                } else {
                    is_ws(c)
                        || matches!(c, b'<' | b'>' | b'"' | b'\'' | b'/' | b'=')
                        || (self.templates && self.action_end(end).is_some())
                        || (self.mode == Mode::Svelte && matches!(c, b'{' | b'}'))
                };
                if stop {
                    break;
                }
                end += 1;
            }
            self.attribute_name(start, end);
            self.pos = end;
            let eq = self.skip_ws(end);
            if self.at(eq) == b'=' {
                self.tok(k::EQUALS, eq, eq + 1);
                self.pos = eq + 1;
                self.attribute_value();
            }
        }
        self.close(n);
    }

    fn attribute_name(&mut self, start: usize, end: usize) {
        let name = &self.src[start..end];
        let directive = self.mode == Mode::Vue
            && (name.starts_with("v-") || name.starts_with([':', '.', '@', '#']));
        if !directive {
            return self.tok(k::ATTR_NAME, start, end);
        }
        // `v-on:click.once`, `@click.once`, `:[key]`: a token for each part and each `:` or `.`.
        let mut i = start;
        while i < end {
            let c = self.b[i];
            let stop = if matches!(c, b':' | b'.') || (i == start && matches!(c, b'@' | b'#')) {
                i + 1
            } else if c == b'[' {
                self.src[i..end].find(']').map_or(end, |j| i + j + 1)
            } else {
                let mut j = i + 1;
                while j < end && !matches!(self.b[j], b':' | b'.') {
                    j += 1;
                }
                j
            };
            self.tok(k::ATTR_NAME, i, stop);
            i = stop;
        }
    }

    fn attribute_value(&mut self) {
        let at = self.skip_ws(self.pos);
        let c = self.at(at);
        if c == b'"' || c == b'\'' {
            let end = match self.value_end(at + 1, c) {
                Some(end) => end,
                None => {
                    self.error(at, at + 1, "attribute value is not closed");
                    self.b.len()
                }
            };
            self.tok(k::ATTR_VALUE, at, end);
            self.pos = end;
        } else if c == b'{' && self.mode == Mode::Svelte {
            let end = self.brace_end(at).unwrap_or(self.b.len());
            self.mustache(at, end);
            self.pos = end;
        } else {
            let mut end = at;
            while end < self.b.len()
                && !is_ws(self.b[end])
                && !matches!(self.b[end], b'<' | b'>' | b'"' | b'\'' | b'=' | b'`')
                && !(self.mode == Mode::Angular
                    && self.b[end] == b'/'
                    && matches!(self.at(end + 1), b'/' | b'*' | b'>'))
            {
                end += 1;
            }
            self.tok(k::ATTR_VALUE, at, end);
            self.pos = end;
        }
    }

    /// `{{ expression }}` of Vue.
    fn interpolation(&mut self) -> bool {
        let start = self.pos;
        let found = match self.mode {
            Mode::Angular => self.ng_interpolation_end(start + 2),
            _ => self.src[start + 2..].find("}}").map(|i| start + 2 + i),
        };
        let Some(end) = found else {
            return false;
        };
        let inner = start + 2;
        if self.templates {
            self.template_action(start, end);
        }
        let n = self.open(k::INTERPOLATION, start);
        self.tok(k::L_BRACE_BRACE, start, inner);
        if !self.b[inner..end].is_empty() {
            let kind = if self.mode == Mode::Angular {
                k::TEXT
            } else {
                k::RAW_TEXT
            };
            self.leaf(k::CHUNK, kind, inner, end);
        }
        self.tok(k::R_BRACE_BRACE, end, end + 2);
        self.close(n);
        self.pos = end + 2;
        true
    }
}
