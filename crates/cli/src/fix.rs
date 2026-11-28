//! `sudsy fix [--unsafe | --apply ID,...] [paths...]`: applies fixes until none is left, or
//! exactly the fixes named by id, then formats, and reports what remains.

use crate::lint::{self, Collected, Report, ReportArgs, exit_code};
use crate::new::{Base, Entry, keep_new};
use crate::report::FileDiagnostics;
use crate::session::{Session, load, par_map};
use anyhow::{Context, Result, anyhow};
use clap::Args;
use core_incremental::{Database, File, Stage, fix_text, format, lint as lint_query, transient};
use core_rules::fix_ids::fix_ids;
use core_rules::fixes::{apply, select};
use core_rules::{Diagnostic, Fix, LineIndex};
use languages::Project;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Args)]
pub struct FixArgs {
    #[command(flatten)]
    report: ReportArgs,
    /// Also apply fixes that can change behavior or need a newer language version.
    #[arg(long = "unsafe", conflicts_with = "apply")]
    allow_unsafe: bool,
    /// Apply exactly the fixes with these ids (comma-separated or repeated; the report prints them
    /// as `fix ID (kind)`), suggestions included, and no other fix.
    #[arg(long, value_delimiter = ',', value_name = "ID")]
    apply: Vec<String>,
}

/// Which fixes a run applies.
enum Mode<'a> {
    /// Every fix of these kinds, round after round.
    Kinds { allow_unsafe: bool },
    /// The fixes with these ids, in one round.
    Ids(&'a HashSet<String>),
}

/// What happened to one file.
struct Fixed {
    /// The file as read, for `--new`.
    file: File,
    path: String,
    text: String,
    changed: bool,
    diagnostics: Vec<Diagnostic>,
    /// Rules whose fixes made the file unparseable, so that round was not applied.
    broke: Option<Vec<String>>,
    named: Named,
}

/// The fixes of a file that were named by id.
#[derive(Default)]
struct Named {
    /// The ids found in the file.
    found: Vec<String>,
    /// Found but not applied: the fix overlaps another named one.
    overlapping: Vec<String>,
    applied: usize,
}

/// Why a file was not processed.
enum Failed {
    File(anyhow::Error),
    /// The config or the format options are invalid: the run stops.
    Run(anyhow::Error),
}

impl From<core_incremental::Failure> for Failed {
    fn from(f: core_incremental::Failure) -> Failed {
        let e = anyhow!("{}", f.message);
        if matches!(f.stage, Stage::Options | Stage::Config) {
            Failed::Run(e)
        } else {
            Failed::File(e)
        }
    }
}

/// The fixes of `file` named in `wanted`, applied in one round. A fix that overlaps another named
/// one is left for a later run (the edit changes its line, so its id may change too); a round that
/// would leave the file unparseable is not applied.
fn apply_named(
    db: &Database,
    file: File,
    shown: &str,
    wanted: &HashSet<String>,
) -> Result<(String, Option<Vec<String>>, Named), Failed> {
    let text = file.text(db).clone();
    let linted = lint_query(db, file)
        .as_ref()
        .map_err(|f| Failed::from(f.clone()))?;
    let ids = fix_ids(shown, &text, &LineIndex::new(&text), &linted.diagnostics);
    let mut named_fixes: Vec<(&Fix, &String)> = Vec::new();
    for (d, ids) in linted.diagnostics.iter().zip(&ids) {
        for (fix, id) in d.fixes.iter().zip(ids) {
            if let Some(id) = id
                && wanted.contains(id)
            {
                named_fixes.push((fix, id));
            }
        }
    }
    let mut named = Named {
        found: named_fixes.iter().map(|(_, id)| (*id).clone()).collect(),
        ..Named::default()
    };
    if named_fixes.is_empty() || !linted.parses {
        return Ok((text, None, named));
    }
    let chosen = select(&linted.diagnostics, |f| {
        named_fixes.iter().any(|(n, _)| std::ptr::eq(*n, f))
    });
    named.overlapping = named_fixes
        .iter()
        .filter(|(n, _)| !chosen.iter().any(|c| std::ptr::eq(*c, *n)))
        .map(|(_, id)| (*id).clone())
        .collect();
    let next = apply(&text, &chosen);
    let after = transient(db, file, &next);
    let parses = lint_query(db, after)
        .as_ref()
        .map_err(|f| Failed::from(f.clone()))?
        .parses;
    if !parses {
        let mut rules: Vec<String> = linted
            .diagnostics
            .iter()
            .filter(|d| d.fixes.iter().any(|f| chosen.contains(&f)))
            .map(|d| d.rule.clone())
            .collect();
        rules.dedup();
        return Ok((text, Some(rules), named));
    }
    named.applied = chosen.len();
    Ok((next, None, named))
}

