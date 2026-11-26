//! Directory scope: the project phase of the rules that judge the files of one directory as a
//! unit (`Rule::scope` is `Scope::Directory`). A file's facts are a query of its own text, the
//! unit is an interned list of its files, and the unit's findings are a query over both, so an
//! edit, a new file or a deleted file re-runs only the facts of the file that changed and the one
//! `finish` of its unit.
//!
//! concept: none (infrastructure: directory scope)

use crate::db::Db;
use crate::failure::{Failure, Stage, guarded};
use crate::inputs::File;
use crate::queries::{parse, provider_of};
use crate::rules::{active_rules, generated_skipped};
use core_rules::suppress::Suppressions;
use core_rules::{Diagnostic, DirectoryScope, FileContext, LineIndex, ProjectFacts};
use core_rules::{collect_directory, collect_nothing, directory_rules, run_directory};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// The files of one directory's unit, in path order, and whether the directory holds files of the
/// unit that were not read (`DirectoryScope::opaque`, or a file that could not be read).
#[salsa::interned(debug)]
pub struct Unit<'db> {
    #[returns(ref)]
    pub files: Vec<File>,
    #[returns(copy)]
    pub opaque: bool,
}

/// The directory scope of `file`, when one of its active rules has one and the file belongs to it.
/// Reads the file's active rules only: with no directory rule on, nothing else is done.
pub fn directory_scope(db: &dyn Db, file: File) -> Option<DirectoryScope> {
    let (active, _) = active_rules(db, file).as_ref().ok()?;
    let path = file.path(db);
    directory_rules(active)
        .map(|(_, scope)| scope)
        .find(|scope| scope.is_member(path))
}

/// Runs `f` on the file's tree, the way the project phase sees it; `None` when the file has
/// syntax errors.
fn with_context<R>(
    db: &dyn Db,
    file: File,
    f: impl FnOnce(&FileContext<'_>) -> R,
) -> Result<Option<R>, Failure> {
    let provider = provider_of(db, file)?;
    let parsed = parse(db, file).as_ref().map_err(Clone::clone)?;
    if !parsed.errors.is_empty() {
        return Ok(None);
    }
    let text = file.text(db);
    let root = parsed.root();
    let lines = LineIndex::new(text);
    let context = FileContext {
        path: file.path(db),
        text,
        root: &root,
        grammar: provider.grammar(),
        lines: &lines,
        body_kinds: provider.body_kinds(),
        hooks: provider.rule_hooks(),
        embedded: &[],
        cells: &[],
        semantics: None,
    };
    Ok(Some(f(&context)))
}

/// What the directory rules collect from one file. Empty (and so "not collected") when the file
/// has syntax errors.
#[salsa::tracked(returns(ref))]
pub fn module_facts(db: &dyn Db, file: File) -> Result<ProjectFacts, Failure> {
    guarded(Stage::Lint, || {
        let (active, _) = active_rules(db, file).as_ref().map_err(Clone::clone)?;
        if directory_rules(active).next().is_none() {
            return Ok(ProjectFacts::default());
        }
        if *generated_skipped(db, file) {
            return Ok(collect_nothing(file.path(db), active));
        }
        let facts = with_context(db, file, |context| collect_directory(context, active))?;
        Ok(facts.unwrap_or_default())
    })
}

/// Drops the findings that a suppression comment of the file they are reported in silences: the
/// project and directory phases report on files other than the one whose rules ran.
pub(crate) fn silenced(
    db: &dyn Db,
    found: Vec<(PathBuf, Diagnostic)>,
    files: &[File],
) -> Result<Vec<(PathBuf, Diagnostic)>, Failure> {
    let mut keep = vec![true; found.len()];
    let mut done: Vec<&PathBuf> = Vec::new();
    for (path, _) in &found {
        if done.contains(&path) {
            continue;
        }
        done.push(path);
        let Some(&file) = files.iter().find(|f| f.path(db) == path) else {
            continue;
        };
        let silenced = with_context(db, file, |context| {
            let suppressions = Suppressions::scan(context);
            found
                .iter()
                .map(|(p, d)| p == path && suppressions.suppresses(context, d))
                .collect::<Vec<_>>()
        })?;
        for (k, silenced) in keep.iter_mut().zip(silenced.unwrap_or_default()) {
            *k &= !silenced;
        }
    }
    let mut keep = keep.into_iter();
    let mut found = found;
    found.retain(|_| keep.next().unwrap_or(true));
    Ok(found)
}

/// What the directory rules find in a unit, on any of its files; the caller keeps the files it
/// was asked about. The rules and their options are those of the unit's first file.
#[salsa::tracked(returns(ref))]
pub fn module_diagnostics<'db>(
    db: &'db dyn Db,
    unit: Unit<'db>,
) -> Result<Vec<(PathBuf, Diagnostic)>, Failure> {
    guarded(Stage::Lint, || {
        let files = unit.files(db);
        let Some(&first) = files.first() else {
            return Ok(Vec::new());
        };
        let (active, _) = active_rules(db, first).as_ref().map_err(Clone::clone)?;
        let mut facts = ProjectFacts::default();
        for &file in files {
            if let Ok(found) = module_facts(db, file) {
                facts.extend(found);
            }
        }
        let paths: Vec<PathBuf> = files.iter().map(|f| f.path(db).clone()).collect();
        silenced(
            db,
            run_directory(active, &facts, &paths, unit.opaque(db)),
            files,
        )
    })
}

