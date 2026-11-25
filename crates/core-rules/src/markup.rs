//! The elements of a markup file as a flat tree: name, attributes, parent, children and whether
//! text stands directly in an element. Rules over the structure of a document (what an element may
//! contain, what a link or a form holds) read this instead of walking syntax nodes. An element is a
//! node whose first child node is a `#start-tag` with the slot `name` and the attribute nodes in
//! `entries` (each a `#key-value` with `key` and `value`); `#content` nodes that are not elements
//! (interpolations, blocks) count as text, and the elements inside them (`@if (x) { ... }`) belong
//! to the element around them; other nodes are looked through.
//!
//! An attribute that a template dialect writes as a binding (`[alt]`, `:alt`, `(click)`, `@click`)
//! is read by the dialect's [`Spellings`]: its `base` is the name without the binding syntax and
//! `bind` says whether it sets a property or listens to an event, so a check asks for `alt` or for
//! the event `click` once, whatever the dialect writes.
//!
//! concept: markup-tree
//! what: The elements of a markup document with names, attributes, parents and children.
//! tools: html-validate element-permitted-content, wcag/h30, hidden-focusable, angular-eslint, eslint-plugin-vue
//! reads: tag:start-tag, tag:key-value, tag:chunk, tag:content

use core_syntax::{Grammar, SyntaxNode, TextRange, TextSize};
use std::path::Path;

/// What a binding does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bind {
    Prop,
    Event,
}

#[derive(Debug)]
struct Row {
    /// The file-name suffixes of the dialect (`component.html`, `vue`, `html`).
    dialects: Vec<String>,
    bind: Bind,
    /// Characters at which the name ends (modifiers: `.` in Vue).
    strip: String,
    /// `(prefix, suffix)` around the name.
    forms: Vec<(String, String)>,
}

/// How the dialects write bound attributes, from rows `<dialect> <prop|event> [strip=<chars>]
/// <prefix>%<suffix>...`, where `%` stands for the name.
#[derive(Debug, Default)]
pub struct Spellings {
    rows: Vec<Row>,
}

impl Spellings {
    pub fn parse(text: &str) -> Spellings {
        let mut rows = Vec::new();
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            let mut words = line.split_whitespace();
            let (Some(dialect), Some(kind)) = (words.next(), words.next()) else {
                continue;
            };
            let bind = match kind {
                "prop" => Bind::Prop,
                "event" => Bind::Event,
                _ => continue, // a row of the content model
            };
            let mut strip = String::new();
            let mut forms = Vec::new();
            for w in words {
                if let Some(chars) = w.strip_prefix("strip=") {
                    strip = chars.to_string();
                } else if let Some((a, b)) = w.split_once('%') {
                    forms.push((a.to_string(), b.to_string()));
                }
            }
            rows.push(Row {
                dialects: dialect.split(',').map(str::to_string).collect(),
                bind,
                strip,
                forms,
            });
        }
        Spellings { rows }
    }

    /// The rows of the dialect of `path`: those of the longest suffix that ends the file name
    /// (`a.component.html` is Angular, `a.html` is HTML).
    pub fn of(&self, path: &Path) -> Dialect<'_> {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let suffix = |r: &Row| {
            r.dialects
                .iter()
                .filter(|d| {
                    name.strip_suffix(d.as_str())
                        .is_some_and(|rest| rest.ends_with('.'))
                })
                .map(String::len)
                .max()
        };
        let longest = self.rows.iter().filter_map(suffix).max();
        Dialect(
            self.rows
                .iter()
                .filter(|r| longest.is_some() && suffix(r) == longest)
                .collect(),
        )
    }
}

/// The spellings of one file's dialect.
#[derive(Debug, Default)]
pub struct Dialect<'a>(Vec<&'a Row>);