fn fix_file(db: &Database, project: &Project, path: &Path, mode: &Mode) -> Result<Fixed, Failed> {
    let file = load(db, project, path).map_err(Failed::File)?;
    let (before, had_bom) = (file.text(db).clone(), file.had_bom(db));
    let shown = project.show(path);
    let (mut text, broke, named) = match mode {
        Mode::Kinds { allow_unsafe } => {
            let outcome = fix_text(db, file, |fix| fix.kind.applies(*allow_unsafe))?;
            (outcome.text, outcome.broke, Named::default())
        }
        Mode::Ids(wanted) => apply_named(db, file, &shown, wanted)?,
    };
    // Then format (by id: only a file that got a fix); a file that does not parse is left as it is.
    if matches!(mode, Mode::Kinds { .. }) || named.applied > 0 {
        let fixed = transient(db, file, &text);
        let linted = lint_query(db, fixed)
            .as_ref()
            .map_err(|f| Failed::from(f.clone()))?;
        if linted.parses {
            text = format(db, fixed)
                .as_ref()
                .map_err(|f| Failed::from(f.clone()))?
                .clone();
        }
    }
    let changed = text != before;
    if changed {
        let bytes = crate::atomic::encode_like_file(path, &text, had_bom).map_err(Failed::File)?;
        crate::atomic::write(path, &bytes)
            .context("writing the file")
            .map_err(Failed::File)?;
    }
    let after = transient(db, file, &text);
    let diagnostics = lint_query(db, after)
        .as_ref()
        .map_err(|f| Failed::from(f.clone()))?
        .diagnostics
        .clone();
    Ok(Fixed {
        file,
        path: shown,
        text,
        changed,
        diagnostics,
        broke,
        named,
    })
}

/// What a run did to the files of its session.
struct Applied<'a> {
    collected: Collected,
    /// Files written.
    changed: usize,
    /// Fixes applied by id.
    applied: usize,
    /// The ids found in the files.
    found: HashSet<String>,
    /// The path and the file as read of each entry of `collected.files`.
    handles: Vec<(&'a PathBuf, File)>,
}

impl Applied<'_> {
    fn keep_new(&mut self, db: &Database, base: &Base) -> Result<()> {
        let entries = self
            .handles
            .iter()
            .zip(self.collected.files.iter_mut())
            .map(|(&(path, file), report)| Entry { path, file, report })
            .collect();
        keep_new(db, base, entries)
    }
}

/// Fixes and formats every file of `session`; warns on stderr about rounds and named fixes that
/// were not applied.
fn apply_all<'a>(session: &'a Session, mode: &Mode) -> Result<Applied<'a>> {
    let (db, project) = (&session.db, &session.project);
    let results = par_map(db, &session.paths, |db, path| {
        fix_file(db, project, path, mode)
    });
    let mut out = Applied {
        collected: Collected {
            files: Vec::new(),
            failed: 0,
        },
        changed: 0,
        applied: 0,
        found: HashSet::new(),
        handles: Vec::new(),
    };
    for (path, result) in session.paths.iter().zip(results) {
        match result {
            Ok(fixed) => {
                out.changed += usize::from(fixed.changed);
                if let Some(rules) = &fixed.broke {
                    eprintln!(
                        "warning: {}: fixes of {} would leave the file unparseable and were not applied",
                        fixed.path,
                        rules.join(", ")
                    );
                }
                for id in &fixed.named.overlapping {
                    eprintln!(
                        "warning: {}: fix {id} overlaps another named fix and was not applied; \
                         its id after this run is in the report",
                        fixed.path
                    );
                }
                out.applied += fixed.named.applied;
                out.found.extend(fixed.named.found);
                out.handles.push((path, fixed.file));
                out.collected.files.push(FileDiagnostics {
                    path: fixed.path,
                    text: fixed.text,
                    diagnostics: fixed.diagnostics,
                });
            }
            Err(Failed::Run(e)) => return Err(e),
            Err(Failed::File(e)) => {
                out.collected.failed += 1;
                crate::report::file_error(&project.show(path), &format!("{e:#}"));
            }
        }
    }
    Ok(out)
}

