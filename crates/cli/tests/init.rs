//! `sudsy init --from <tool>`: each directory under `tests/fixtures/init/` holds one tool's config
//! files (the directory name is the `--from` value). The generated `sudsy.yaml` and the report
//! are snapshots, and the config must load. Debug builds also check every mapping against the
//! parity catalog while migrating.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/init")
}

fn sudsy(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn copy_fixture(tool: &str) -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    for file in fs::read_dir(root().join(tool)).unwrap() {
        let path = file.unwrap().path();
        fs::copy(&path, t.path().join(path.file_name().unwrap())).unwrap();
    }
    t
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

#[test]
fn every_tool_fixture_migrates_to_a_loadable_config() {
    let mut tools = 0;
    for dir in fs::read_dir(root()).unwrap() {
        let tool = dir.unwrap().file_name().to_string_lossy().into_owned();
        let t = copy_fixture(&tool);
        let out = sudsy(t.path(), &["init", "--from", &tool]);
        assert_eq!(out.status.code(), Some(0), "{tool}: {}", text(&out.stderr));
        let yaml = fs::read_to_string(t.path().join("sudsy.yaml")).unwrap();
        insta::assert_snapshot!(tool.clone(), format!("{yaml}---\n{}", text(&out.stdout)));
        let printed = sudsy(t.path(), &["config", "print"]);
        assert_eq!(
            printed.status.code(),
            Some(0),
            "{tool}: {}",
            text(&printed.stderr)
        );
        tools += 1;
    }
    assert_eq!(tools, 10);
}

#[test]
fn an_existing_config_is_kept_without_force() {
    let t = copy_fixture("csharpier");
    fs::write(t.path().join("sudsy.yaml"), "analysis: fast\n").unwrap();
    let out = sudsy(t.path(), &["init", "--from", "csharpier"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        fs::read_to_string(t.path().join("sudsy.yaml")).unwrap(),
        "analysis: fast\n"
    );
    let out = sudsy(t.path(), &["init", "--from", "csharpier", "--force"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(
        fs::read_to_string(t.path().join("sudsy.yaml"))
            .unwrap()
            .contains("csharpier-compat")
    );
}

#[test]
fn a_javascript_eslint_config_is_refused_with_an_explanation() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("eslint.config.js"), "export default [];\n").unwrap();
    let out = sudsy(t.path(), &["init", "--from", "eslint-jsonc"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("eslint.config.js is JavaScript"),
        "{}",
        text(&out.stderr)
    );
    assert!(!t.path().join("sudsy.yaml").exists());
}

#[test]
fn a_javascript_stylelint_config_is_refused_with_an_explanation() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("stylelint.config.js"), "export default {};\n").unwrap();
    let out = sudsy(t.path(), &["init", "--from", "stylelint"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("stylelint.config.js is JavaScript"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn a_missing_oversized_or_broken_config_is_an_error() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(
        sudsy(t.path(), &["init", "--from", "yamllint"])
            .status
            .code(),
        Some(2)
    );
    fs::write(t.path().join(".yamllint"), "rules: [unterminated\n").unwrap();
    let out = sudsy(t.path(), &["init", "--from", "yamllint"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains(".yamllint"),
        "{}",
        text(&out.stderr)
    );
    fs::write(t.path().join(".yamllint"), "#".repeat(2 << 20)).unwrap();
    let out = sudsy(t.path(), &["init", "--from", "yamllint"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("limit"), "{}", text(&out.stderr));
}

#[test]
fn write_editorconfig_appends_once() {
    let t = copy_fixture("csharpier");
    let out = sudsy(
        t.path(),
        &["init", "--from", "csharpier", "--write-editorconfig"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let first = fs::read_to_string(t.path().join(".editorconfig")).unwrap();
    assert!(first.contains("dotnet_diagnostic.IDE0055.severity = none"));
    let out = sudsy(
        t.path(),
        &[
            "init",
            "--from",
            "csharpier",
            "--force",
            "--write-editorconfig",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(t.path().join(".editorconfig")).unwrap(),
        first
    );
    let out = sudsy(
        t.path(),
        &["init", "--from", "yamllint", "--write-editorconfig"],
    );
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn agents_adds_one_block_to_the_instruction_file() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path();
    // Alone: no sudsy.yaml, a new AGENTS.md.
    assert!(sudsy(dir, &["init", "--agents"]).status.success());
    assert!(!dir.join("sudsy.yaml").exists());
    let once = fs::read_to_string(dir.join("AGENTS.md")).unwrap();
    assert!(once.contains("sudsy check") && once.contains("sudsy fix"));
    // And the Claude Code hooks.
    let settings = fs::read_to_string(dir.join(".claude/settings.json")).unwrap();
    assert!(
        settings.contains("\"Edit|Write|MultiEdit\"")
            && settings.contains("hook post-edit\"")
            && settings.contains("hook stop\""),
        "{settings}"
    );
    // Again: unchanged.
    assert!(sudsy(dir, &["init", "--agents"]).status.success());
    assert_eq!(fs::read_to_string(dir.join("AGENTS.md")).unwrap(), once);
    assert_eq!(
        fs::read_to_string(dir.join(".claude/settings.json")).unwrap(),
        settings
    );
    // Only CLAUDE.md: that file, its text kept; the settings there are merged.
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("CLAUDE.md"), "# Mine\n").unwrap();
    fs::create_dir(t.path().join(".claude")).unwrap();
    let mine = "{\n  \"model\": \"x\"\n}\n";
    fs::write(t.path().join(".claude/settings.json"), mine).unwrap();
    assert!(sudsy(t.path(), &["init", "--agents"]).status.success());
    let merged = fs::read_to_string(t.path().join(".claude/settings.json")).unwrap();
    assert!(
        merged.starts_with("{\n  \"model\": \"x\",\n  \"hooks\": {")
            && merged.contains("hook stop"),
        "{merged}"
    );
    assert!(!t.path().join("AGENTS.md").exists());
    let text = fs::read_to_string(t.path().join("CLAUDE.md")).unwrap();
    assert!(
        text.starts_with("# Mine\n\n<!-- sudsy:start -->")
            && text.matches("sudsy:start").count() == 1
    );
}

#[test]
fn init_without_agents_flag_writes_no_instruction_file() {
    let t = tempfile::tempdir().unwrap();
    assert!(sudsy(t.path(), &["init"]).status.success());
    assert!(t.path().join("sudsy.yaml").exists());
    assert!(!t.path().join("AGENTS.md").exists() && !t.path().join("CLAUDE.md").exists());
}

/// A small repository with several languages, tool configs and a `.vscode` folder.
fn mixed() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    for (name, text) in [
        ("src/a.py", "print('x')\n"),
        ("src/b.py", "print('y')\n"),
        ("src/c.js", "let a = 1;\n"),
        ("data.json", "{\"a\": 1}\n"),
        ("cfg.yaml", "a: 1\n"),
        (".github/workflows/ci.yml", "name: ci\non: push\njobs: {}\n"),
        ("README.md", "# hi\n"),
        (".prettierrc", "{ \"printWidth\": 100, \"tabWidth\": 4 }\n"),
        (".yamllint", "extends: default\n"),
        (".vscode/settings.json", "{}\n"),
    ] {
        let path = t.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    t
}

/// Every answer combination the tests cover leaves a config that loads and a `check` that runs
/// (exit 0 or 1: findings are fine, a config error or a failed file is exit 2).
#[test]
fn every_tested_combination_leaves_a_config_that_checks() {
    let cases: [&[&str]; 8] = [
        &["--yes"],
        &["--yes", "--languages", "python,json"],
        &["--yes", "--languages", "yaml"],
        &["--yes", "--preset", "json/biome", "--preset", "python/ruff"],
        &[
            "--yes",
            "--from",
            "prettier",
            "--preset",
            "yaml/prettier-compat",
        ],
        &["--yes", "--from", "yamllint", "--from", "prettier"],
        &["--yes", "--vscode", "--ci", "--agents"],
        &["--yes", "--languages", "markdown", "--vscode", "--ci"],
    ];
    for case in cases {
        let t = mixed();
        let mut args = vec!["init"];
        args.extend(case);
        let out = sudsy(t.path(), &args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{case:?}: {}",
            text(&out.stderr)
        );
        let printed = sudsy(t.path(), &["config", "print"]);
        assert_eq!(
            printed.status.code(),
            Some(0),
            "{case:?}: {}",
            text(&printed.stderr)
        );
        let checked = sudsy(t.path(), &["check"]);
        let code = checked.status.code();
        assert!(
            code == Some(0) || code == Some(1),
            "{case:?}: {}",
            text(&checked.stderr)
        );
        let report = text(&checked.stdout);
        assert!(
            !report
                .lines()
                .any(|l| l.contains("workflows/sudsy.yml") && l.contains(": error[")),
            "{case:?}: {report}"
        );
    }
}

#[test]
fn deselected_languages_are_excluded_and_a_workflow_is_not_caught_with_yaml() {
    let t = mixed();
    assert!(
        sudsy(t.path(), &["init", "--yes", "--languages", "yaml"])
            .status
            .success()
    );
    let yaml = fs::read_to_string(t.path().join("sudsy.yaml")).unwrap();
    assert!(
        yaml.contains("**/*.py") && yaml.contains("**/*.json"),
        "{yaml}"
    );
    assert!(!yaml.contains("**/*.yml"), "{yaml}");
    let out = text(&sudsy(t.path(), &["check"]).stdout);
    assert!(!out.contains("a.py") && !out.contains("data.json"), "{out}");
}

#[test]
fn init_never_overwrites_a_file_and_a_refused_run_writes_nothing() {
    let t = mixed();
    let dir = t.path();
    fs::create_dir_all(dir.join(".github/workflows")).unwrap();
    fs::write(dir.join(".github/workflows/sudsy.yml"), "mine: 1\n").unwrap();
    fs::write(
        dir.join(".vscode/extensions.json"),
        "// mine\n{ \"recommendations\": [] }\n",
    )
    .unwrap();
    fs::write(dir.join("AGENTS.md"), "# Mine\n").unwrap();
    let out = sudsy(dir, &["init", "--yes", "--vscode", "--ci", "--agents"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let read = |p: &str| fs::read_to_string(dir.join(p)).unwrap();
    assert_eq!(read(".github/workflows/sudsy.yml"), "mine: 1\n");
    assert_eq!(
        read(".vscode/extensions.json"),
        "// mine\n{ \"recommendations\": [] }\n"
    );
    assert!(read("AGENTS.md").starts_with("# Mine\n\n<!-- sudsy:start -->"));
    // The settings file was plain JSON: merged, with the extension as formatter per language.
    assert!(read(".vscode/settings.json").contains("sudsy.sudsy"));
    // An existing config stops the run before anything else is written.
    let t = mixed();
    fs::write(t.path().join("sudsy.yaml"), "analysis: fast\n").unwrap();
    let out = sudsy(t.path(), &["init", "--yes", "--vscode", "--ci", "--agents"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        fs::read_to_string(t.path().join("sudsy.yaml")).unwrap(),
        "analysis: fast\n"
    );
    assert!(
        !t.path().join(".github/workflows/sudsy.yml").exists()
            && !t.path().join("AGENTS.md").exists()
    );
}

#[test]
fn without_a_terminal_init_writes_the_starter_only() {
    let t = mixed();
    assert!(sudsy(t.path(), &["init"]).status.success());
    assert_eq!(
        fs::read_to_string(t.path().join("sudsy.yaml")).unwrap(),
        "# https://github.com/bingsubandit/sudsy\nextends: []   # e.g. preset:python/ruff\n"
    );
    assert!(
        !t.path().join("AGENTS.md").exists()
            && !t.path().join(".github/workflows/sudsy.yml").exists()
    );
}

/// The files `init` writes pass Sudsy's formatter with the config it just wrote, and an edited
/// JSON file keeps the user's key order and indentation.
#[test]
fn written_files_are_formatted_and_user_json_keeps_its_order() {
    let t = mixed();
    let user = "{\n\t\"z.last\": 1,\n\t\"a.first\": [\n\t\t\"x\"\n\t]\n}\n";
    fs::write(t.path().join(".vscode/settings.json"), user).unwrap();
    fs::write(
        t.path().join(".vscode/extensions.json"),
        "{\n\t\"recommendations\": [\n\t\t\"b.b\"\n\t]\n}\n",
    )
    .unwrap();
    let out = sudsy(
        t.path(),
        &[
            "init", "--yes", "--from", "yamllint", "--from", "prettier", "--vscode", "--ci",
            "--agents",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let settings = fs::read_to_string(t.path().join(".vscode/settings.json")).unwrap();
    assert!(
        settings.starts_with("{\n\t\"z.last\": 1,\n\t\"a.first\": [\n\t\t\"x\"\n\t],\n\t\"["),
        "{settings}"
    );
    let ext = fs::read_to_string(t.path().join(".vscode/extensions.json")).unwrap();
    assert_eq!(
        ext,
        "{\n\t\"recommendations\": [\n\t\t\"b.b\",\n\t\t\"sudsy.sudsy\"\n\t]\n}\n"
    );
    for file in [
        "sudsy.yaml",
        "CLAUDE.md",
        "AGENTS.md",
        ".github/workflows/sudsy.yml",
    ] {
        if t.path().join(file).exists() {
            let checked = sudsy(t.path(), &["format", "--check", file]);
            assert_eq!(
                checked.status.code(),
                Some(0),
                "{file}: {}",
                text(&checked.stdout)
            );
        }
    }
}
