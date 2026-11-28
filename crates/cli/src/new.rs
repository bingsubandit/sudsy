//! `--new[=REV]` of `lint`, `check` and `fix`: keeps only the findings that the file's text at the
//! git revision REV does not have. That text is read from git and linted in memory with the
//! current config; findings are matched by fingerprint (`core_rules::fingerprint`). Nothing is
//! stored.

use crate::report::FileDiagnostics;
use crate::session::par_map;
use anyhow::{Result, anyhow};
use core_incremental::{Database, File, format, lint, transient};
use core_rules::FORMAT;
use core_rules::fingerprint::new_findings;
use core_workspace::git::{Changes, GitError, Repo, State};
use std::path::{Path, PathBuf};

/// The revision the findings are compared with.
pub struct Base {
    repo: Repo,
    rev: String,
    changes: Changes,
}

impl Base {
    /// Fails outside a git repository and for an unknown revision.
    pub fn open(rev: &str) -> Result<Base> {
        let dir = std::env::current_dir()?;
        let repo = Repo::discover(&dir).map_err(|e| anyhow!("--new compares with git: {e}"))?;
        Base::at(repo, rev).map_err(|e| anyhow!("--new: {e}"))
    }

    /// `rev` of `repo`.
    pub fn at(repo: Repo, rev: &str) -> Result<Base, GitError> {
        let changes = repo.changes(rev)?;
        Ok(Base {
            repo,
            rev: rev.into(),
            changes,
        })
    }

    /// The files of the working tree that differ from the revision: changed ones and those git
    /// does not track (ignored ones left out), sorted.
    pub fn worktree_files(&self) -> Result<Vec<PathBuf>, GitError> {
        let mut rels: Vec<String> = self.changes.changed().map(String::from).collect();
        rels.extend(self.repo.untracked()?);
        rels.sort();
        rels.dedup();
        Ok(rels
            .iter()
            .map(|rel| self.repo.path(rel))
            .filter(|path| path.is_file())
            .collect())
    }
}

/// A file of the report and the database file its findings were found on.
pub struct Entry<'a> {
    pub path: &'a Path,
    pub file: File,
    pub report: &'a mut FileDiagnostics,
}

/// Which findings of `report` are new against `text`, the file at the base (`None`: not there).
/// `check`'s `format` finding is new when the base was formatted.
fn compare(
    db: &Database,
    file: File,
    text: Option<&String>,
    report: &FileDiagnostics,
) -> Vec<bool> {
    let all = || vec![true; report.diagnostics.len()];
    let Some(text) = text else { return all() };
    let at = transient(db, file, text);
    let Ok(linted) = lint(db, at).as_ref() else {
        return all();
    };
    let mut new = new_findings(
        &report.text,
        &report.diagnostics,
        Some((text.as_str(), linted.diagnostics.as_slice())),
    );
    if let Some(i) = report.diagnostics.iter().position(|d| d.rule == FORMAT) {
        new[i] = linted.parses && format(db, at).as_ref().is_ok_and(|f| f == text);
    }
    new
}

/// Drops the findings of `entries` that the base has. A file the base does not have keeps all;
/// an unchanged one keeps none.
pub fn keep_new(db: &Database, base: &Base, entries: Vec<Entry>) -> Result<()> {
    let rels: Vec<Option<String>> = entries.iter().map(|e| base.repo.relative(e.path)).collect();
    let states: Vec<State> = rels
        .iter()
        .map(|rel| base.changes.state(rel.as_deref()))
        .collect();
    let changed: Vec<&str> = rels
        .iter()
        .zip(&states)
        .filter(|(_, s)| **s == State::Changed)
        .filter_map(|(rel, _)| rel.as_deref())
        .collect();
    let mut blobs = base.repo.read_at(&base.rev, &changed)?.into_iter();
    let jobs: Vec<(&Entry, State, Option<String>)> = entries
        .iter()
        .zip(states)
        .map(|(e, state)| {
            let text = (state == State::Changed)
                .then(|| blobs.next().flatten())
                .flatten()
                .and_then(|bytes| {
                    let source = provider_api::SourceFile::from_bytes(&e.report.path, 1, bytes);
                    source.ok().map(|s| s.text)
                });
            (e, state, text)
        })
        .collect();
    let keep: Vec<Vec<bool>> = par_map(db, &jobs, |db, (e, state, text)| match state {
        State::Unchanged => vec![false; e.report.diagnostics.len()],
        _ => compare(db, e.file, text.as_ref(), e.report),
    });
    drop(jobs);
    for (entry, keep) in entries.into_iter().zip(keep) {
        let mut keep = keep.into_iter();
        entry
            .report
            .diagnostics
            .retain(|_| keep.next().unwrap_or(true));
    }
    Ok(())
}
