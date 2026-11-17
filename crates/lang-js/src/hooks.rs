//! What the shared rules ask of JavaScript: the static name of an object key, and getters and
//! setters (which may share a name).

use crate::rules::key_name;
use core_rules::{Accessor, RuleHooks};
use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

pub struct JsHooks;

impl RuleHooks for JsHooks {
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        match key {
            SyntaxElement::Node(n) => key_name(n, text).map_or(Cow::Borrowed(""), Cow::Owned),
            SyntaxElement::Token(_) => Cow::Borrowed(""),
        }
    }

    /// A computed key (`[a]`) has no static name.
    fn is_name(&self, key: &SyntaxElement, text: &str) -> bool {
        matches!(key, SyntaxElement::Node(n) if key_name(n, text).is_some())
    }

    fn custom_entries(&self) -> bool {
        true
    }

    fn has_accessors(&self) -> bool {
        true
    }

    fn accessor(&self, entry: &SyntaxNode) -> Option<Accessor> {
        let first = entry
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| matches!(t.text(), "get" | "set"))?;
        Some(if first.text() == "get" {
            Accessor::Get
        } else {
            Accessor::Set
        })
    }
}
