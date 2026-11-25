//! The `template` family: checks of Vue, Svelte and Angular template syntax on attributes, tags and
//! mustaches. They are Rust
//! rather than pattern files because a pattern rule compiles a regex that costs more than all
//! of these rules together (the lint bench of `.vue` and `.svelte` inputs).
//!
//! Param `check` (default `directive`; the others are named in `build`) and, for every check,
//! `only` (`vue`, `svelte` or `component.html`: the rule runs on files with that extension or
//! file-name suffix only; in HTML, `slot`, `is` and `<template>` are standard). `directive` reads the attributes called `name` (`v-if`, `v-bind`, ...; `:x`, `@x`
//! and `#x` are `v-bind`, `v-on` and `v-slot`, a plain attribute is `slot`) and reports one
//! when any of its conditions holds:
//! - `value`: `required`, `forbidden`, `unless-arg` / `unless-modifier` (required when there is
//!   no argument / no modifier), `iteration` (`item in items`), `vue-prefixed` (`vue:name`);
//! - `no-arg`; `modifiers` (`none`, or the comma-separated ones allowed); `banned` (modifiers
//!   that are reported, comma-separated); `numeric-modifier`;
//! - `with`: the element also has one of these directives; `after`: the previous element does
//!   not have one of them (`v-else` after `v-if`);
//! - `bad-host`: `html` (a plain lowercase element: not `<template>`, `<component>` or a
//!   component), `file-input` (an `<input type="file">`);
//! - `host`: the rule only looks at this element, `template`, `html`, `not-slot` or `checkable`
//!   (an `<input>` that is a checkbox or a radio button).
//!
//! The other checks take `prefix` (`duplicate`: the attributes starting with it must not repeat;
//! `strip-modifiers` compares the names without `|modifiers`) and `directives` (`svelte-shorthand`:
//! the directives, comma-separated, whose `={name}` is redundant; none: plain attributes).
//!
//! concept: new template-syntax-checks
//! what: Checks of Vue, Svelte and Angular template syntax on attributes, tags and mustaches.
//! tools: eslint-plugin-vue, eslint-plugin-svelte, angular-eslint

use crate::generated::kind as k;
use crate::ng_checks;
mod vue;
use core_rules::{Edit, Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxNodeExt, TextRange, TextSize};

pub const FAMILY: Family = Family {
    name: "template",
    build,
};

#[derive(Clone, Copy, PartialEq)]
enum Value {
    Any,
    Required,
    Forbidden,
    UnlessArg,
    UnlessModifier,
    Iteration,
    VuePrefixed,
}

#[derive(Clone, Copy, PartialEq)]
enum Host {
    Template,
    Html,
    NotSlot,
    FileInput,
    Checkable,
}

enum Modifiers {
    Any,
    None,
    Only(Vec<String>),
}

struct Directive {
    name: String,
    /// The starts of the attribute texts that can be this directive.
    prefixes: Vec<String>,
    value: Value,
    no_arg: bool,
    modifiers: Modifiers,
    banned: Vec<String>,
    numeric: bool,
    with: Vec<String>,
    after: Vec<String>,
    host: Option<Host>,
    bad_host: Option<Host>,
    /// No condition at all: the attribute is reported wherever it is (a deprecated one).
    unconditional: bool,
}

enum Check {
    Directive(Box<Directive>),
    Shorthand {
        from: String,
        to: String,
    },
    /// `on`: the arguments of `v-on` (the events), else attributes and `v-bind` arguments.
    Hyphenation {
        on: bool,
    },
    KeyOnChild,
    ChildContent,
    LoneTemplate,
    TextareaMustache,
    ComponentIs,
    Duplicate {
        prefix: String,
        strip: bool,
        same_value: bool,
    },
    RawSpecial,
    ChildrenSnippet,
    UselessMustache,
    ObjectMustache,
    SvelteShorthand {
        directives: Vec<String>,
    },
    SpacedComment,
    StyleProperty,
    TargetBlank,
    NestedStyle,
    BananaInBox,
    LooseEquality,
    NegatedAsync,
    ForTrack,
    ForKey,
    SelfClosing,
    AttributesOrder,
    DupeElseIf,
    ComponentHtml,
}

