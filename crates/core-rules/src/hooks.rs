//! What the shared rules ask a language about its own syntax. A language that tags its grammar
//! (`#mapping`, `#sequence`, `#key-part`) and gives it a `keys` line gets the shared key and
//! sequence rules with no code of its own; a language whose keys or scalars the rows cannot
//! describe implements the hooks it needs.
//!
//! concept: key-identity
//! what: What a key or scalar is called, read from tags and the keys row, for the key rules.
//! tools: eslint-plugin-jsonc, yamllint key rules
//! reads: grammar:keys

use crate::names;
use crate::rule::FileContext;
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, TextRange};
use std::borrow::Cow;

/// A scalar the sorting of sequence items understands.
#[derive(Clone, Debug, PartialEq)]
pub enum Scalar<'a> {
    String(Cow<'a, str>),
    Number(f64),
}

/// A getter or setter entry of an object: one of each may share a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accessor {
    Get,
    Set,
}

pub trait RuleHooks: Send + Sync {
    /// The keys, in order, of a `#mapping` whose entries are not nodes of their own (a Python
    /// dict is flat: `key : value , ...`); `None` when the entries are `#key-value` nodes.
    fn flat_keys(&self, _mapping: &SyntaxNode) -> Option<Vec<SyntaxElement>> {
        None
    }

    /// Whether a key is a name at all: `no-duplicate-keys` skips the others (HTML: template
    /// words inside a start tag, `{{ if }}`). The default is every key, or with `keys static` the
    /// keys that have a static name ([`names::is_name`]).
    fn is_name(&self, g: &Grammar, key: &SyntaxElement, _text: &str) -> bool {
        names::is_name(g, key)
    }

    /// The identity of a key (the `key` item of a `#key-value` entry): the name that is compared,
    /// sorted and checked. `text` is the file's text. The default reads the grammar's `keys` line
    /// ([`names::key_name`]).
    fn key_name<'a>(&self, g: &Grammar, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        names::key_name(g, key, text)
    }

    /// The value of a string or number item of a sequence (`item` is the value node); `None` for
    /// anything else, which `sorted-array-values` leaves alone ([`names::scalar`]).
    fn scalar<'a>(&self, g: &Grammar, item: &SyntaxNode, text: &'a str) -> Option<Scalar<'a>> {
        names::scalar(g, item, text)
    }

    /// Is the line a `key: value` entry whose value is one unbreakable word (no space)? Lets
    /// `max-line-length` accept it with `allow_non_breakable_inline_mappings`. The default has no
    /// such lines.
    fn inline_mapping_line(&self, _line: &str) -> bool {
        false
    }

    /// Rules the code silences for a range by its own syntax (PowerShell's
    /// `SuppressMessageAttribute`), as `(rule id, range)`. The default has none.
    fn suppressed_ranges(&self, _file: &FileContext<'_>) -> Vec<(String, TextRange)> {
        Vec::new()
    }
}

/// The hooks of a language that has none of its own.
pub struct DefaultHooks;

impl RuleHooks for DefaultHooks {}

pub static DEFAULT_HOOKS: DefaultHooks = DefaultHooks;
