//! The `run-command` rule family: a check on the commands of a `RUN`, read from its Shell tree
//! (see the `embed` rows of `grammars/dockerfile.grammar`). The selection and the check are
//! `core_rules::commands`; params: those, `cache-dir` (a `--mount` of type cache or tmpfs on that
//! directory exempts the instruction) and `message`. With the exec form (`RUN ["a", "b"]`) the
//! command is the JSON strings.
//!
//! concept: embedded-code

use crate::generated::kind as k;
use core_rules::commands::{Cmd, CommandCheck, shell_commands};
use core_rules::tree::significant_range;
use core_rules::{EmbeddedTree, Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::SyntaxNode;
use core_syntax::SyntaxNodeExt;
use std::cell::RefCell;
use std::rc::Rc;

pub const FAMILY: Family = Family {
    name: "run-command",
    build,
};

struct RunCommand {
    meta: &'static RuleMeta,
    message: String,
    check: CommandCheck,
    cache_dir: Option<String>,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let rule = RunCommand {
        meta,
        check: CommandCheck::read(&mut p)?,
        message: p.text("message")?,
        cache_dir: p.optional("cache-dir"),
    };
    p.done()?;
    Ok(Box::new(rule))
}

thread_local! {
    /// The commands of the last instruction read: every rule asks about the same one in turn.
    static LAST: RefCell<Option<(SyntaxNode, Rc<Vec<Cmd>>)>> = const { RefCell::new(None) };
}

/// The commands of the instruction's own `RUN` (not a nested `ONBUILD`'s).
pub(super) fn run_commands(node: &SyntaxNode, embedded: &[EmbeddedTree]) -> Option<Rc<Vec<Cmd>>> {
    let mut tokens = node.tokens();
    let keyword = tokens.find(|t| t.kind() == k::KEYWORD)?;
    if !keyword.text().eq_ignore_ascii_case("run") {
        return None;
    }
    LAST.with_borrow_mut(|last| {
        if let Some((n, cmds)) = last
            && n == node
        {
            return Some(cmds.clone());
        }
        let cmds = Rc::new(read_commands(node, embedded));
        *last = Some((node.clone(), cmds.clone()));
        Some(cmds)
    })
}

fn read_commands(node: &SyntaxNode, embedded: &[EmbeddedTree]) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    let mut json = Vec::new();
    for t in node.tokens() {
        match t.kind() {
            // The script of `RUN cmd` or `RUN <<EOT`, parsed as shell.
            k::SHELL_COMMAND | k::HEREDOC => {
                for e in embedded
                    .iter()
                    .filter(|e| e.language == "shell" && t.text_range().contains_range(e.range))
                {
                    cmds.extend(shell_commands(&e.root));
                }
            }
            k::JSON_STRING => json.push(t.text().trim_matches('"').to_string()),
            _ => {}
        }
    }
    if !json.is_empty() {
        cmds = vec![Cmd {
            name: json.remove(0),
            args: json,
        }];
    }
    cmds
}

/// Whether a `--mount` of type cache or tmpfs targets `dir`.
fn mounted(node: &SyntaxNode, dir: &str) -> bool {
    node.tokens().filter(|t| t.kind() == k::WORD).any(|t| {
        let w = t.text();
        w.starts_with("--mount=") && (w.contains("cache") || w.contains("tmpfs")) && w.contains(dir)
    })
}

impl Rule for RunCommand {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[core_syntax::SyntaxKind] {
        &[k::INSTRUCTION]
    }

    fn reads_embedded(&self) -> bool {
        true
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(cmds) = run_commands(node, ctx.file.embedded) else {
            return;
        };
        if self.check.hit(&cmds) && !self.cache_dir.as_deref().is_some_and(|d| mounted(node, d)) {
            ctx.report(
                significant_range(node, ctx.file.grammar),
                self.message.clone(),
            );
        }
    }
}
