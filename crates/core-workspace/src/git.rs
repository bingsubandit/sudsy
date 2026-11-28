//! The files of a git repository at a revision, read through the `git` executable: which files
//! differ from the working tree, and their text there. `--new` and the editor's `new` scope compare
//! findings with these texts.
//!
//! concept: none (infrastructure: git base revision)

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("{} is not in a git repository", .0.display())]
    NotARepository(PathBuf),
    #[error("`{0}` is not a revision of this repository")]
    BadRevision(String),
    #[error("running git: {0}")]
    Run(String),
}

/// A working tree and its repository.
#[derive(Clone, Debug)]
pub struct Repo {
    /// The top of the working tree, canonical.
    top: PathBuf,
    /// The repository's own directory (a worktree's, for a linked worktree).
    git_dir: PathBuf,
}

/// Where a file of the working tree stands against a revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Not in the revision (untracked, added since, or outside the repository).
    Absent,
    /// The same as in the revision.
    Unchanged,
    /// In the revision with other content.
    Changed,
}

/// The files of a revision and those the working tree changed, as paths relative to the top.
pub struct Changes {
    in_rev: HashSet<String>,
    changed: HashSet<String>,
}

impl Changes {
    pub fn state(&self, rel: Option<&str>) -> State {
        match rel {
            Some(rel) if self.in_rev.contains(rel) => {
                if self.changed.contains(rel) {
                    State::Changed
                } else {
                    State::Unchanged
                }
            }
            _ => State::Absent,
        }
    }
}

fn git(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    command
}

/// Runs `command`; its stdout, or the first line of its stderr as the error.
fn output(mut command: Command) -> Result<Vec<u8>, String> {
    let out = command.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(out.stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    Err(stderr.lines().next().unwrap_or("failed").to_string())
}

/// The NUL-separated names of `out`.
fn names(out: &[u8]) -> HashSet<String> {
    out.split(|&b| b == 0)
        .filter(|n| !n.is_empty())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .collect()
}

impl Repo {
    /// The repository that holds `dir`.
    pub fn discover(dir: &Path) -> Result<Repo, GitError> {
        let mut command = git(dir);
        command.args(["rev-parse", "--show-toplevel", "--absolute-git-dir"]);
        let out = match command.output() {
            Ok(out) => out,
            Err(e) => return Err(GitError::Run(e.to_string())),
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines();
        match (out.status.success(), lines.next(), lines.next()) {
            (true, Some(top), Some(git_dir)) => Ok(Repo {
                top: std::fs::canonicalize(top).unwrap_or_else(|_| top.into()),
                git_dir: git_dir.into(),
            }),
            _ => Err(GitError::NotARepository(dir.to_path_buf())),
        }
    }

    /// `path` relative to the top of the working tree, with `/` separators; `None` outside it.
    pub fn relative(&self, path: &Path) -> Option<String> {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let rel = path.strip_prefix(&self.top).ok()?;
        let parts: Vec<_> = rel.components().map(|c| c.as_os_str().to_str()).collect();
        Some(parts.into_iter().collect::<Option<Vec<_>>>()?.join("/"))
    }

    /// The files of `rev` and those of them that differ in the working tree (staged or not).
    pub fn changes(&self, rev: &str) -> Result<Changes, GitError> {
        let mut verify = git(&self.top);
        verify.args(["rev-parse", "--verify", "--quiet", "--end-of-options"]);
        verify.arg(format!("{rev}^{{commit}}"));
        if output(verify).is_err() {
            return Err(GitError::BadRevision(rev.into()));
        }
        let mut tree = git(&self.top);
        tree.args(["ls-tree", "-r", "-z", "--full-tree", "--name-only", rev]);
        let mut diff = git(&self.top);
        diff.args(["diff", "--name-only", "-z", "--no-renames", rev, "--"]);
        Ok(Changes {
            in_rev: names(&output(tree).map_err(GitError::Run)?),
            changed: names(&output(diff).map_err(GitError::Run)?),
        })
    }

    /// The bytes of the files `rels` at `rev`, `None` for one it does not have; one git process.
    pub fn read_at(&self, rev: &str, rels: &[&str]) -> Result<Vec<Option<Vec<u8>>>, GitError> {
        if rels.is_empty() {
            return Ok(Vec::new());
        }
        let run = |e: std::io::Error| GitError::Run(e.to_string());
        let mut child = git(&self.top)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(run)?;
        let requests: Vec<u8> = rels
            .iter()
            .flat_map(|rel| format!("{rev}:{rel}\n").into_bytes())
            .collect();
        let mut stdin = child.stdin.take().expect("piped");
        // Written from another thread: git answers while it reads, and a full pipe would block.
        let writer = std::thread::spawn(move || stdin.write_all(&requests));
        let mut reader = BufReader::new(child.stdout.take().expect("piped"));
        let mut out = Vec::with_capacity(rels.len());
        let mut header = String::new();
        for _ in rels {
            header.clear();
            reader.read_line(&mut header).map_err(run)?;
            let size = header
                .trim_end()
                .rsplit(' ')
                .next()
                .and_then(|s| s.parse::<usize>().ok());
            match size {
                Some(size) if header.split(' ').nth(1) == Some("blob") => {
                    let mut bytes = vec![0; size + 1];
                    reader.read_exact(&mut bytes).map_err(run)?;
                    bytes.pop();
                    out.push(Some(bytes));
                }
                _ => out.push(None),
            }
        }
        drop(reader);
        let _ = writer.join();
        child.wait().map_err(run)?;
        Ok(out)
    }

    /// Changes whenever `HEAD` moves (a commit, checkout or reset appends to its log), so a cache
    /// of texts at `HEAD` is kept while it stays the same.
    pub fn head_stamp(&self) -> Option<(SystemTime, u64)> {
        let log = std::fs::metadata(self.git_dir.join("logs").join("HEAD"))
            .or_else(|_| std::fs::metadata(self.git_dir.join("HEAD")))
            .ok()?;
        Some((log.modified().ok()?, log.len()))
    }
}
