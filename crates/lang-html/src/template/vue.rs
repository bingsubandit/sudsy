//! Checks of the `template` family that read a whole start tag or a chain of elements: the
//! `v-for` key, the self-closing style, the order of attributes and repeated `v-else-if`
//! conditions. Each returns the ranges to report.
//!
//! concept: template-syntax-checks

use super::{
    Attr, SyntaxNode, TextRange, TextSize, attrs, code, is_component, previous_tag, tag_name,
};
use crate::generated::kind as k;
use core_rules::markup::{Bind, Dialect};
use core_syntax::SyntaxNodeExt;

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
const BUILT_IN: &[&str] = &[
    "component",
    "suspense",
    "template",
    "transition",
    "transition-group",
    "keep-alive",
    "slot",
    "teleport",
];

/// The start tag without the blank space before it.
fn tag_range(tag: &SyntaxNode, text: &str) -> TextRange {
    let (s, start) = code(tag, text);
    TextRange::at(start, TextSize::of(s.trim_end()))
}

/// The attribute binds `key`, as the dialect spells a binding.
fn is_key(a: &Attr<'_>, dialect: &Dialect<'_>) -> bool {
    dialect.read(a.full) == Some((Bind::Prop, "key"))
}

/// A `v-for` element without a `:key`: a `<template>` or `<slot>` passes the duty to its child
/// elements, a component is checked by `valid-v-for`.
pub(super) fn for_key(tag: &SyntaxNode, text: &str, path: &std::path::Path) -> Vec<TextRange> {
    let mut out = Vec::new();
    if attrs(tag, text).iter().any(|a| a.name == "v-for") {
        key_of(tag, text, &crate::content::spellings().of(path), &mut out);
    }
    out
}

fn key_of(tag: &SyntaxNode, text: &str, dialect: &Dialect<'_>, out: &mut Vec<TextRange>) {
    let a = attrs(tag, text);
    if a.iter().any(|a| is_key(a, dialect)) {
        return;
    }
    let Some((name, _)) = tag_name(tag, text) else {
        return;
    };
    if matches!(name, "template" | "slot") {
        for child in tag.parent().into_iter().flat_map(|e| e.children()) {
            if let Some(t) = child
                .child(k::START_TAG)
                .filter(|_| child.kind() == k::ELEMENT)
            {
                key_of(&t, text, dialect, out);
            }
        }
    } else if !is_component(name)
        && !a.iter().any(|a| {
            a.name == "is" || a.name == "v-is" || a.name == "v-bind" && a.parts().0 == Some("is")
        })
    {
        out.push(tag_range(tag, text));
    }
}

/// `<div></div>` is written `<div />` and `<img />` is written `<img>`, except for the elements
/// of the file's root.
pub(super) fn self_closing(tag: &SyntaxNode, text: &str) -> Vec<TextRange> {
    let (Some((name, _)), Some(element)) = (tag_name(tag, text), tag.parent()) else {
        return Vec::new();
    };
    if element.parent().is_none_or(|p| p.kind() == k::DOCUMENT) {
        return Vec::new();
    }
    let selfclosing = text[tag.text_range()].trim_end().ends_with("/>");
    let void = VOID.contains(&name.to_ascii_lowercase().as_str());
    let component = is_component(name) || BUILT_IN.contains(&name);
    let wrong = if void && !component {
        selfclosing
    } else {
        let end = element
            .child(k::END_TAG)
            .map_or(element.text_range().end(), |e| e.text_range().start());
        let inside = TextRange::new(tag.text_range().end(), end.max(tag.text_range().end()));
        !selfclosing && text[inside].trim().is_empty()
    };
    if wrong {
        vec![tag_range(tag, text)]
    } else {
        Vec::new()
    }
}

/// The position of an attribute in Vue's recommended order: `v-for`, conditions, `v-pre` and
/// `v-once`, `id`, `ref`, `key` and slots, `v-model`, other directives, attributes, events,
/// `v-html` and `v-text`; `is` first.
fn position(a: &Attr<'_>) -> u8 {
    if a.directive && a.name != "v-bind" {
        return match a.name {
            "v-is" => 0,
            "v-for" => 1,
            "v-if" | "v-else-if" | "v-else" | "v-show" | "v-cloak" => 2,
            "v-pre" | "v-once" => 3,
            "v-slot" => 5,
            "v-model" => 6,
            "v-on" => 9,
            "v-html" | "v-text" => 10,
            _ => 7,
        };
    }
    match if a.directive {
        a.parts().0.unwrap_or("")
    } else {
        a.name
    } {
        "is" => 0,
        "id" => 4,
        "ref" | "key" | "slot" | "slot-scope" => 5,
        _ => 8,
    }
}

/// An attribute that comes after one that should follow it. A bare `v-bind="object"` takes the
/// place of the attribute it precedes, and is left out when next to one.
pub(super) fn attributes_order(tag: &SyntaxNode, text: &str) -> Vec<TextRange> {
    let all = attrs(tag, text);
    let object = |a: &Attr<'_>| a.directive && a.name == "v-bind" && a.parts().0.is_none();
    let value = |a: &Attr<'_>| !a.directive || a.name == "v-bind" || a.name == "v-model";
    let kept: Vec<&Attr<'_>> = all
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            !object(a) || !(*i > 0 && value(&all[i - 1]) || all.get(i + 1).is_some_and(value))
        })
        .map(|(_, a)| a)
        .collect();
    let at = |i: usize| {
        if object(kept[i]) {
            kept[i + 1..]
                .iter()
                .find(|n| value(n) && !object(n))
                .map_or(8, |n| position(n))
        } else {
            position(kept[i])
        }
    };
    let mut out = Vec::new();
    let mut before = kept.first().map_or(0, |_| at(0));
    for (i, a) in kept.iter().enumerate().skip(1) {
        let here = at(i);
        if before <= here {
            before = here;
        } else {
            out.push(a.range());
        }
    }
    out
}

fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .filter(|w| !w.is_empty())
}

/// Whether the `:key` reads a variable that the `v-for` declares.
pub(super) fn uses_loop_variable(for_attr: &Attr<'_>, key: &Attr<'_>) -> bool {
    let value = for_attr.value().unwrap_or("");
    let Some(alias) = value
        .split_once(" in ")
        .or_else(|| value.split_once(" of "))
        .map(|(a, _)| a)
    else {
        return false;
    };
    let declared: Vec<&str> = words(alias).collect();
    words(key.value().unwrap_or("")).any(|w| declared.contains(&w))
}

fn condition(a: &Attr<'_>) -> String {
    a.value().unwrap_or("").split_whitespace().collect()
}

/// A `v-else-if` whose condition, or each of whose `||` alternatives, an earlier branch of the
/// chain already has.
pub(super) fn dupe_else_if(a: &Attr<'_>, tag: &SyntaxNode, text: &str) -> bool {
    let mine = condition(a);
    if mine.is_empty() {
        return false;
    }
    let mut seen: Vec<String> = Vec::new();
    let mut at = previous_tag(tag);
    while let Some(t) = at {
        let a = attrs(&t, text);
        let Some(c) = a.iter().find(|x| x.name == "v-else-if" || x.name == "v-if") else {
            break;
        };
        seen.extend(condition(c).split("||").map(str::to_string));
        if c.name == "v-if" {
            break;
        }
        at = previous_tag(&t);
    }
    mine.split("||").all(|term| seen.iter().any(|s| s == term))
}