struct Template {
    meta: &'static RuleMeta,
    check: Check,
    message: String,
    only: Option<String>,
}

fn list(text: Option<String>) -> Vec<String> {
    text.map_or_else(Vec::new, |t| {
        t.split(',').map(|s| s.trim().to_string()).collect()
    })
}

fn host(p: &mut Params, key: &str) -> Result<Option<Host>, String> {
    let Some(text) = p.optional(key) else {
        return Ok(None);
    };
    match text.as_str() {
        "template" => Ok(Some(Host::Template)),
        "html" => Ok(Some(Host::Html)),
        "not-slot" => Ok(Some(Host::NotSlot)),
        "file-input" => Ok(Some(Host::FileInput)),
        "checkable" => Ok(Some(Host::Checkable)),
        _ => Err(format!("param `{key}`: `{text}` is not a host")),
    }
}

impl Directive {
    fn build(p: &mut Params) -> Result<Directive, String> {
        let name = p.text("name")?;
        let prefixes = match name.as_str() {
            "v-bind" => vec!["v-bind".into(), ":".into()],
            "v-on" => vec!["v-on".into(), "@".into()],
            "v-slot" => vec!["v-slot".into(), "#".into()],
            _ => vec![name.clone()],
        };
        let value = p.choice(
            "value",
            false,
            &[
                ("any", Value::Any),
                ("required", Value::Required),
                ("forbidden", Value::Forbidden),
                ("unless-arg", Value::UnlessArg),
                ("unless-modifier", Value::UnlessModifier),
                ("iteration", Value::Iteration),
                ("vue-prefixed", Value::VuePrefixed),
            ],
        )?;
        let modifiers = match p.optional("modifiers").as_deref() {
            None => Modifiers::Any,
            Some("none") => Modifiers::None,
            Some(list) => Modifiers::Only(list.split(',').map(|s| s.trim().to_string()).collect()),
        };
        let mut d = Directive {
            name,
            prefixes,
            value,
            no_arg: p.flag("no-arg")?,
            modifiers,
            banned: list(p.optional("banned")),
            numeric: p.flag("numeric-modifier")?,
            with: list(p.optional("with")),
            after: list(p.optional("after")),
            host: host(p, "host")?,
            bad_host: host(p, "bad-host")?,
            unconditional: false,
        };
        d.unconditional = d.value == Value::Any
            && matches!(d.modifiers, Modifiers::Any)
            && !d.no_arg
            && d.banned.is_empty()
            && !d.numeric
            && d.with.is_empty()
            && d.after.is_empty()
            && d.bad_host.is_none();
        Ok(d)
    }
}

/// Reads the params of one `check`.
type Make = fn(&mut Params) -> Result<Check, String>;

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let make = p.choice::<Make>(
        "check",
        false,
        &[
            ("directive", |p| {
                Ok(Check::Directive(Box::new(Directive::build(p)?)))
            }),
            ("shorthand", |p| {
                Ok(Check::Shorthand {
                    from: p.text("from")?,
                    to: p.text("to")?,
                })
            }),
            ("hyphenation", |p| {
                Ok(Check::Hyphenation { on: p.flag("on")? })
            }),
            ("key-on-child", |_| Ok(Check::KeyOnChild)),
            ("child-content", |_| Ok(Check::ChildContent)),
            ("lone-template", |_| Ok(Check::LoneTemplate)),
            ("textarea-mustache", |_| Ok(Check::TextareaMustache)),
            ("component-is", |_| Ok(Check::ComponentIs)),
            ("duplicate", |p| {
                Ok(Check::Duplicate {
                    prefix: p.text("prefix")?,
                    strip: p.flag("strip-modifiers")?,
                    same_value: p.flag("same-value")?,
                })
            }),
            ("raw-special", |_| Ok(Check::RawSpecial)),
            ("children-snippet", |_| Ok(Check::ChildrenSnippet)),
            ("useless-mustache", |_| Ok(Check::UselessMustache)),
            ("object-mustache", |_| Ok(Check::ObjectMustache)),
            ("svelte-shorthand", |p| {
                Ok(Check::SvelteShorthand {
                    directives: list(p.optional("directives")),
                })
            }),
            ("spaced-comment", |_| Ok(Check::SpacedComment)),
            ("style-property", |_| Ok(Check::StyleProperty)),
            ("target-blank", |_| Ok(Check::TargetBlank)),
            ("nested-style", |_| Ok(Check::NestedStyle)),
            ("banana-in-box", |_| Ok(Check::BananaInBox)),
            ("loose-equality", |_| Ok(Check::LooseEquality)),
            ("negated-async", |_| Ok(Check::NegatedAsync)),
            ("for-track", |_| Ok(Check::ForTrack)),
            ("for-key", |_| Ok(Check::ForKey)),
            ("self-closing", |_| Ok(Check::SelfClosing)),
            ("attributes-order", |_| Ok(Check::AttributesOrder)),
            ("dupe-else-if", |_| Ok(Check::DupeElseIf)),
            ("component-html", |_| Ok(Check::ComponentHtml)),
        ],
    )?;
    let check = make(&mut p)?;
    let rule = Template {
        meta,
        check,
        message: p.text("message")?,
        only: p.optional("only"),
    };
    p.done()?;
    Ok(Box::new(rule))
}