/// The safe fixes and the formatter on every file of `session`, as `fix --new` does: the new
/// findings that remain, and how many files were written.
pub fn safe_new(session: &Session, base: &Base) -> Result<(Collected, usize)> {
    let mut applied = apply_all(
        session,
        &Mode::Kinds {
            allow_unsafe: false,
        },
    )?;
    applied.keep_new(&session.db, base)?;
    Ok((applied.collected, applied.changed))
}

pub fn run(args: FixArgs) -> Result<ExitCode> {
    let base = args.report.base()?;
    let session: Session = args.report.open()?;
    let wanted: Vec<String> = args
        .apply
        .iter()
        .map(|id| id.trim().to_ascii_lowercase())
        .filter(|id| !id.is_empty())
        .collect();
    let mut wanted_set = HashSet::new();
    let wanted: Vec<String> = wanted
        .into_iter()
        .filter(|id| wanted_set.insert(id.clone()))
        .collect();
    let mode = if args.apply.is_empty() {
        Mode::Kinds {
            allow_unsafe: args.allow_unsafe,
        }
    } else {
        Mode::Ids(&wanted_set)
    };
    let mut applied = apply_all(&session, &mode)?;
    if matches!(mode, Mode::Ids(_)) {
        eprintln!("{} fix(es) applied", applied.applied);
    }
    eprintln!("{} file(s) changed", applied.changed);
    if let Some(base) = &base {
        applied.keep_new(&session.db, base)?;
    }
    let report = Report {
        format: args.report.format,
        collapse: true,
        new: base.is_some(),
    };
    let collected = &applied.collected;
    lint::print(&session, &report, &collected.files);
    let unknown: Vec<&String> = wanted
        .iter()
        .filter(|id| !applied.found.contains(*id))
        .collect();
    if unknown.is_empty() {
        return Ok(exit_code(collected));
    }
    let names: Vec<String> = unknown.iter().map(|id| format!("`{id}`")).collect();
    eprintln!(
        "error: no fix has the id{} {} (its line changed, or the fix is gone); {}",
        if names.len() > 1 { "s" } else { "" },
        names.join(", "),
        current_ids(&collected.files)
    );
    Ok(ExitCode::from(2))
}

/// The ids most to show after an unknown id.
const LISTED_IDS: usize = 20;

/// `the fix ids now: ID (rule, path:line), ...`.
fn current_ids(files: &[FileDiagnostics]) -> String {
    let mut listed = Vec::new();
    let mut total = 0;
    for file in files {
        let lines = LineIndex::new(&file.text);
        let ids = fix_ids(&file.path, &file.text, &lines, &file.diagnostics);
        for (d, ids) in file.diagnostics.iter().zip(ids) {
            for id in ids.into_iter().flatten() {
                total += 1;
                if listed.len() < LISTED_IDS {
                    let line = lines.line(d.range.start()) + 1;
                    listed.push(format!("{id} ({}, {}:{line})", d.rule, file.path));
                }
            }
        }
    }
    match total {
        0 => "no fix has an id now".into(),
        n if n > listed.len() => format!(
            "the fix ids now: {}, ... ({} more)",
            listed.join(", "),
            n - listed.len()
        ),
        _ => format!("the fix ids now: {}", listed.join(", ")),
    }
}
