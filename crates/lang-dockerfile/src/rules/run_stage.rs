//! Families on the `RUN` instructions of a stage: `consecutive-run` (a `RUN` directly after another
//! one that does the same kind of work) and `run-pipe-shell` (a `RUN` with a pipe in a stage whose
//! `SHELL` has not been given an option). Both read the commands of the Shell tree of each `RUN`.
//!
//! concept: embedded-code

use super::run_command::run_commands;
use super::stages::{Inst, instructions};
use crate::generated::kind as k;
use core_rules::commands::{has_pipeline, single_command_flags};
use core_rules::unique::Adjacent;
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

/// A `RUN` that is one command and has the same flags as the `RUN` before it.
fn consecutive(_: &(), doc: &SyntaxNode, ctx: &mut RuleContext<'_>, message: &str) {
    let mut before = Adjacent::default();
    for i in instructions(doc) {
        let flags = (i.name == "RUN")
            .then(|| run_commands(&i.node, ctx.file.embedded))
            .flatten()
            .and_then(|cmds| single_command_flags(&cmds, i.words()));
        let same = before.repeats(flags.clone());
        if flags.is_some() && same {
            ctx.report(i.range(ctx), message);
        }
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