/// The text of an attribute node without the blank space before it (a line break and the
/// indentation belong to the attribute that follows), and where it starts.
fn code<'a>(node: &SyntaxNode, text: &'a str) -> (&'a str, TextSize) {
    let range = node.text_range();
    let s = &text[range];
    let trimmed = s.trim_start();
    (
        trimmed,
        range.start() + TextSize::of(&s[..s.len() - trimmed.len()]),
    )
}

/// An attribute: its name as written, split into what Vue means by it.
struct Attr<'a> {
    start: TextSize,
    full: &'a str,
    /// `v-if`, `v-bind` (also for `:x`), ... or a plain attribute's name.
    name: &'a str,
    directive: bool,
    /// What follows the name's `:` (or the shorthand character), `arg.mod.mod`.
    rest: &'a str,
    /// The text after the name: ` = "value"` and the trivia.
    after: &'a str,
}

impl<'a> Attr<'a> {
    fn parse(node: &SyntaxNode, text: &'a str) -> Option<Attr<'a>> {
        let (s, start) = code(node, text);
        let end = s
            .find(|c: char| c.is_ascii_whitespace() || c == '=')
            .unwrap_or(s.len());
        let full = s.get(..end).filter(|f| !f.is_empty())?;
        let (name, rest, directive) = if full.starts_with("v-") {
            match full.find([':', '.']) {
                Some(i) => (
                    &full[..i],
                    full[i..].strip_prefix(':').unwrap_or(&full[i..]),
                    true,
                ),
                None => (full, "", true),
            }
        } else {
            match full.as_bytes()[0] {
                b':' => ("v-bind", &full[1..], true),
                b'@' => ("v-on", &full[1..], true),
                b'#' => ("v-slot", &full[1..], true),
                _ => (full, "", false),
            }
        };
        Some(Attr {
            start,
            full,
            name,
            directive,
            rest,
            after: &s[end..],
        })
    }

    fn range(&self) -> TextRange {
        TextRange::at(self.start, TextSize::of(self.full))
    }

    /// The argument and the modifiers.
    fn parts(&self) -> (Option<&'a str>, impl Iterator<Item = &'a str>) {
        let rest = self.rest;
        let split = if rest.starts_with('[') {
            rest.find(']').map_or(rest.len(), |i| i + 1)
        } else {
            rest.find('.').unwrap_or(rest.len())
        };
        let (arg, mods) = rest.split_at(split);
        (
            (!arg.is_empty()).then_some(arg),
            mods.split('.').filter(|m| !m.is_empty()),
        )
    }

    /// The value without its quotes; `None` without `=`.
    fn value(&self) -> Option<&'a str> {
        let v = self.after.trim_start().strip_prefix('=')?.trim_start();
        Some(match v.chars().next() {
            Some(q @ ('"' | '\'')) => {
                let inner = &v[1..];
                &inner[..inner.find(q).unwrap_or(inner.len())]
            }
            _ => &v[..v.find(|c: char| c.is_ascii_whitespace()).unwrap_or(v.len())],
        })
    }
}

