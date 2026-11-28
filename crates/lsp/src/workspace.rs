//! The state behind the server: the incremental database with the project's config and the open
//! documents. Knows nothing about the transport, so the handlers in the sibling modules and the
//! tests use it directly.

use crate::position::apply_change;
use core_config::{ConfigError, EditorConfig};
use core_incremental::{
    Database, Db, File, Siblings, is_manifest, is_source, manifest_changed, manifests,
    open_projects, reads_index, source_globs,
};
use languages::{Project, installed::Checker};
use lsp_types::TextDocumentContentChangeEvent;
use provider_api::LanguageProvider;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
struct Doc {
    file: File,
    /// Open in the editor but its language is not installed: kept (and kept in sync) so that
    /// installing the language can start serving it, invisible to every handler until then.
    skipped: bool,
}

/// `Clone` makes a snapshot for a job on the analysis thread: it shares the database's memos and
/// the project; the documents are the inputs as they were.
#[derive(Clone)]
pub struct Workspace {
    pub(crate) db: Database,
    root: PathBuf,
    /// The project of the last successful config load.
    project: Option<Arc<Project>>,
    docs: HashMap<PathBuf, Doc>,
    /// The project's other files, read from disk when the first `deep` diagnostics are asked for
    /// (`None` before that). They give the project-scope rules every file; an edit in the editor
    /// takes a file out of here and into `docs`.
    background: Option<Arc<HashMap<PathBuf, File>>>,
    /// The files of a directory rule's unit that are not open, read from disk and kept while
    /// their modification time and size stay the same.
    pub(crate) siblings: Arc<Siblings>,
    /// Set once an active rule reads the symbol index: the projects are opened and follow the
    /// files (`refresh_index`). Until then the workspace opens no project.
    index_open: bool,
    /// The files that make up the projects changed since the last `refresh_index`.
    index_dirty: bool,
    /// Source files that changed on disk while not open, applied by the next `refresh_index`.
    changed_sources: Vec<PathBuf>,
    /// Set by the `new` diagnostics scope: the files at `HEAD` the findings are compared with.
    pub(crate) head: Option<Arc<crate::scope::HeadFiles>>,
}

impl Workspace {
    /// A workspace rooted at `root`. The second value is why the config could not be loaded, if
    /// it could not: every file then fails with that reason until the config is fixed.
    pub fn new(
        root: PathBuf,
        providers: Vec<Box<dyn LanguageProvider>>,
    ) -> (Workspace, Option<String>) {
        let mut workspace = Workspace {
            db: Database::new(providers),
            root,
            project: None,
            docs: HashMap::new(),
            background: None,
            siblings: Arc::default(),
            index_open: false,
            index_dirty: false,
            changed_sources: Vec::new(),
            head: None,
        };
        workspace.db.prestart();
        let error = workspace.reload_config();
        (workspace, error)
    }

    /// Tells the semantic providers which files are the project: those the walk visits, so not
    /// what `.gitignore` or `exclude` leave out.
    fn open_semantic_workspace(&self, project: &Project) {
        let roots = [self.root.clone()];
        let found = core_workspace::discover_files(&roots, &project.filter()).unwrap_or_default();
        self.db.open_project(&roots, &found);
    }

    /// `new`: documents show only the findings their text at `HEAD` does not have.
    pub fn set_new_scope(&mut self, new: bool) {
        self.head = new.then(|| Arc::new(crate::scope::HeadFiles::new(&self.root)));
    }

    /// Reads the config and the `.editorconfig` values again, for every open document. Returns
    /// the reason when the config is invalid.
    pub fn reload_config(&mut self) -> Option<String> {
        let loaded = Project::load(&self.root, core_incremental::Db::providers(&self.db));
        let error = match loaded {
            Ok(project) => {
                self.open_semantic_workspace(&project);
                self.db.set_config(Ok(project.chain.clone()));
                self.db.set_extensions(project.extensions.clone());
                self.project = Some(Arc::new(project));
                None
            }
            Err(e) => {
                let roots = [self.root.clone()];
                let all = core_workspace::Filter::none();
                let found = core_workspace::discover_files(&roots, &all).unwrap_or_default();
                self.db.open_project(&roots, &found);
                self.project = None;
                self.db.set_config(Err(ConfigError::new(e.0.clone())));
                self.db.set_extensions(Default::default());
                Some(e.0)
            }
        };
        self.background = None;
        self.index_dirty = true;
        for (path, doc) in &self.docs {
            self.db
                .set_file_editorconfig(doc.file, EditorConfig::for_file(path));
        }
        error
    }

    /// Does a change to this file affect the config: a config file name, `.editorconfig`, or a
    /// file the current config extends.
    pub fn is_config_file(&self, path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        core_config::CONFIG_FILE_NAMES.contains(&name)
            || name == ".editorconfig"
            || self.project.as_ref().is_some_and(|p| {
                p.chain
                    .files
                    .iter()
                    .any(|f| f.file.as_deref() == Some(path))
                    || p.extensions.files.iter().any(|f| f == path)
            })
    }

