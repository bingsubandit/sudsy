//! Shared parts of the `check` family of a language: a rule that runs one function of a table on
//! the nodes of given kinds (`params: { check: <name> }`; `{name}` in the message is what the
//! function passes to [`NodeCheck::hit`]), and the cache of a per-file analysis that several
//! checks read (a scope model, the findings of one walk).
//!
//! concept: check-family
//! what: A rule family that runs one function of a language table on nodes of given kinds.
//! tools: none (plumbing of Rust checks)
//! reads: family:check

use crate::tree::significant_range;
use crate::{Diagnostic, FileContext, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};
use std::cell::RefCell;
use std::rc::Rc;

/// The function of one check.
pub type Run = fn(&NodeCheck, &SyntaxNode, &mut RuleContext<'_>);

/// A row of a language's table: the check's name, the node kinds it runs on, its function.
pub type Entry = (&'static str, &'static [SyntaxKind], Run);

pub struct NodeCheck {
    pub meta: &'static RuleMeta,
    pub message: String,
    kinds: &'static [SyntaxKind],
    run: Run,
}

/// The `build` of a `check` family over `table`; the rule's remaining param is `message`.
pub fn build(
    table: &'static [Entry],
    meta: &'static RuleMeta,
    mut p: Params,
) -> Result<Box<dyn Rule>, String> {
    let name = p.text("check")?;
    let Some((_, kinds, run)) = table.iter().find(|(n, ..)| *n == name) else {
        return Err(format!("param `check`: unknown check `{name}`"));
    };
    let rule = NodeCheck {
        meta,
        message: p.text("message")?,
        kinds,
        run: *run,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for NodeCheck {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn bodies(&self) -> bool {
        false
    }

    fn kinds(&self) -> &[SyntaxKind] {
        self.kinds
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        (self.run)(self, node, ctx);
    }
}

impl NodeCheck {
    /// Reports at `range`; the message's `{name}` is `name`.
    pub fn hit<'a>(
        &self,
        ctx: &'a mut RuleContext<'_>,
        range: TextRange,
        name: &str,
    ) -> &'a mut Diagnostic {
        ctx.report(range, self.message.replace("{name}", name))
    }

    /// Reports one item of a list at `item` (its range, when the item has one), else at the
    /// significant range of `node`, the whole list.
    pub fn hit_item<'a>(
        &self,
        ctx: &'a mut RuleContext<'_>,
        item: Option<TextRange>,
        node: &SyntaxNode,
        name: &str,
    ) -> &'a mut Diagnostic {
        let range = item.unwrap_or_else(|| significant_range(node, ctx.file.grammar));
        self.hit(ctx, range, name)
    }
}

/// The last analysis of a file, kept for the rules that run on the same tree after it. Declare
/// it in a `thread_local!`; a request for another tree computes and replaces it.
pub struct FileCache<T>(RefCell<Option<(SyntaxNode, Rc<T>)>>);

impl<T> FileCache<T> {
    pub const fn new() -> Self {
        FileCache(RefCell::new(None))
    }

    /// The analysis of `file`, from `build` when the cache holds another tree's.
    pub fn get(&self, file: &FileContext<'_>, build: impl FnOnce() -> T) -> Rc<T> {
        let mut slot = self.0.borrow_mut();
        if let Some((root, value)) = &*slot
            && root == file.root
        {
            return value.clone();
        }
        let value = Rc::new(build());
        *slot = Some((file.root.clone(), value.clone()));
        value
    }
}

impl<T> Default for FileCache<T> {
    fn default() -> Self {
        Self::new()
    }
}
