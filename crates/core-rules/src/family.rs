//! Rule families: Rust code that built-in rule files instantiate by name. A file with
//! `family: <name>` and `params:` instead of `rule:` becomes one rule of that family; the
//! language provider lists its families, the pattern loader (`core-patterns`) builds the rules.
//!
//! concept: none (glue: family registry)

use crate::{Rule, RuleMeta};
use std::collections::BTreeMap;

/// Makes the rule from its metadata (read from the file) and its `params`.
pub type Build = fn(&'static RuleMeta, Params) -> Result<Box<dyn Rule>, String>;

#[derive(Clone, Copy)]
pub struct Family {
    pub name: &'static str,
    pub build: Build,
}

/// The `params:` of a rule file (plus its `message`, under `message`). A family takes what it
/// reads and calls [`Params::done`], which rejects every param it did not read.
pub struct Params(BTreeMap<String, String>);

impl Params {
    pub fn new(map: BTreeMap<String, String>) -> Params {
        Params(map)
    }

    /// The text of param `key`; an error when it is missing.
    pub fn text(&mut self, key: &str) -> Result<String, String> {
        self.0
            .remove(key)
            .ok_or_else(|| format!("param `{key}` is missing"))
    }

    /// The text of param `key`, if given.
    pub fn optional(&mut self, key: &str) -> Option<String> {
        self.0.remove(key)
    }

    /// `key` is one of `options` (`(text, value)` pairs); the first pair is the default unless
    /// `required`.
    pub fn choice<T: Copy>(
        &mut self,
        key: &str,
        required: bool,
        options: &[(&str, T)],
    ) -> Result<T, String> {
        let Some(text) = self.0.remove(key) else {
            return match options.first() {
                Some((_, v)) if !required => Ok(*v),
                _ => Err(format!("param `{key}` is missing")),
            };
        };
        let found = options.iter().find(|(name, _)| *name == text);
        found.map(|(_, v)| *v).ok_or_else(|| {
            let names: Vec<&str> = options.iter().map(|(n, _)| *n).collect();
            format!("param `{key}`: `{text}` is not one of {}", names.join(", "))
        })
    }

    /// `key` is `true` or `false` (default `false`).
    pub fn flag(&mut self, key: &str) -> Result<bool, String> {
        self.choice(key, false, &[("false", false), ("true", true)])
    }

    /// Every param not read yet, in key order; a family whose param names are its data.
    pub fn rest(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.0).into_iter().collect()
    }

    /// Fails on a param nothing read.
    pub fn done(self) -> Result<(), String> {
        match self.0.keys().next() {
            Some(key) => Err(format!("unknown param `{key}`")),
            None => Ok(()),
        }
    }
}

/// A family rule whose check is one function over the nodes of `kinds`, with the family's own
/// `state` (read from its params) and the rule's message: the rest of a family is then its `build`.
pub struct Checked<S> {
    pub meta: &'static RuleMeta,
    pub message: String,
    pub state: S,
    pub kinds: &'static [core_syntax::SyntaxKind],
    pub run: fn(&S, &core_syntax::SyntaxNode, &mut crate::RuleContext<'_>, &str),
}

impl<S: Send + Sync> Rule for Checked<S> {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[core_syntax::SyntaxKind] {
        self.kinds
    }

    fn check(&self, node: &core_syntax::SyntaxNode, ctx: &mut crate::RuleContext<'_>) {
        (self.run)(&self.state, node, ctx, &self.message);
    }
}