impl Dialect<'_> {
    /// The binding `written` is and the name it binds: the longest matching prefix wins.
    pub fn read<'t>(&self, written: &'t str) -> Option<(Bind, &'t str)> {
        let mut best: Option<(usize, Bind, &'t str)> = None;
        for row in &self.0 {
            for (prefix, suffix) in &row.forms {
                let Some(inner) = written
                    .strip_prefix(prefix.as_str())
                    .and_then(|r| r.strip_suffix(suffix.as_str()))
                else {
                    continue;
                };
                let end = inner.find(|c| row.strip.contains(c)).unwrap_or(inner.len());
                let name = &inner[..end];
                if name.is_empty()
                    || name.starts_with(['[', '('])
                    || best.is_some_and(|(len, ..)| len >= prefix.len())
                {
                    continue;
                }
                best = Some((prefix.len(), row.bind, name));
            }
        }
        best.map(|(_, bind, name)| (bind, name))
    }
}

/// An attribute as written.
#[derive(Clone, Debug)]
pub struct Attr {
    pub name: String,
    /// The name without the binding syntax (the same as `name` for a plain attribute).
    pub base: String,
    /// Set when the dialect writes the attribute as a binding.
    pub bind: Option<Bind>,
    /// Without quotes; `None` for an attribute without `=`. A binding's value is code.
    pub value: Option<String>,
    pub key: TextRange,
    pub value_range: Option<TextRange>,
}

#[derive(Clone, Debug)]
pub struct Element {
    pub name: String,
    pub name_range: TextRange,
    /// The start tag.
    pub range: TextRange,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub attrs: Vec<Attr>,
    /// Text that is not blank (or an interpolation) stands directly in the element.
    pub text: bool,
    /// The element has an end tag.
    pub end_tag: bool,
    /// The start tag ends in `/>`.
    pub empty: bool,
    /// Not written: added by a content model that opens elements implicitly.
    pub implied: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Markup {
    pub elements: Vec<Element>,
    /// Elements that have no parent element.
    pub roots: Vec<usize>,
}

/// What reading a file needs besides the tree: its text, whether names fold to lowercase (HTML)
/// and its dialect's bindings.
struct Reading<'a> {
    grammar: &'a Grammar,
    text: &'a str,
    fold: bool,
    dialect: &'a Dialect<'a>,
}

impl Markup {
    /// The elements of `root` in document order. `fold` lowercases names (HTML).
    pub fn build(
        root: &SyntaxNode,
        grammar: &Grammar,
        text: &str,
        fold: bool,
        dialect: &Dialect<'_>,
    ) -> Markup {
        let mut markup = Markup::default();
        let reading = Reading {
            grammar,
            text,
            fold,
            dialect,
        };
        markup.content(root, None, &reading);
        markup
    }

    /// The children of `node` (an element or the root): elements are added; other `#content`
    /// nodes and non-blank text tokens make `parent` hold text.
    fn content(&mut self, node: &SyntaxNode, parent: Option<usize>, r: &Reading<'_>) {
        for child in node.children_with_tokens() {
            match child {
                rowan::NodeOrToken::Token(t) => {
                    if let Some(p) = parent
                        && r.grammar.has_tag(t.kind(), "chunk")
                        && !r.text[t.text_range()].trim().is_empty()
                    {
                        self.elements[p].text = true;
                    }
                }
                rowan::NodeOrToken::Node(n) => {
                    if let Some(index) = self.element(&n, parent, r) {
                        self.content(&n, Some(index), r);
                        self.elements[index].end_tag = r
                            .grammar
                            .slot(&n, "end")
                            .is_some_and(|e| e.as_node().is_some());
                    } else if r.grammar.has_tag(n.kind(), "content") {
                        if let Some(p) = parent {
                            self.elements[p].text = true;
                        }
                        self.content(&n, parent, r);
                    } else {
                        self.content(&n, parent, r);
                    }
                }
            }
        }
    }

