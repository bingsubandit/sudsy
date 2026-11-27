//! Where the file of a name or an import lives: one [`Locator`] per package system (Composer's
//! autoload rules, a list of import path roots), built from the manifest files of a project and
//! answering by path arithmetic and a file-exists check, never by reading source. The answer says
//! whether the locator is complete for what was asked: only then does "no such file" mean the
//! name is absent; a name outside every known prefix is `Unknown`.
//!
//! concept: package-locator
//! what: Finds the file of a qualified type name or an import from a project's package manifests (Composer autoload, import path roots).
//! tools: PHPStan class.notFound, buf IMPORT_USED
//! reads: file:symbols.rows

mod composer;
mod import_paths;

pub use composer::Composer;
pub use import_paths::ImportPaths;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What a locator knows about where something is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Located {
    /// These files exist and could hold it.
    Files(Vec<PathBuf>),
    /// The locator covers what was asked and no file is there: whatever the locator maps is not
    /// declared in a file of its own.
    Missing,
    /// What was asked is outside what the locator covers (no prefix, no root, no manifest).
    Unknown,
}

/// A package system's way of finding files.
pub trait Locator: Send + Sync {
    /// The files that may declare the type `name` (qualified, as the language writes it).
    fn type_files(&self, name: &str) -> Located;

    /// The file an import path names.
    fn import_file(&self, path: &str) -> Located;

    /// Files that declare names without a name-to-path rule (autoloaded function files, class
    /// maps). Read when a name is not found elsewhere.
    fn autoloaded(&self) -> Vec<PathBuf>;

    /// The folder the installed dependencies live in: its files are read on demand, so a project
    /// does not count them as its own.
    fn dependency_dir(&self) -> Option<PathBuf> {
        None
    }
}

/// The locator of the package system `system` (the `packages` fact of a language's rows) for the
/// project at `root`; `None` for a system this build does not know.
pub fn open(system: &str, root: &Path) -> Option<Arc<dyn Locator>> {
    match system {
        "composer" => Some(Arc::new(Composer::open(root))),
        "import-paths" => Some(Arc::new(ImportPaths::new(vec![root.to_path_buf()], true))),
        _ => None,
    }
}

/// The file `rel` below `dir`, spelled as on disk: a name that PHP compares without case finds
/// its file on a case-sensitive disk (Linux), and on a case-insensitive one (Windows, macOS) the
/// answer is the on-disk spelling, not the one asked for, so one file is one path.
pub(crate) fn existing(dir: &Path, rel: &str) -> Option<PathBuf> {
    let path = dir.join(rel);
    // A case-sensitive disk only finds the exact spelling, so a hit is already the on-disk one.
    if cfg!(target_os = "linux") && path.is_file() {
        return Some(path);
    }
    let mut at = dir.to_path_buf();
    for part in Path::new(rel).components() {
        let want = part.as_os_str().to_string_lossy();
        let names: Vec<_> = std::fs::read_dir(&at)
            .ok()?
            .flatten()
            .map(|e| e.file_name())
            .collect();
        let found = names
            .iter()
            .find(|n| n.to_string_lossy() == want)
            .or_else(|| {
                names
                    .iter()
                    .find(|n| n.to_string_lossy().eq_ignore_ascii_case(&want))
            })?;
        at.push(found);
    }
    at.is_file().then_some(at)
}