fn tag_name<'a>(tag: &SyntaxNode, text: &'a str) -> Option<(&'a str, TextRange)> {
    let t = tag.token(k::TAG_NAME)?;
    Some((&text[t.text_range()], t.text_range()))
}

/// The attributes of a start tag.
fn attrs<'a>(tag: &SyntaxNode, text: &'a str) -> Vec<Attr<'a>> {
    tag.children()
        .filter(|c| c.kind() == k::ATTRIBUTE)
        .filter_map(|c| Attr::parse(&c, text))
        .collect()
}

/// Does the file name end in `.suffix` (an extension, or `component.html`)?
pub(crate) fn path_has_suffix(path: &std::path::Path, suffix: &str) -> bool {
    path.to_str().is_some_and(|p| {
        p.strip_suffix(suffix)
            .is_some_and(|rest| rest.ends_with('.'))
    })
}

fn has(attrs: &[Attr<'_>], names: &[String]) -> bool {
    attrs.iter().any(|a| names.iter().any(|n| n == a.name))
}

/// A component: a tag in PascalCase, or with a `-` or `.`.
fn is_component(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase()) || name.contains(['-', '.'])
}

fn is_host(h: Host, tag: &SyntaxNode, text: &str) -> bool {
    let Some((name, _)) = tag_name(tag, text) else {
        return false;
    };
    match h {
        Host::Template => name == "template",
        Host::NotSlot => name != "slot",
        Host::Html => !is_component(name) && !matches!(name, "template" | "component"),
        Host::Checkable => {
            name.eq_ignore_ascii_case("input")
                && attrs(tag, text).iter().any(|a| {
                    a.full == "type"
                        && a.value().is_some_and(|v| {
                            v.eq_ignore_ascii_case("checkbox") || v.eq_ignore_ascii_case("radio")
                        })
                })
        }
        Host::FileInput => {
            name.eq_ignore_ascii_case("input")
                && attrs(tag, text).iter().any(|a| {
                    a.full == "type" && a.value().is_some_and(|v| v.eq_ignore_ascii_case("file"))
                })
        }
    }
}

/// The start tag of the element before `tag`'s, when that is an element (blank space and
/// comments between them do not count).
fn previous_tag(tag: &SyntaxNode) -> Option<SyntaxNode> {
    let mut prev = tag.parent()?.prev_sibling();
    while let Some(n) = prev {
        match n.kind() {
            k::ELEMENT => return n.child(k::START_TAG),
            k::CHUNK
                if n.tokens()
                    .all(|t| matches!(t.kind(), k::WHITESPACE | k::NEWLINE | k::COMMENT)) =>
            {
                prev = n.prev_sibling();
            }
            _ => return None,
        }
    }
    None
}

impl Directive {
    fn violated(&self, a: &Attr<'_>, tag: &SyntaxNode, text: &str) -> bool {
        let (arg, mods) = a.parts();
        let mods: Vec<&str> = mods.collect();
        let value = a.value();
        let blank = value.is_some_and(|v| v.trim().is_empty());
        let empty = blank || value.is_none();
        let bad_value = match self.value {
            Value::Any => false,
            Value::Required => empty,
            Value::Forbidden => value.is_some(),
            Value::UnlessArg => blank || value.is_none() && arg.is_none(),
            Value::UnlessModifier => mods.is_empty() && (blank || value.is_none()),
            Value::Iteration => !value.is_some_and(|v| {
                let words: Vec<&str> = v.split_whitespace().collect();
                words.len() >= 3
                    && words[1..words.len() - 1]
                        .iter()
                        .any(|w| *w == "in" || *w == "of")
            }),
            Value::VuePrefixed => value.is_some_and(|v| !v.starts_with("vue:")),
        };
        let bad_modifiers = match &self.modifiers {
            Modifiers::Any => false,
            Modifiers::None => !mods.is_empty(),
            Modifiers::Only(ok) => mods.iter().any(|m| !ok.iter().any(|o| o == m)),
        } || mods.iter().any(|m| self.banned.iter().any(|b| b == m))
            || self.numeric && mods.iter().any(|m| m.bytes().all(|b| b.is_ascii_digit()));
        self.unconditional
            || bad_value
            || bad_modifiers
            || self.no_arg && arg.is_some()
            || self.bad_host.is_some_and(|h| is_host(h, tag, text))
            || !self.with.is_empty() && has(&attrs(tag, text), &self.with)
            || !self.after.is_empty()
                && !previous_tag(tag).is_some_and(|t| has(&attrs(&t, text), &self.after))
    }
}