    /// `(language, why)` when `path` is a known language that is not installed.
    pub fn missing_language(&self, path: &Path) -> Option<(String, String)> {
        let (language, why) = Checker::new(self.db.providers()).missing(path)?;
        Some((language.to_string(), why))
    }

    #[cfg(feature = "pack-mode")]
    /// The languages of the open documents that are not installed and not ignored, sorted.
    pub fn missing_open_languages(&self) -> Vec<String> {
        let missing: std::collections::BTreeSet<String> = (self.docs.keys())
            .filter_map(|p| self.missing_language(p))
            .map(|(language, _)| language)
            .filter(|l| !self.ignores_language(l))
            .collect();
        missing.into_iter().collect()
    }

    #[cfg(feature = "pack-mode")]
    /// Whether the project's config says files of `language` are skipped without a word.
    pub fn ignores_language(&self, language: &str) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| p.chain.needs().get(language) == Some(&core_config::Need::Ignored))
    }

    #[cfg(feature = "pack-mode")]
    /// Serves the documents that were waiting for a language that is installed now.
    pub fn activate_installed(&mut self) {
        let waiting: Vec<PathBuf> = (self.docs.iter())
            .filter(|(_, d)| d.skipped)
            .map(|(p, _)| p.clone())
            .collect();
        let mut checker = Checker::new(self.db.providers());
        let ready: Vec<PathBuf> = (waiting.into_iter())
            .filter(|p| checker.missing(p).is_none())
            .collect();
        for path in ready {
            if let Some(doc) = self.docs.get_mut(&path) {
                doc.skipped = false;
            }
        }
    }

    /// Starts tracking a document; false when no provider handles the file or its language is
    /// not installed (the document is then only kept for [`Workspace::activate_installed`]).
    pub fn open(&mut self, path: PathBuf, version: i32, text: String) -> bool {
        // A file the config excludes (a package cache, generated code) gets no diagnostics and no
        // formatting, though the editor opened it.
        if !self.db.supports(&path) || self.project.as_ref().is_some_and(|p| p.is_excluded(&path)) {
            return false;
        }
        let skipped = self.missing_language(&path).is_some();
        let label = path.to_string_lossy().into_owned();
        let Ok(file) = File::from_bytes(&self.db, path.clone(), label, version as u64, text.into())
        else {
            return false;
        };
        if let Some(background) = &mut self.background {
            Arc::make_mut(background).remove(&path);
        }
        self.docs.insert(path, Doc { file, skipped });
        self.index_dirty = true;
        !skipped
    }

    /// Applies the changes of one `didChange`, in order. `shared`: other threads hold handles of
    /// the database. Setting an input would then wait for them (a semantic request can take
    /// seconds), so the document gets a new input instead; the old one stays for them.
    pub fn change(
        &mut self,
        path: &Path,
        version: i32,
        changes: &[TextDocumentContentChangeEvent],
        shared: bool,
    ) {
        let Some(doc) = self.docs.get(path) else {
            return;
        };
        let mut text = doc.file.text(&self.db).clone();
        for change in changes {
            apply_change(&mut text, change.range, &change.text);
        }
        let (file, skipped) = (doc.file, doc.skipped);
        if shared {
            let new = File::new(
                &self.db,
                path.to_path_buf(),
                version as u64,
                text,
                file.had_bom(&self.db),
                file.editorconfig(&self.db).clone(),
            );
            self.docs
                .insert(path.to_path_buf(), Doc { file: new, skipped });
            self.index_dirty = true;
        } else {
            self.db.set_file_text(file, version as u64, text);
        }
    }

    pub fn close(&mut self, path: &Path) {
        self.index_dirty = true;
        if self.docs.remove(path).is_some_and(|d| !d.skipped)
            && let Some(file) = self.background.is_some().then(|| self.read(path)).flatten()
            && let Some(background) = &mut self.background
        {
            Arc::make_mut(background).insert(path.to_path_buf(), file);
        }
    }

    /// Makes the files' semantic answers depend on the edits so far. Setting the input cancels the
    /// queries of other handles: call it while none runs.
    pub fn bump_revision(&mut self) {
        self.db.bump_workspace_revision();
    }

    /// Opens the projects the index rules read, or keeps them in step with the documents. Opens
    /// none while no active rule of an open document reads the index. Sets inputs: call it while
    /// no request runs.
    pub fn refresh_index(&mut self) {
        if !self.index_dirty {
            return;
        }
        if !self.index_open {
            let wanted = self
                .docs
                .values()
                .filter(|d| !d.skipped)
                .any(|d| reads_index(&self.db, d.file));
            if !wanted {
                return;
            }
            self.index_open = true;
        }
        self.index_dirty = false;
        if !self.changed_sources.is_empty() {
            self.reread_sources();
            manifest_changed(&mut self.db);
        }
        self.load_project();
        let mut files: Vec<File> = (self.docs.values())
            .filter(|d| !d.skipped)
            .map(|d| d.file)
            .collect();
        if let Some(background) = &self.background {
            files.extend(background.values().copied());
        }
        open_projects(&mut self.db, &files, std::slice::from_ref(&self.root));
    }

    /// Whether the workspace opened its projects for the index rules.
    pub fn index_open(&self) -> bool {
        self.index_open
    }

    /// A watched file changed: when it is a package manifest, the projects find their packages
    /// again. True when it was one.
    pub fn manifest_changed(&mut self, path: &Path) -> bool {
        if !is_manifest(&self.db, path) {
            return false;
        }
        if self.index_open {
            manifest_changed(&mut self.db);
            self.index_dirty = true;
        }
        true
    }

    /// A watched file changed: when it is a source file of a language with a package system that
    /// is not open, it is read again at the next `refresh_index` (created, changed or deleted by
    /// something else: a checkout, a generator). True when it was one.
    pub fn source_changed(&mut self, path: &Path) -> bool {
        if !self.index_open || self.docs.contains_key(path) || !is_source(&self.db, path) {
            return false;
        }
        self.changed_sources.push(path.to_path_buf());
        self.index_dirty = true;
        true
    }

    /// Globs of the manifests of the languages with a package system.
    pub fn manifest_globs(&self) -> Vec<String> {
        manifests(&self.db)
            .into_iter()
            .map(|m| format!("**/{m}"))
            .collect()
    }

    /// File name globs of the sources `source_changed` takes.
    pub fn source_globs(&self) -> Vec<String> {
        source_globs(&self.db)
    }

    /// The background files after the changes on disk: a walk finds created and deleted files,
    /// the changed ones are read again, the rest keep their inputs.
    fn reread_sources(&mut self) {
        let changed = std::mem::take(&mut self.changed_sources);
        let (Some(old), Some(project)) = (self.background.take(), self.project.clone()) else {
            return;
        };
        let paths =
            core_workspace::discover_files(std::slice::from_ref(&self.root), &project.filter())
                .unwrap_or_default();
        let mut checker = Checker::new(self.db.providers());
        let mut background = HashMap::new();
        for path in paths {
            if !self.db.supports(&path) || self.docs.contains_key(&path) {
                continue;
            }
            let kept = old.get(&path).copied().filter(|_| !changed.contains(&path));
            let file = kept.or_else(|| {
                checker
                    .missing(&path)
                    .is_none()
                    .then(|| self.read(&path))
                    .flatten()
            });
            if let Some(file) = file {
                background.insert(path, file);
            }
        }
        self.background = Some(Arc::new(background));
    }

    /// Does the config ask for `analysis: deep`?
    pub fn is_deep(&self) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| p.chain.depth() == core_config::Depth::Deep)
    }

    /// Reads the project's files that are not open, once, for the project-scope rules.
    pub fn load_project(&mut self) {
        if self.background.is_some() {
            return;
        }
        let Some(project) = &self.project else { return };
        let paths =
            core_workspace::discover_files(std::slice::from_ref(&self.root), &project.filter())
                .unwrap_or_default();
        let mut background = HashMap::new();
        let mut checker = Checker::new(self.db.providers());
        for path in paths {
            if self.db.supports(&path)
                && checker.missing(&path).is_none()
                && !self.docs.contains_key(&path)
                && let Some(file) = self.read(&path)
            {
                background.insert(path, file);
            }
        }
        self.background = Some(Arc::new(background));
    }

    fn read(&self, path: &Path) -> Option<File> {
        let bytes = std::fs::read(path).ok()?;
        File::from_bytes(
            &self.db,
            path.to_path_buf(),
            path.to_string_lossy(),
            1,
            bytes,
        )
        .ok()
    }

    /// Every file the project-scope rules see: the open documents and the loaded rest.
    pub(crate) fn project_files(&self) -> Vec<File> {
        self.docs
            .values()
            .filter(|d| !d.skipped)
            .map(|d| d.file)
            .chain(self.background.iter().flat_map(|b| b.values().copied()))
            .collect()
    }

    /// The open documents directly in `dir`.
    pub(crate) fn project_open_in(&self, dir: &Path) -> Vec<File> {
        self.docs
            .iter()
            .filter(|(p, d)| !d.skipped && p.parent() == Some(dir))
            .map(|(_, d)| d.file)
            .collect()
    }

    /// The input of an open document.
    pub(crate) fn file(&self, path: &Path) -> Option<File> {
        self.docs.get(path).filter(|d| !d.skipped).map(|d| d.file)
    }

    pub fn text(&self, path: &Path) -> Option<String> {
        self.file(path).map(|f| f.text(&self.db).clone())
    }
}
