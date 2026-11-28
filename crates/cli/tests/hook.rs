//! `sudsy hook post-edit` and `sudsy hook stop`: the Claude Code hook input on stdin, the exit codes
//! the agent reads (0 silent, 2 with stderr), the loop guard of the stop hook.

use super::lint::{err, out, read, write};
use super::new::git;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn hook(dir: &Path, event: &str, input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .args(["hook", event])
        .current_dir(dir)
        .env("GIT_CEILING_DIRECTORIES", dir.parent().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// The PostToolUse input of an Edit of `file` in `dir`.
fn edited(dir: &Path, file: &str) -> String {
    let path = dir.join(file).display().to_string().replace('\\', "/");
    let cwd = dir.display().to_string().replace('\\', "/");
    format!(
        r#"{{"session_id": "s", "cwd": "{cwd}", "hook_event_name": "PostToolUse", "tool_name": "Edit",
            "tool_input": {{"file_path": "{path}", "old_string": "a", "new_string": "b"}},
            "tool_response": {{}}, "tool_use_id": "t"}}"#
    )
}

fn stopped(dir: &Path, active: bool) -> String {
    let cwd = dir.display().to_string().replace('\\', "/");
    format!(
        r#"{{"session_id": "s", "cwd": "{cwd}", "hook_event_name": "Stop", "stop_hook_active": {active}}}"#
    )
}

/// (exit code, stdout, stderr)
fn result(o: &Output) -> (Option<i32>, String, String) {
    (o.status.code(), out(o), err(o))
}

fn silent() -> (Option<i32>, String, String) {
    (Some(0), String::new(), String::new())
}

/// A repository whose `HEAD` has `a.yaml` with one `yes` (an error).
fn repo() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "exclude: [\"vendor/**\"]\nlint:\n  rules:\n    ambiguous-booleans: error\n",
    );
    write(t.path(), "a.yaml", "- yes\n- 1\n");
    git(t.path(), &["init", "-q"]);
    git(t.path(), &["add", "-A"]);
    git(t.path(), &["commit", "-qm", "base"]);
    t
}

#[test]
fn post_edit_shows_the_agent_only_the_new_findings() {
    let t = repo();
    // The old `yes` is not this edit's: nothing to say.
    write(t.path(), "a.yaml", "- yes\n- 2\n");
    assert_eq!(
        result(&hook(t.path(), "post-edit", &edited(t.path(), "a.yaml"))),
        silent()
    );
    // A new one: the human report on stderr, exit 2, never a fix or a format.
    write(t.path(), "a.yaml", "- yes\n- 2\n-    no\n");
    let o = hook(t.path(), "post-edit", &edited(t.path(), "a.yaml"));
    let (code, stdout, stderr) = result(&o);
    assert_eq!((code, stdout.as_str()), (Some(2), ""), "{stderr}");
    assert!(
        stderr.starts_with("a.yaml:3:6: error[ambiguous-booleans]")
            && stderr.contains("    3 | -    no")
            && stderr.ends_with("1 file(s) checked, 1 new error(s), 0 new warning(s)\n"),
        "{stderr}"
    );
    assert_eq!(read(t.path(), "a.yaml"), "- yes\n- 2\n-    no\n");
}

#[test]
fn post_edit_is_silent_on_files_sudsy_does_not_check() {
    let t = repo();
    write(t.path(), "notes.txt", "yes\n");
    write(t.path(), "vendor/v.yaml", "- yes\n");
    for file in ["notes.txt", "vendor/v.yaml", "gone.yaml"] {
        let o = hook(t.path(), "post-edit", &edited(t.path(), file));
        assert_eq!(result(&o), silent(), "{file}");
    }
    let o = hook(
        t.path(),
        "post-edit",
        r#"{"tool_name": "Bash", "tool_input": {"command": "ls"}}"#,
    );
    assert_eq!(result(&o), silent());
    // Input that is not a hook's: a hook error for the user (exit 1), not for the agent.
    let o = hook(t.path(), "post-edit", "not json");
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).starts_with("sudsy hook: "), "{}", err(&o));
}

#[test]
fn stop_fixes_and_formats_the_changed_files_and_turns_the_agent_back_once() {
    let t = repo();
    // Changed and untracked files are formatted; a committed unchanged one is not touched.
    write(t.path(), "a.yaml", "- yes\n- 1\n- no\n");
    write(t.path(), "new/c.yaml", "k:    1\n");
    write(t.path(), "ignored.yaml", "k:    1\n");
    write(t.path(), ".gitignore", "ignored.yaml\n");
    let o = hook(t.path(), "stop", &stopped(t.path(), false));
    let (code, stdout, stderr) = result(&o);
    assert_eq!((code, stdout.as_str()), (Some(2), ""), "{stderr}");
    assert!(
        stderr.starts_with("Sudsy fixed and formatted 1 file(s); read them again before editing.\na.yaml:3:3: error[ambiguous-booleans]")
            && !stderr.contains("a.yaml:1:"),
        "{stderr}"
    );
    assert_eq!(read(t.path(), "new/c.yaml"), "k: 1\n");
    assert_eq!(read(t.path(), "ignored.yaml"), "k:    1\n");
    // The agent stops again without fixing it: it may, and the user gets the findings as a note.
    let o = hook(t.path(), "stop", &stopped(t.path(), true));
    let (code, stdout, stderr) = result(&o);
    assert_eq!((code, stderr.as_str()), (Some(0), ""), "{stdout}");
    assert!(
        stdout.starts_with(r#"{"systemMessage":"Sudsy findings left:\na.yaml:3:3"#),
        "{stdout}"
    );
    // Clean: nothing at all.
    write(t.path(), "a.yaml", "- yes\n- 1\n");
    assert_eq!(
        result(&hook(t.path(), "stop", &stopped(t.path(), false))),
        silent()
    );
}

#[test]
fn stop_outside_git_changes_nothing() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.yaml", "k:    yes\n");
    let o = hook(t.path(), "stop", &stopped(t.path(), false));
    let (code, stdout, stderr) = result(&o);
    assert_eq!((code, stdout.as_str()), (Some(0), ""));
    assert!(
        stderr.contains("is not in a git repository; nothing fixed, formatted or checked")
            && stderr.lines().count() == 1,
        "{stderr}"
    );
    assert_eq!(read(t.path(), "a.yaml"), "k:    yes\n");
}
