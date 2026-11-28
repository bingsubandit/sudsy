//! `sudsy.diagnostics.scope: new` (the `diagnosticsScope` initialization option): a document shows
//! only the findings its text at `HEAD` does not have, the comparison of the CLI's `--new`. The
//! text at `HEAD` is read once per file and kept, as a database file, while `HEAD` stays.

use core_incremental::{Database, File, transient};
use core_workspace::git::Repo;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

type Stamp = Option<(SystemTime, u64)>;

/// The files at `HEAD` of the workspace's repository.
pub struct HeadFiles {
    /// `None` outside a repository: every finding is new.
    repo: Option<Repo>,
    /// Per path: the `HEAD` it was read at and the file there (`None`: not in `HEAD`).
    files: Mutex<HashMap<PathBuf, (Stamp, Option<File>)>>,
}

impl HeadFiles {
    pub fn new(root: &Path) -> HeadFiles {
        HeadFiles {
            repo: Repo::discover(root).ok(),
            files: Mutex::default(),
        }
    }

    /// The file `like` (at `path`) as `HEAD` has it; `None` when `HEAD` does not have it.
    pub fn at_head(&self, db: &Database, path: &Path, like: File) -> Option<File> {
        let repo = self.repo.as_ref()?;
        let stamp = repo.head_stamp();
        let mut files = self.files.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, file)) = files.get(path)
            && *at == stamp
        {
            return *file;
        }
        let file = repo
            .relative(path)
            .and_then(|rel| {
                let bytes = repo.read_at("HEAD", &[&rel]).ok()?.pop()??;
                provider_api::SourceFile::from_bytes(path.to_string_lossy(), 1, bytes).ok()
            })
            .map(|source| transient(db, like, &source.text));
        files.insert(path.to_path_buf(), (stamp, file));
        file
    }
}