impl Rule for Template {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        match self.check {
            Check::LoneTemplate
            | Check::TextareaMustache
            | Check::ComponentIs
            | Check::RawSpecial
            | Check::TargetBlank
            | Check::ForKey
            | Check::SelfClosing
            | Check::AttributesOrder
            | Check::NestedStyle => &[k::START_TAG],
            Check::ChildrenSnippet => &[k::BLOCK_TAG],
            Check::UselessMustache | Check::ObjectMustache => &[k::MUSTACHE],
            Check::SpacedComment => &[k::DOCUMENT],
            Check::LooseEquality | Check::NegatedAsync => {
                &[k::ATTRIBUTE, k::INTERPOLATION, k::BLOCK_HEAD]
            }
            Check::ForTrack => &[k::BLOCK_HEAD],
            _ => &[k::ATTRIBUTE],
        }
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        if let Some(only) = &self.only
            && !path_has_suffix(ctx.file.path, only)
        {
            return;
        }
        let text = ctx.file.text;
        match &self.check {
            Check::LoneTemplate => {
                let Some((name, range)) = tag_name(node, text) else {
                    return;
                };
                let nested = node
                    .parent()
                    .and_then(|e| e.parent())
                    .is_some_and(|p| p.kind() == k::ELEMENT);
                let directive = |a: &Attr<'_>| {
                    matches!(
                        a.name,
                        "v-if"
                            | "v-else"
                            | "v-else-if"
                            | "v-for"
                            | "v-slot"
                            | "v-pre"
                            | "v-is"
                            | "slot"
                            | "slot-scope"
                            | "scope"
                            | "is"
                    ) || a.name.starts_with("shadowroot")
                };
                if name == "template" && nested && !attrs(node, text).iter().any(directive) {
                    ctx.report(range, self.message.clone());
                }
            }
            Check::TextareaMustache => {
                if tag_name(node, text).is_some_and(|(n, _)| n.eq_ignore_ascii_case("textarea")) {
                    for c in node.parent().into_iter().flat_map(|e| e.children()) {
                        if c.kind() == k::INTERPOLATION {
                            ctx.report(c.text_range(), self.message.clone());
                        }
                    }
                }
            }
            Check::ComponentIs => {
                let Some((name, range)) = tag_name(node, text) else {
                    return;
                };
                let is = |a: &Attr<'_>| {
                    a.name == "v-is"
                        || a.name == "is"
                        || a.name == "v-bind" && a.parts().0 == Some("is")
                };
                if name == "component" && !attrs(node, text).iter().any(is) {
                    ctx.report(range, self.message.clone());
                }
            }
            Check::ForKey | Check::SelfClosing | Check::AttributesOrder => {
                let ranges = match self.check {
                    Check::ForKey => vue::for_key(node, text, ctx.file.path),
                    Check::SelfClosing => vue::self_closing(node, text),
                    _ => vue::attributes_order(node, text),
                };
                for range in ranges {
                    ctx.report(range, self.message.clone());
                }
            }
            Check::RawSpecial => {
                if let Some((name, range)) = tag_name(node, text)
                    && matches!(
                        name,
                        "head" | "body" | "window" | "document" | "element" | "options"
                    )
                {
                    ctx.report(range, self.message.clone());
                }
            }
            Check::NestedStyle => {
                if let Some((name, range)) = tag_name(node, text)
                    && name == "style"
                    && node
                        .parent()
                        .and_then(|e| e.parent())
                        .is_some_and(|p| p.kind() != k::DOCUMENT)
                {
                    ctx.report(range, self.message.clone());
                }
            }
            Check::ChildrenSnippet => {
                let (s, start) = code(node, text);
                let empty = s
                    .strip_prefix("{#snippet")
                    .map(str::trim_start)
                    .and_then(|r| r.strip_prefix("children"))
                    .map(str::trim_start)
                    .and_then(|r| r.strip_prefix('('))
                    .is_some_and(|r| r.trim_start().starts_with(')'));
                if empty {
                    ctx.report(
                        TextRange::at(start, TextSize::of(s.trim_end())),
                        self.message.clone(),
                    );
                }
            }
            Check::UselessMustache => {
                let (s, start) = code(node, text);
                let s = s.trim_end();
                let inner = s
                    .strip_prefix('{')
                    .and_then(|r| r.strip_suffix('}'))
                    .map(str::trim);
                if let Some(inner) = inner
                    && let Some(q) = inner
                        .chars()
                        .next()
                        .filter(|c| matches!(c, '"' | '\'' | '`'))
                    && inner.len() >= 2
                    && inner.ends_with(q)
                {
                    let body = &inner[1..inner.len() - 1];
                    if !body.contains([q, '\\']) && !(q == '`' && body.contains("${")) {
                        ctx.report(TextRange::at(start, TextSize::of(s)), self.message.clone());
                    }
                }
            }
            Check::ObjectMustache => {
                let (s, start) = code(node, text);
                let s = s.trim_end();
                let inner = s.strip_prefix('{').and_then(|r| r.strip_suffix('}'));
                let inner = inner.map_or("", str::trim_start);
                let word = |w: &str| {
                    inner.strip_prefix(w).is_some_and(|r| {
                        !r.starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '$')
                    })
                };
                let phrase = match inner.chars().next() {
                    Some('{') => "object",
                    Some('[') => "array",
                    _ if word("function") || word("async") => "function",
                    _ if word("class") => "class",
                    _ => return,
                };
                let at = TextRange::at(start, TextSize::of(s));
                ctx.report(at, self.message.replace("{phrase}", phrase));
            }
            Check::BananaInBox => {
                if let Some(a) = Attr::parse(node, text)
                    && ng_checks::banana_in_box(a.full)
                {
                    ctx.report(a.range(), self.message.clone());
                }
            }
            Check::ForTrack => {
                let head = text[node.text_range()].trim_start();
                let head = head.split('{').next().unwrap_or(head);
                if ng_checks::for_without_track(head) {
                    ctx.report(node.text_range(), self.message.clone());
                }
            }
            Check::LooseEquality | Check::NegatedAsync => {
                let (code, start) = if node.kind() == k::ATTRIBUTE {
                    let Some(a) = Attr::parse(node, text).filter(|a| ng_checks::is_binding(a.full))
                    else {
                        return;
                    };
                    let Some(v) = a.value() else { return };
                    // (the value starts after the quote)
                    let at = text[a.start.into()..]
                        .find(v)
                        .map_or(a.start.into(), |i| usize::from(a.start) + i);
                    (v, at)
                } else {
                    let r = node.text_range();
                    (&text[r], r.start().into())
                };
                let hits = if matches!(self.check, Check::LooseEquality) {
                    ng_checks::loose_equality(code)
                } else {
                    ng_checks::negated_async(code)
                };
                let len = if matches!(self.check, Check::LooseEquality) {
                    2
                } else {
                    1
                };
                for i in hits {
                    ctx.report(
                        TextRange::at(TextSize::from((start + i) as u32), TextSize::from(len)),
                        self.message.clone(),
                    );
                }
            }
            Check::SpacedComment => {
                for t in node
                    .descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .filter(|t| t.kind() == k::COMMENT)
                {
                    let c = &text[t.text_range()];
                    let inner = c.strip_prefix("<!--").and_then(|c| c.strip_suffix("-->"));
                    if inner.is_some_and(|i| {
                        !i.is_empty()
                            && !(i.starts_with(char::is_whitespace)
                                && i.ends_with(char::is_whitespace))
                    }) {
                        ctx.report(t.text_range(), self.message.clone());
                    }
                }
            }
            Check::TargetBlank => {
                let attrs = attrs(node, text);
                // a link to a scheme or host, or one that is computed (`{x}`, `{href}`, `bind:href`, `:href`)
                let external = attrs.iter().any(|a| {
                    (a.full == "href"
                        && a.after.trim_start().strip_prefix('=').is_some_and(|v| {
                            let v = v.trim_start().trim_start_matches(['"', '\'']);
                            let scheme = v
                                .bytes()
                                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                                .count();
                            v.starts_with('{')
                                || v.starts_with("//")
                                || scheme > 0 && v[scheme..].starts_with(':')
                        }))
                        || matches!(a.full, "{href}" | "bind:href")
                        || a.name == "v-bind" && a.parts().0 == Some("href")
                });
                let safe = attrs.iter().any(|a| {
                    a.full == "rel"
                        && a.value().is_some_and(|v| {
                            let v = v.to_ascii_lowercase();
                            ["noopener", "noreferrer"]
                                .iter()
                                .all(|t| v.split_whitespace().any(|w| w == *t))
                        })
                });
                if let Some(target) = attrs
                    .iter()
                    .find(|a| a.full == "target" && a.value() == Some("_blank"))
                    && external
                    && !safe
                {
                    ctx.report(target.range(), self.message.clone());
                }
            }
            check => {
                let s = code(node, text).0;
                let candidate = match check {
                    Check::Directive(d) => d.prefixes.iter().any(|p| s.starts_with(p.as_str())),
                    Check::Shorthand { from, .. } => s.starts_with(from.as_str()),
                    Check::Duplicate { prefix, .. } => s.starts_with(prefix.as_str()),
                    Check::SvelteShorthand { .. } => s.contains("={") || s.contains("= {"),
                    Check::KeyOnChild => s.starts_with(":key") || s.starts_with("v-bind:key"),
                    Check::DupeElseIf => s.starts_with("v-else-if"),
                    Check::StyleProperty => s.starts_with("style:"),
                    Check::ComponentHtml => s.starts_with("v-html") || s.starts_with("v-text"),
                    Check::Hyphenation { on: true } => s.starts_with('@') || s.starts_with("v-on"),
                    Check::ChildContent => s.starts_with("v-html") || s.starts_with("v-text"),
                    _ => s
                        .bytes()
                        .take_while(|b| !b.is_ascii_whitespace() && *b != b'=')
                        .any(|b| b.is_ascii_uppercase()),
                };
                if !candidate {
                    return;
                }
                let Some(a) = Attr::parse(node, text) else {
                    return;
                };
                let Some(tag) = node.parent() else {
                    return;
                };
                let hit = match check {
                    Check::Directive(d) => {
                        a.name == d.name
                            && d.host.is_none_or(|h| is_host(h, &tag, text))
                            && d.violated(&a, &tag, text)
                    }
                    Check::Shorthand { from, .. } => a.full.starts_with(from.as_str()),
                    Check::Hyphenation { on } => {
                        let target = match (a.directive, a.name) {
                            (false, _) if !on => Some(a.full),
                            (true, "v-bind") if !on => a.parts().0,
                            (true, "v-on") if *on => a.parts().0,
                            _ => None,
                        };
                        target.is_some_and(|t| {
                            !t.starts_with('[')
                                && t.as_bytes()
                                    .windows(2)
                                    .any(|w| w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase())
                        }) && tag_name(&tag, text).is_some_and(|(n, _)| is_component(n))
                    }
                    Check::DupeElseIf => a.name == "v-else-if" && vue::dupe_else_if(&a, &tag, text),
                    Check::StyleProperty => {
                        let name = a.full["style:".len()..].split('|').next().unwrap_or("");
                        !name.starts_with('-') && !lang_css::data::has("property", name)
                    }
                    Check::ComponentHtml => {
                        matches!(a.name, "v-html" | "v-text")
                            && tag_name(&tag, text).is_some_and(|(n, _)| is_component(n))
                    }
                    Check::KeyOnChild => {
                        a.name == "v-bind"
                            && a.parts().0 == Some("key")
                            && !attrs(&tag, text).iter().any(|x| {
                                matches!(x.name, "v-if" | "v-else-if" | "v-else" | "v-for")
                            })
                            && tag
                                .parent()
                                .and_then(|e| e.parent())
                                .and_then(|g| g.child(k::START_TAG))
                                .is_some_and(|t| {
                                    let all = attrs(&t, text);
                                    is_host(Host::Template, &t, text)
                                        && all.iter().find(|x| x.name == "v-for").is_some_and(|f| {
                                            vue::uses_loop_variable(f, &a)
                                                && !all.iter().any(|x| {
                                                    x.name == "v-bind"
                                                        && x.parts().0 == Some("key")
                                                        && vue::uses_loop_variable(f, x)
                                                })
                                        })
                                })
                    }
                    Check::Duplicate {
                        strip, same_value, ..
                    } => {
                        let key = |a: &Attr<'_>| {
                            let name = a
                                .full
                                .split('|')
                                .next()
                                .filter(|_| *strip)
                                .unwrap_or(a.full);
                            if *same_value {
                                let value: String = a.after.split_whitespace().collect();
                                format!("{name}{value}")
                            } else {
                                name.to_string()
                            }
                        };
                        let mine = key(&a);
                        // with `same-value` every one of the equal directives is reported, else the later ones
                        let before =
                            std::iter::successors(node.prev_sibling(), SyntaxNode::prev_sibling);
                        let after =
                            std::iter::successors(node.next_sibling(), SyntaxNode::next_sibling)
                                .take(if *same_value { usize::MAX } else { 0 });
                        before
                            .chain(after)
                            .filter(|n| n.kind() == k::ATTRIBUTE)
                            .filter_map(|n| Attr::parse(&n, text))
                            .any(|o| key(&o) == mine)
                    }
                    Check::SvelteShorthand { directives } => {
                        let inner = a
                            .after
                            .trim()
                            .strip_prefix('=')
                            .map(str::trim)
                            .and_then(|r| r.strip_prefix('{'))
                            .and_then(|r| r.strip_suffix('}'))
                            .map(str::trim);
                        let target = if directives.is_empty() {
                            Some(a.full)
                        } else {
                            a.full
                                .split_once(':')
                                .filter(|(d, _)| directives.iter().any(|x| x == d))
                                .map(|(_, n)| n)
                        };
                        match (inner, target) {
                            (Some(i), Some(t)) => {
                                i == t
                                    && !t.is_empty()
                                    && t.chars()
                                        .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                            }
                            _ => false,
                        }
                    }
                    Check::ChildContent => {
                        matches!(a.name, "v-html" | "v-text")
                            && tag.parent().is_some_and(|e| {
                                e.children().any(|c| match c.kind() {
                                    k::ELEMENT | k::INTERPOLATION => true,
                                    k::CHUNK => !text[c.text_range()].trim().is_empty(),
                                    _ => false,
                                })
                            })
                    }
                    _ => false,
                };
                if !hit {
                    return;
                }
                if let Check::SvelteShorthand { directives } = check {
                    let end = a.start + TextSize::of(code(node, text).0.trim_end());
                    let whole = TextRange::new(a.start, end);
                    let edit = if directives.is_empty() {
                        Edit::replace(whole, format!("{{{}}}", a.full))
                    } else {
                        Edit::delete(TextRange::new(a.start + TextSize::of(a.full), end))
                    };
                    ctx.report_fix(
                        a.range(),
                        self.message.clone(),
                        ("Use the shorthand", vec![edit]),
                    );
                } else if let Check::Shorthand { from, to } = check {
                    let range = TextRange::at(a.start, TextSize::of(from.as_str()));
                    ctx.report_fix(
                        range,
                        self.message.clone(),
                        ("Use the shorthand", vec![Edit::replace(range, to.as_str())]),
                    );
                } else {
                    ctx.report(a.range(), self.message.clone());
                }
            }
        }
    }
}
