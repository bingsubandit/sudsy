//! The database: salsa storage plus the (immutable) provider list.
//!
//! concept: none (infrastructure: salsa database)

use crate::inputs::{ConfigFile, ExtensionSet, File, ProjectEntry, Projects, WorkspaceRevision};
use core_config::{ConfigChain, ConfigError, EditorConfig};
use provider_api::{LanguageProvider, select_provider_index};
use salsa::Setter;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

pub type Providers = Arc<[Box<dyn LanguageProvider>]>;

/// What queries need besides inputs. The providers are part of the environment rather than an
/// input: the set is fixed for the life of a database. Pattern and plugin rules, which change
/// with the config, are the [`ExtensionSet`] input.
#[salsa::db]
pub trait Db: salsa::Database {
    fn providers(&self) -> &[Box<dyn LanguageProvider>];

    /// The file at `path` read from disk when first asked for, and again when its size or
    /// modification time changed (dependencies of a project: nothing lints them). `None` when it
    /// cannot be read as source text.
    fn dependency_file(&self, path: &Path) -> Option<File>;
}

/// Files read on demand, by canonical path (one file under several spellings, such as names that
/// differ only by case on a case-insensitive file system or a symlink, is one input), with the
/// stamp they were read at, and the canonical path of each spelling asked for.
#[derive(Default)]
struct Dependencies(Mutex<Spellings>);

#[derive(Default)]
struct Spellings {
    canonical: HashMap<std::path::PathBuf, std::path::PathBuf>,
    files: HashMap<std::path::PathBuf, Read>,
}

/// A file read from disk: its modification time and size then, and its input (`None`: not source text).
type Read = (Option<SystemTime>, u64, Option<File>);

/// Callback for salsa's events (`WillExecute`, `DidValidateMemoizedValue`, ...). Tests use it to
/// count query executions.
pub type EventHook = Box<dyn Fn(salsa::Event) + Send + Sync>;

/// One database per run (CLI) or per workspace (LSP). `Clone` makes a handle that shares the
/// memos; each thread works through its own handle. Setting an input needs `&mut` access to a
/// handle and waits until every other handle is dropped, so inputs are set while a single
/// handle exists.
#[salsa::db]
#[derive(Clone)]
pub struct Database {
    storage: salsa::Storage<Self>,
    providers: Providers,
    dependencies: Arc<Dependencies>,
}

#[salsa::db]
impl salsa::Database for Database {}

#[salsa::db]
impl Db for Database {
    fn providers(&self) -> &[Box<dyn LanguageProvider>] {
        &self.providers
    }

    fn dependency_file(&self, path: &Path) -> Option<File> {
        let meta = std::fs::metadata(path).ok()?;
        let stamp = (meta.modified().ok(), meta.len());
        let mut guard = self
            .dependencies
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let Spellings { canonical, files } = &mut *guard;
        let key = canonical
            .entry(path.to_path_buf())
            .or_insert_with(|| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
            .clone();
        if let Some((time, len, file)) = files.get(&key)
            && (*time, *len) == stamp
        {
            return *file;
        }
        let file = std::fs::read(path).ok().and_then(|bytes| {
            File::from_bytes(self, path.to_path_buf(), path.to_string_lossy(), 1, bytes).ok()
        });
        files.insert(key, (stamp.0, stamp.1, file));
        file
    }
}

impl Database {
    pub fn new(providers: Vec<Box<dyn LanguageProvider>>) -> Self {
        Self::build(providers, None)
    }

    pub fn with_event_hook(providers: Vec<Box<dyn LanguageProvider>>, hook: EventHook) -> Self {
        Self::build(providers, Some(hook))
    }

    fn build(providers: Vec<Box<dyn LanguageProvider>>, hook: Option<EventHook>) -> Self {
        let db = Database {
            storage: salsa::Storage::new(hook),
            providers: providers.into(),
            dependencies: Arc::default(),
        };
        ConfigFile::new(&db, Ok(ConfigChain::default()));
        ExtensionSet::new(&db, core_rules::Extensions::default());
        WorkspaceRevision::new(&db, 0);
        Projects::new(&db, Vec::new());
        db
    }

    /// Whether some provider handles `path`. Lets callers skip unsupported files without
    /// reading them.
    pub fn supports(&self, path: &Path) -> bool {
        select_provider_index(&self.providers, path).is_some()
    }

    /// The position of the provider that handles `path`.
    pub fn provider_index(&self, path: &Path) -> Option<usize> {
        select_provider_index(&self.providers, path).map(|(i, _)| i)
    }

    /// Lets the providers start their background processes ahead of the first request.
    pub fn prestart(&self) {
        for provider in self.providers.iter() {
            provider.prestart();
        }
    }

    /// Tells the providers with a semantic model which directories, projects or solutions make up
    /// the project; their files are part of every file's binding.
    pub fn open_workspace(&self, paths: &[std::path::PathBuf]) {
        for provider in self.providers.iter() {
            provider.open_workspace(paths);
        }
    }

    /// [`Database::open_workspace`] for the files a walk found: `found`, the files discovered
    /// under `roots` (so without what the ignore files and `exclude` leave out), those the
    /// provider handles, and the files named in `roots` (a project or solution file).
    pub fn open_project(&self, roots: &[std::path::PathBuf], found: &[std::path::PathBuf]) {
        for provider in self.providers.iter() {
            let matchers = provider.file_matchers();
            let paths: std::collections::BTreeSet<&std::path::PathBuf> = roots
                .iter()
                .filter(|r| r.is_file())
                .chain(
                    found
                        .iter()
                        .filter(|f| matchers.iter().any(|m| m.matches(f))),
                )
                .collect();
            provider.open_workspace(&paths.into_iter().cloned().collect::<Vec<_>>());
        }
    }

    /// An editor edit: the new text of `file` at the editor's document `version`.
    pub fn set_file_text(&mut self, file: File, version: u64, text: String) {
        file.set_text(self).to(text);
        file.set_version(self).to(version);
    }

    /// Some document opened, changed or closed: other files' semantic answers are stale.
    pub fn bump_workspace_revision(&mut self) {
        let input = WorkspaceRevision::get(self);
        let next = input.revision(self) + 1;
        input.set_revision(self).to(next);
    }

    /// The `.editorconfig` values of `file` changed.
    pub fn set_file_editorconfig(&mut self, file: File, editorconfig: EditorConfig) {
        file.set_editorconfig(self).to(editorconfig);
    }

    /// Replaces the pattern and plugin rules (see `core_rules::Extensions`).
    /// An unchanged set (loaders reuse unchanged rule objects) invalidates nothing.
    pub fn set_extensions(&mut self, extensions: core_rules::Extensions) {
        if *ExtensionSet::get(self).extensions(self) == extensions {
            return;
        }
        let _previous = ExtensionSet::get(self).set_extensions(self).to(extensions);
    }

    /// Replaces the projects the index rules read.
    pub fn set_projects(&mut self, list: Vec<ProjectEntry>) {
        let _previous = Projects::get(self).set_list(self).to(list);
    }

    /// Replaces the config files.
    pub fn set_config(&mut self, chain: Result<ConfigChain, ConfigError>) {
        let _previous = ConfigFile::get(self).set_chain(self).to(chain);
    }
}