/// The files of units read from disk, kept between requests: a file is read again when its
/// modification time or size changed. Shared by the threads of an editor session.
#[derive(Default)]
pub struct Siblings {
    cache: Mutex<HashMap<PathBuf, Read>>,
}

/// A file read from disk: its modification time and size when it was read, and its input.
type Read = (Option<SystemTime>, u64, File);

impl Siblings {
    /// The unit of `dir` as the disk has it now plus `own`, the files of the directory the caller
    /// already holds (open documents, files of the run): they win over the disk and count even
    /// when the disk has no such file (an unsaved document).
    pub fn unit<'db>(
        &self,
        db: &'db dyn Db,
        dir: &Path,
        scope: &DirectoryScope,
        own: &[File],
    ) -> Unit<'db> {
        let mut paths: Vec<PathBuf> = Vec::new();
        let mut opaque = false;
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if scope.is_opaque(&path) {
                        opaque = true;
                    } else if scope.is_member(&path) && path.is_file() {
                        paths.push(path);
                    }
                }
            }
            Err(_) => opaque = true,
        }
        paths.extend(
            own.iter()
                .map(|f| f.path(db).clone())
                .filter(|p| scope.is_member(p)),
        );
        paths.sort();
        paths.dedup();
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let mut files = Vec::with_capacity(paths.len());
        for path in &paths {
            if let Some(&file) = own.iter().find(|f| f.path(db) == path) {
                files.push(file);
                continue;
            }
            let stamp = std::fs::metadata(path)
                .ok()
                .map(|m| (m.modified().ok(), m.len()));
            match (stamp, cache.get(path)) {
                (Some((time, len)), Some(&(t, l, file))) if t == time && l == len => {
                    files.push(file)
                }
                (Some((time, len)), _) => {
                    let read = std::fs::read(path).ok().and_then(|bytes| {
                        File::from_bytes(db, path.clone(), path.to_string_lossy(), 1, bytes).ok()
                    });
                    match read {
                        Some(file) => {
                            cache.insert(path.clone(), (time, len, file));
                            files.push(file);
                        }
                        None => opaque = true,
                    }
                }
                (None, _) => opaque = true,
            }
        }
        cache.retain(|path, _| path.parent() != Some(dir) || paths.contains(path));
        Unit::new(db, files, opaque)
    }
}
