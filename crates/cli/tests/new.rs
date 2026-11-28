//! `--new[=REV]`: only the findings the file's text at a git revision does not have.

use super::lint::{err, out, sudsy, write};
use std::path::Path;
use std::process::Command;

pub(crate) fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

/// A repository whose `HEAD` has `a.yaml` (one `yes`, an error) and `b.yaml` (one `yes`).
fn repo() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    ambiguous-booleans: error\n",
    );
    write(t.path(), "a.yaml", "- yes\n- 1\n");
    write(t.path(), "b.yaml", "- yes\n");
    git(t.path(), &["init", "-q"]);
    git(t.path(), &["add", "-A"]);
    git(t.path(), &["commit", "-qm", "base"]);
    t
}

#[test]
fn only_findings_the_revision_lacks_are_reported_and_counted() {
    let t = repo();
    // Unchanged files: nothing to say, exit 0, no summary.
    let o = sudsy(t.path(), &["lint", "--new"]);
    assert_eq!(
        (o.status.code(), out(&o), err(&o)),
        (Some(0), String::new(), String::new())
    );
    // The old `yes` moved: still old. A second equal line and an edited line are new.
    write(t.path(), "a.yaml", "- 0\n- yes\n- 1\n- yes\n- yes # c\n");
    // Not in `HEAD`: every finding is new.
    write(t.path(), "c.yaml", "- yes\n");
    let o = sudsy(t.path(), &["lint", "--new", "--format", "concise"]);
    assert_eq!(o.status.code(), Some(1));
    let report = out(&o);
    let shown: Vec<&str> = report
        .lines()
        .map(|l| l.split(": ").next().unwrap())
        .collect();
    assert_eq!(
        shown,
        ["a.yaml:4:3", "a.yaml:5:3", "c.yaml:1:3"],
        "{report}"
    );
    assert!(
        err(&o).contains("4 file(s) checked, 3 new error(s), 0 new warning(s)"),
        "{}",
        err(&o)
    );
    // Against the revision before the commit that would have them, they are new too.
    git(t.path(), &["commit", "-qam", "next"]);
    let o = sudsy(
        t.path(),
        &["lint", "--new=HEAD~1", "--format", "concise", "a.yaml"],
    );
    assert_eq!(out(&o).lines().count(), 2, "{}", out(&o));
}

#[test]
fn check_reports_a_file_left_unformatted_only_when_the_revision_was_formatted() {
    let t = repo();
    write(t.path(), "d.yaml", "k:    1\n");
    git(t.path(), &["add", "-A"]);
    git(t.path(), &["commit", "-qm", "d"]);
    write(t.path(), "d.yaml", "k:    1\nj:    2\n");
    write(t.path(), "b.yaml", "- 1\n-    2\n");
    let o = sudsy(t.path(), &["check", "--new", "--format", "concise"]);
    assert_eq!(
        out(&o),
        "b.yaml:1:1: error[format]: the file is not formatted; run `sudsy format`\n"
    );
}

#[test]
fn new_needs_a_repository_and_a_revision() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.yaml", "- yes\n");
    let o = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .args(["lint", "--new"])
        .current_dir(t.path())
        .env("GIT_CEILING_DIRECTORIES", t.path().parent().unwrap())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(
        err(&o).contains("is not in a git repository"),
        "{}",
        err(&o)
    );
    let t = repo();
    let o = sudsy(t.path(), &["fix", "--new=nope"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(err(&o).contains("`nope` is not a revision"), "{}", err(&o));
}
