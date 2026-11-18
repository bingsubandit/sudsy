//! The `names` family: a check on the names of one kind of thing (CSS properties, units and
//! selector parts, HTML class names, ...). The language supplies the kinds of thing as a
//! [`Targets`] table and reads the names off its tree; the checks are written once. Params:
//! - `target`: what the names are, one of the language's targets;
//! - `mode`: `allowed` and `disallowed` (option `list`: names, or `/regex/`), `pattern` (each
//!   name must match option `pattern`), `lower` (each name is written in lowercase), `unique`
//!   (a name repeats within the node).
//!
//! The message has `{name}`.

use crate::unique::Repeats;
use crate::{Family, Params, Re, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};
use std::marker::PhantomData;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Allowed,
    Disallowed,
    Pattern,
    Lower,
    Unique,
}

/// The kinds of thing a language's `names` rules can check.
pub trait Targets: 'static {
    type Target: Copy + PartialEq + Send + Sync + 'static;
    /// The `target` param values.
    const ALL: &'static [(&'static str, Self::Target)];
    /// The node kinds the rule subscribes to.
    fn kinds(target: Self::Target) -> &'static [SyntaxKind];
    /// Whether `allowed` and `disallowed` compare without case.
    fn insensitive(_target: Self::Target) -> bool {
        false
    }
    /// Names `lower` accepts with capitals.
    fn keep_case(_target: Self::Target) -> &'static [&'static str] {
        &[]
    }
    /// The names in `node` with where each is.
    fn collect(
        target: Self::Target,
        mode: Mode,
        node: &SyntaxNode,
        ctx: &RuleContext<'_>,
        out: &mut Vec<(String, TextRange)>,
    );
}

pub const fn family<T: Targets>() -> Family {
    Family {
        name: "names",
        build: build::<T>,
    }
}

struct Names<T: Targets> {
    meta: &'static RuleMeta,
    target: T::Target,
    mode: Mode,
    message: String,
    /// The compiled `pattern` option, by its source.
    cache: Mutex<Option<(String, Option<Re>)>>,
    targets: PhantomData<fn() -> T>,
}

fn build<T: Targets>(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let target = p.choice("target", true, T::ALL)?;
    let mode = p.choice(
        "mode",
        true,
        &[
            ("allowed", Mode::Allowed),
            ("disallowed", Mode::Disallowed),
            ("pattern", Mode::Pattern),
            ("lower", Mode::Lower),
            ("unique", Mode::Unique),
        ],
    )?;
    let rule = Names::<T> {
        meta,
        target,
        mode,
        message: p.text("message")?,
        cache: Mutex::new(None),
        targets: PhantomData,
    };
    p.done()?;
    Ok(Box::new(rule))
}

/// Whether `entry` (a name, or `/regex/`) selects `name`.
pub fn selects(entry: &str, name: &str, insensitive: bool) -> bool {
    match entry.strip_prefix('/').and_then(|e| e.strip_suffix('/')) {
        Some(re) => Re::lazy(re).is_ok_and(|re| re.is_match(name)),
        None if insensitive => entry.eq_ignore_ascii_case(name),
        None => entry == name,
    }
}

impl<T: Targets> Names<T> {
    fn pattern_match(&self, source: &str, name: &str) -> bool {
        let mut cache = self.cache.lock().unwrap();
        if cache.as_ref().is_none_or(|(s, _)| s != source) {
            *cache = Some((source.to_string(), Re::lazy(source).ok()));
        }
        cache
            .as_ref()
            .and_then(|(_, re)| re.as_ref())
            .is_none_or(|re| re.is_match(name))
    }
}

impl<T: Targets> Rule for Names<T> {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        T::kinds(self.target)
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut names = Vec::new();
        T::collect(self.target, self.mode, node, ctx, &mut names);
        if names.is_empty() {
            return;
        }
        let list = ctx.options.list("list");
        let pattern = ctx.options.string("pattern");
        let insensitive = T::insensitive(self.target);
        let mut seen = Repeats::default();
        for (name, range) in names {
            let hit = match self.mode {
                Mode::Allowed => {
                    !list.is_empty() && !list.iter().any(|e| selects(e, &name, insensitive))
                }
                Mode::Disallowed => list.iter().any(|e| selects(e, &name, insensitive)),
                Mode::Pattern => !pattern.is_empty() && !self.pattern_match(pattern, &name),
                Mode::Lower => {
                    name.chars().any(|c| c.is_uppercase())
                        && !T::keep_case(self.target).contains(&name.as_str())
                }
                Mode::Unique => {
                    let message = || self.message.replace("{name}", &name);
                    let key = name.to_ascii_lowercase();
                    seen.check(ctx, key, range, message, "first used here");
                    continue;
                }
            };
            if hit {
                ctx.report(range, self.message.replace("{name}", &name));
            }
        }
    }
}
