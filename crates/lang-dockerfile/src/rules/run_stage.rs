//! Families on the `RUN` instructions of a stage: `consecutive-run` (a `RUN` directly after another
//! one that does the same kind of work) and `run-pipe-shell` (a `RUN` with a pipe in a stage whose
//! `SHELL` has not been given an option). Both read the commands of the Shell tree of each `RUN`.
//!
//! concept: embedded-code

use super::run_command::run_commands;
use super::stages::{Inst, instructions};
use crate::generated::kind as k;
use core_rules::commands::has_pipeline;
use core_rules::{Checked, Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::SyntaxNode;

pub const CONSECUTIVE: Family = Family {
    name: "consecutive-run",
    build: |meta, mut p| {
        let message = p.text("message")?;
        p.done()?;
        Ok(checked(meta, message, (), consecutive))
    },
};

pub const PIPE_SHELL: Family = Family {
    name: "run-pipe-shell",
    build: |meta, mut p: Params| {
        let (message, option) = (p.text("message")?, p.text("option")?);
        p.done()?;
        Ok(checked(meta, message, option, pipe_shell))
    },
};

fn checked<S: Send + Sync + 'static>(
    meta: &'static RuleMeta,
    message: String,
    state: S,
    run: fn(&S, &SyntaxNode, &mut RuleContext<'_>, &str),
) -> Box<dyn Rule> {
    Box::new(Checked {
        meta,
        message,
        state,
        kinds: &[k::DOCUMENT],
        run,
    })
}

/// The flags of a `RUN` when it is a single command, else `None`.
fn single_command_flags(i: &Inst, ctx: &RuleContext<'_>) -> Option<Vec<String>> {
    let cmds = run_commands(&i.node, ctx.file.embedded)?;
    (cmds.len() == 1).then(|| {
        i.words()
            .into_iter()
            .take_while(|w| w.starts_with("--"))
            .collect()
    })
}

/// A `RUN` that is one command and has the same flags as the `RUN` before it.
fn consecutive(_: &(), doc: &SyntaxNode, ctx: &mut RuleContext<'_>, message: &str) {
    let mut before: Option<Vec<String>> = None;
    for i in instructions(doc) {
        let flags = if i.name == "RUN" {
            single_command_flags(&i, ctx)
        } else {
            None
        };
        if flags.is_some() && flags == before {
            ctx.report(i.range(ctx), message);
        }
        before = flags;
    }
}

/// A `RUN` with a pipe, in a stage where no `SHELL` has a word that contains `option`.
#[allow(clippy::ptr_arg)] // the state type of `Checked<String>`
fn pipe_shell(option: &String, doc: &SyntaxNode, ctx: &mut RuleContext<'_>, message: &str) {
    let mut set = false;
    for i in instructions(doc) {
        match i.name.as_str() {
            "FROM" => set = false,
            "SHELL" => set |= i.words().iter().any(|w| w.contains(option.as_str())),
            "RUN" if !set => {
                let range = i.node.text_range();
                let piped = ctx.file.embedded.iter().any(|e| {
                    e.language == "shell" && range.contains_range(e.range) && has_pipeline(&e.root)
                });
                if piped {
                    ctx.report(i.range(ctx), message);
                }
            }
            _ => {}
        }
    }
}
