//! `sudsy hook <event>`: the agent hooks. Reads the hook input of Claude Code on stdin (the protocol
//! of code.claude.com/docs/en/hooks) and answers with its exit codes: 0 says nothing to the agent,
//! 2 shows stderr to it (after an edit) or makes it go on (at a stop); any other code is a hook
//! error shown to the user only.
//! - `post-edit` (PostToolUse on Edit, Write, MultiEdit): the new findings of the edited file,
//!   never a fix or a format (the agent's next edit must find the text it holds).
//! - `stop` (Stop): safe fixes and the formatter on the files changed since `HEAD` and the
//!   untracked ones, then their new findings; a stop the hook already turned back ends anyway.

use crate::lint::{self, keep_new_of};
use crate::new::Base;
use crate::report::{self, FileDiagnostics, OutputFormat};
use crate::session::Session;
use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use core_workspace::git::Repo;
use serde::Deserialize;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Clone, Copy, ValueEnum)]
enum Event {
    /// After the agent edited a file: lint it.
    PostEdit,
    /// When the agent stops: fix and format the changed files, then lint them.
    Stop,
}

#[derive(Args)]
pub struct HookArgs {
    #[arg(value_enum)]
    event: Event,
}

/// The fields of the hook input that Sudsy reads; the others are ignored.
#[derive(Deserialize, Default)]
struct Input {
    cwd: Option<PathBuf>,
    tool_input: Option<ToolInput>,
    /// Set on a Stop when the agent goes on because a stop hook turned it back.
    #[serde(default)]
    stop_hook_active: bool,
}

#[derive(Deserialize)]
struct ToolInput {
    /// Edit, Write and MultiEdit (whose `edits` are all in this file).
    file_path: Option<PathBuf>,
}

/// Exit code 2: the agent reads stderr.
const TO_AGENT: u8 = 2;

pub fn run(args: HookArgs) -> ExitCode {
    let result = read_input().and_then(|input| {
        if let Some(cwd) = &input.cwd {
            std::env::set_current_dir(cwd)
                .with_context(|| format!("entering {}", cwd.display()))?;
        }
        match args.event {
            Event::PostEdit => post_edit(&input),
            Event::Stop => stop(&input),
        }
    });
    // A failure (bad input, an invalid config) is for the user, not the agent.
    result.unwrap_or_else(|e| {
        eprintln!("sudsy hook: {e:#}");
        ExitCode::FAILURE
    })
}

fn read_input() -> Result<Input> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .context("reading the hook input")?;
    serde_json::from_str(&text).context("the hook input is not the JSON of a hook event")
}

fn post_edit(input: &Input) -> Result<ExitCode> {
    let Some(path) = input.tool_input.as_ref().and_then(|t| t.file_path.clone()) else {
        return Ok(ExitCode::SUCCESS);
    };
    if !path.is_file() {
        return Ok(ExitCode::SUCCESS);
    }
    // Outside git, or before the first commit, every finding is new.
    let base = Base::open("HEAD").ok();
    let Some(mut session) = handled(&[path])? else {
        return Ok(ExitCode::SUCCESS);
    };
    let loaded = session.load_linted(|db, file| {
        core_incremental::lint(db, file);
    });
    let mut collected = lint::collect(&session, &loaded)?;
    if let Some(base) = &base {
        keep_new_of(&session, base, &loaded, &mut collected.files)?;
    }
    match findings(&collected.files) {
        Some(text) => {
            eprint!("{text}");
            Ok(ExitCode::from(TO_AGENT))
        }
        None => Ok(failed(collected.failed)),
    }
}

fn stop(input: &Input) -> Result<ExitCode> {
    let dir = std::env::current_dir()?;
    let base = match Repo::discover(&dir).and_then(|repo| Base::at(repo, "HEAD")) {
        Ok(base) => base,
        Err(e) => {
            eprintln!("sudsy hook stop: {e}; nothing fixed, formatted or checked");
            return Ok(ExitCode::SUCCESS);
        }
    };
    let Some(session) = handled(&base.worktree_files()?)? else {
        return Ok(ExitCode::SUCCESS);
    };
    let (collected, changed) = crate::fix::safe_new(&session, &base)?;
    let Some(text) = findings(&collected.files) else {
        return Ok(failed(collected.failed));
    };
    let lead = match changed {
        0 => String::new(),
        n => format!("Sudsy fixed and formatted {n} file(s); read them again before editing.\n"),
    };
    if input.stop_hook_active {
        // The agent already went on once for this stop: let it end, and show the user what is left.
        let note =
            serde_json::json!({ "systemMessage": format!("{lead}Sudsy findings left:\n{text}") });
        println!("{note}");
        return Ok(ExitCode::SUCCESS);
    }
    eprint!("{lead}{text}");
    Ok(ExitCode::from(TO_AGENT))
}

/// The session of the files of `paths` that Sudsy handles and the config does not exclude;
/// `None` when there is none.
fn handled(paths: &[PathBuf]) -> Result<Option<Session>> {
    if paths.is_empty() {
        return Ok(None);
    }
    let mut session = Session::open(paths, None)?;
    let project = &session.project;
    session
        .paths
        .retain(|p| !project.is_excluded(p) && project.chain.is_included(p));
    Ok((!session.paths.is_empty()).then_some(session))
}

/// The human report of the findings with its summary line; `None` without findings.
fn findings(files: &[FileDiagnostics]) -> Option<String> {
    let summary = lint::summary(files, true)?;
    let text = report::render(OutputFormat::Human, files, &Vec::new(), true);
    Some(format!("{text}{summary}\n"))
}

/// No findings: 0, or 1 (a hook error for the user) when a file could not be read or linted.
fn failed(files: usize) -> ExitCode {
    if files > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