    /// Adds the element `node` is, when it is one.
    fn element(
        &mut self,
        node: &SyntaxNode,
        parent: Option<usize>,
        r: &Reading<'_>,
    ) -> Option<usize> {
        let start = node.first_child()?;
        if !r.grammar.has_tag(start.kind(), "start-tag") {
            return None;
        }
        let name_range = r.grammar.slot(&start, "name")?.text_range();
        let raw = &r.text[name_range];
        let attrs = r
            .grammar
            .slots(&start, "entries")
            .filter_map(|e| e.into_node())
            .filter_map(|a| attribute(&a, r))
            .collect();
        // The node may start with trivia: the tag starts at the `<` before its name.
        let from = name_range.start() - TextSize::from(1);
        let tag = r.text[TextRange::new(from, start.text_range().end())].trim_end();
        let index = self.elements.len();
        self.elements.push(Element {
            name: if r.fold {
                raw.to_ascii_lowercase()
            } else {
                raw.to_string()
            },
            name_range,
            range: TextRange::at(from, TextSize::of(tag)),
            parent,
            children: Vec::new(),
            attrs,
            text: false,
            end_tag: false,
            empty: tag.ends_with("/>"),
            implied: false,
        });
        match parent {
            Some(p) => self.elements[p].children.push(index),
            None => self.roots.push(index),
        }
        Some(index)
    }

    /// The attribute `name`, plain or written as a property binding (`alt`, `[alt]`, `:alt`).
    pub fn attr(&self, element: usize, name: &str) -> Option<&Attr> {
        self.elements[element]
            .attrs
            .iter()
            .find(|a| a.bind != Some(Bind::Event) && a.base == name)
    }

    /// The value of the plain attribute `name`: a binding's value is code, not text.
    pub fn plain(&self, element: usize, name: &str) -> Option<&str> {
        self.attr(element, name)
            .filter(|a| a.bind.is_none())
            .and_then(|a| a.value.as_deref())
    }

    /// The listeners of the element: `(name, attribute)`, a name as the dialect writes it
    /// (`keydown.enter`).
    pub fn events(&self, element: usize) -> impl Iterator<Item = (&str, &Attr)> {
        self.elements[element]
            .attrs
            .iter()
            .filter(|a| a.bind == Some(Bind::Event))
            .map(|a| (a.base.as_str(), a))
    }

    /// The elements below `element`, in document order.
    pub fn descendants(&self, element: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = self.elements[element]
            .children
            .iter()
            .rev()
            .copied()
            .collect();
        while let Some(i) = stack.pop() {
            out.push(i);
            stack.extend(self.elements[i].children.iter().rev());
        }
        out
    }

    /// `element` and its ancestors, nearest first.
    pub fn ancestors_and_self(&self, element: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(element), |&i| self.elements[i].parent)
    }
}

fn attribute(node: &SyntaxNode, r: &Reading<'_>) -> Option<Attr> {
    // A dialect may write the name in several tokens (`v-on:click.once`): it runs to the `=`.
    let first = r.grammar.slot(node, "key")?.text_range();
    let rest = &r.text[TextRange::new(first.start(), node.text_range().end())];
    let len = rest
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(rest.len());
    let key = TextRange::at(first.start(), TextSize::of(&rest[..len]));
    let value = r.grammar.slot(node, "value").map(|v| {
        let range = v.text_range();
        let raw = &r.text[range];
        let quoted = raw.len() >= 2
            && ((raw.starts_with('"') && raw.ends_with('"'))
                || (raw.starts_with('\'') && raw.ends_with('\'')));
        if quoted {
            let inner = TextRange::new(
                range.start() + TextSize::from(1),
                range.end() - TextSize::from(1),
            );
            (r.text[inner].to_string(), inner)
        } else {
            (raw.to_string(), range)
        }
    });
    let written = &r.text[key];
    let name = if r.fold {
        written.to_ascii_lowercase()
    } else {
        written.to_string()
    };
    let bound = r.dialect.read(&name);
    Some(Attr {
        base: bound.map_or_else(|| name.clone(), |(_, n)| n.to_string()),
        bind: bound.map(|(b, _)| b),
        name,
        value: value.as_ref().map(|v| v.0.clone()),
        key,
        value_range: value.map(|v| v.1),
    })
}
