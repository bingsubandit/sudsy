//! `lint`, `fix`, `check` and `rules`: exit codes, the four output formats, and the fix loop on
//! files on disk. C# is covered by the language crate's tests; these use JSON and YAML only.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

pub(crate) fn sudsy(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

pub(crate) fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

pub(crate) fn read(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

pub(crate) fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n")
}

pub(crate) fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).replace("\r\n", "\n")
}

pub(crate) const CONFIG: &str =
    "lint:\n  rules:\n    sorted-keys: warn\n    ambiguous-booleans: warn\n";

/// A project with an error (duplicate key), warnings (unsorted keys, `yes`) and a fix for each.
pub(crate) fn project() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "sudsy.yaml", CONFIG);
    write(
        t.path(),
        "a.json",
        "{\n  \"b\": 1,\n  \"a\": 2,\n  \"a\": 3\n}\n",
    );
    write(t.path(), "b.yaml", "---\nz: yes\na: 1\n");
    t
}

#[test]
fn lint_human_output_and_exit_code() {
    let t = project();
    let o = sudsy(t.path(), &["lint", "a.json", "b.yaml"]);
    assert_eq!(o.status.code(), Some(1));
    insta::assert_snapshot!(out(&o));
    assert!(err(&o).contains("2 file(s) checked, 1 error(s), 3 warning(s)"));
}

#[test]
fn warnings_alone_do_not_fail() {
    let t = project();
    let o = sudsy(t.path(), &["lint", "b.yaml"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(out(&o).contains("warning[ambiguous-booleans]"));
}

#[test]
fn json_sarif_and_github_formats() {
    let t = project();
    for (name, flag) in [("json", "json"), ("sarif", "sarif"), ("github", "github")] {
        let o = sudsy(t.path(), &["lint", "--format", flag, "a.json", "b.yaml"]);
        assert_eq!(o.status.code(), Some(1), "{name}");
        let text = out(&o);
        match name {
            "github" => insta::assert_snapshot!("github", text),
            _ => {
                let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
                if name == "sarif" {
                    // The version moves with every release, and the rule table is checked below.
                    let driver = &mut value["runs"][0]["tool"]["driver"];
                    driver["version"] = "[version]".into();
                    driver["rules"] = "[rules]".into();
                }
                insta::assert_snapshot!(name, serde_json::to_string_pretty(&value).unwrap());
            }
        }
    }
}

#[test]
fn sarif_lists_the_rules_it_may_mention() {
    let t = project();
    let o = sudsy(t.path(), &["lint", "--format", "sarif", "a.json"]);
    let value: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    let ids: Vec<&str> = value["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    for id in [
        "no-duplicate-keys",
        "max-line-length",
        "syntax-error",
        "format",
    ] {
        assert!(ids.contains(&id), "{id} in {ids:?}");
    }
}

#[test]
fn syntax_errors_are_errors_and_stop_the_rules() {
    let t = project();
    write(t.path(), "bad.json", "{\"a\": }");
    let o = sudsy(t.path(), &["lint", "bad.json"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(out(&o).contains("error[syntax-error]"), "{}", out(&o));
}

#[test]
fn suppression_and_overrides_from_the_config() {
    let t = project();
    write(
        t.path(),
        "sudsy.yaml",
        &format!(
            "{CONFIG}overrides:\n  - files: [\"a.json\"]\n    lint:\n      rules:\n        no-duplicate-keys: \"off\"\n"
        ),
    );
    let o = sudsy(t.path(), &["lint", "a.json"]);
    assert_eq!(o.status.code(), Some(0), "{}", out(&o));
    assert!(!out(&o).contains("no-duplicate-keys"));
}

#[test]
fn an_unknown_rule_is_a_config_error_with_a_suggestion() {
    let t = project();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    sorted-key: warn\n",
    );
    let o = sudsy(t.path(), &["lint", "a.json"]);
    assert_eq!(o.status.code(), Some(2));
    let e = err(&o);
    assert!(
        e.contains("sudsy.yaml:3:")
            && e.contains("unknown rule `sorted-key`")
            && e.contains("did you mean `sorted-keys`"),
        "{e}"
    );
}

#[test]
fn an_invalid_rule_option_is_a_config_error() {
    let t = project();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    sorted-keys:\n      level: warn\n      options:\n        order: 3\n",
    );
    let o = sudsy(t.path(), &["lint", "a.json"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        err(&o).contains("option `order` must be a list of strings"),
        "{}",
        err(&o)
    );
}

#[test]
fn an_invalid_regex_option_is_a_config_error_in_every_command() {
    let t = project();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    sorted-keys:\n      level: warn\n      options:\n        ignored_keys: [\"(\"]\n",
    );
    for command in ["lint", "fix"] {
        let o = sudsy(t.path(), &[command, "a.json"]);
        assert_eq!(o.status.code(), Some(2), "{command}: {}", err(&o));
        assert!(
            err(&o).contains("invalid regular expression `(`") && !err(&o).contains("panicked"),
            "{command}: {}",
            err(&o)
        );
    }
}

#[test]
fn analysis_flag_overrides_the_config() {
    let t = project();
    write(t.path(), "sudsy.yaml", "analysis: deep\n");
    // Accepted in every depth; JSON and YAML have no layers or project rules to change the result.
    for depth in ["fast", "standard", "deep"] {
        let o = sudsy(t.path(), &["lint", "--analysis", depth, "b.yaml"]);
        assert_eq!(o.status.code(), Some(0), "{depth}: {}", err(&o));
    }
}

#[test]
fn fix_applies_safe_fixes_then_formats_and_reports_what_remains() {
    let t = project();
    write(t.path(), "c.yaml", "a:   1\n");
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    document-start: warn\n    sorted-keys: warn\n    ambiguous-booleans: warn\n",
    );
    let o = sudsy(t.path(), &["fix"]);
    // The duplicate key has no fix and stays an error.
    assert_eq!(o.status.code(), Some(1), "{}", err(&o));
    // Safe fix (`---` on c.yaml) and the formatter; unsafe ones (sorting, quoting) wait.
    assert_eq!(read(t.path(), "c.yaml"), "---\na: 1\n");
    assert!(read(t.path(), "a.json").contains("\"b\": 1,\n  \"a\": 2"));
    assert!(read(t.path(), "b.yaml").contains("z: yes"));
    assert!(out(&o).contains("error[no-duplicate-keys]"));
    assert!(err(&o).contains("file(s) changed"));
}

#[test]
fn fix_unsafe_applies_every_fix_and_a_second_run_changes_nothing() {
    let t = project();
    let o = sudsy(t.path(), &["fix", "--unsafe"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(read(t.path(), "b.yaml"), "---\na: 1\nz: \"yes\"\n");
    // Sorted, then formatted (the object was expanded, so it stays expanded).
    assert_eq!(
        read(t.path(), "a.json"),
        "{\n  \"a\": 2,\n  \"a\": 3,\n  \"b\": 1\n}\n"
    );
    let again = sudsy(t.path(), &["fix", "--unsafe"]);
    assert!(err(&again).contains("0 file(s) changed"), "{}", err(&again));
}

#[test]
fn check_reports_unformatted_files_and_rule_violations() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    sorted-keys: warn\n",
    );
    write(t.path(), "clean.json", "{\n  \"a\": 1\n}\n");
    let o = sudsy(t.path(), &["check", "clean.json"]);
    assert_eq!(o.status.code(), Some(0), "{}{}", out(&o), err(&o));

    write(t.path(), "messy.json", "{\"b\":1,\"a\":2}");
    let o = sudsy(t.path(), &["check", "clean.json", "messy.json"]);
    assert_eq!(o.status.code(), Some(1));
    insta::assert_snapshot!(out(&o));
    // Check never writes.
    assert_eq!(read(t.path(), "messy.json"), "{\"b\":1,\"a\":2}");
}

#[test]
fn check_in_json_format_carries_the_format_diagnostic() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "messy.json", "{\"a\":1}");
    let o = sudsy(t.path(), &["check", "--format", "json", "messy.json"]);
    assert_eq!(o.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(value["diagnostics"][0]["rule"], "format");
    assert_eq!(value["summary"]["errors"], 1);
}

#[test]
fn rules_lists_and_explains() {
    let t = tempfile::tempdir().unwrap();
    let list = out(&sudsy(t.path(), &["rules"]));
    for id in [
        "no-duplicate-keys",
        "sorted-keys",
        "ambiguous-booleans",
        "document-start",
        "max-line-length",
        "no-empty-block",
        "async-suffix",
        "using-order",
    ] {
        assert!(list.contains(id), "{id}\n{list}");
    }
    let one = sudsy(t.path(), &["rules", "sorted-keys"]);
    assert_eq!(one.status.code(), Some(0));
    insta::assert_snapshot!(out(&one));
    // A rule with a `why` shows both texts, paragraph by paragraph; the list keeps the first sentence.
    let why = out(&sudsy(t.path(), &["rules", "pipefail-shell"]));
    let (what, why) = why.split_once("\nWhy it matters\n").expect("both headings");
    assert!(what.contains("\nWhat it does\n  Reports a `RUN` with a pipe"));
    assert!(why.starts_with("  The exit status of a pipe"));
    assert!(why.contains("\n\n  To fix, set the option"), "{why}");
    // A rule whose languages share one reason (`crates/core-rules/texts/why.txt`) shows it with
    // the description of its language.
    let shared = out(&sudsy(t.path(), &["rules", "no-regex-spaces"]));
    assert!(
        shared.contains("\nWhy it matters\n  Several spaces in a row"),
        "{shared}"
    );
    // The text of a pattern rule file reaches the page the same way.
    let css = out(&sudsy(t.path(), &["rules", "color-hex-length"]));
    assert!(
        css.contains("\nWhy it matters\n  One length for hex colors"),
        "{css}"
    );
    let list_line = list
        .lines()
        .find(|l| l.starts_with("pipefail-shell"))
        .unwrap();
    assert!(!list_line.contains("does not count"), "{list_line}");
    let unknown = sudsy(t.path(), &["rules", "sorted-kyes"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(err(&unknown).contains("did you mean `sorted-keys`"));
    // An upstream id resolves to the rule; other text searches; --language narrows.
    let upstream = out(&sudsy(t.path(), &["rules", "truthy"]));
    assert!(
        upstream.starts_with(
            "ambiguous-booleans
"
        ),
        "{upstream}"
    );
    assert!(upstream.contains("presets:") && upstream.contains("yaml/yamllint"));
    assert!(out(&sudsy(t.path(), &["rules", "SA1005"])).starts_with(
        "comment-space
"
    ));
    let search = out(&sudsy(t.path(), &["rules", "ANCHOR", "--language", "yaml"]));
    assert!(search.contains("no-unused-anchors") && !search.contains("using-order"));
}

#[test]
fn yaml_anchor_rules_run_through_the_cli() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.yaml", "a: &x 1\nb: *y\n");
    let o = sudsy(t.path(), &["lint", "a.yaml"]);
    assert!(out(&o).contains("no-undefined-alias"), "{}", out(&o));
}

#[test]
fn a_module_is_judged_as_a_whole_and_only_the_named_files_are_reported() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    unused-declarations: warn\n",
    );
    write(t.path(), "m/a.tf", "variable \"x\" {}\nvariable \"y\" {}\n");
    write(t.path(), "m/b.tf", "output \"o\" { value = var.x }\n");
    // The use in `b.tf` counts when only `a.tf` is named; `b.tf` itself gets no report.
    let o = sudsy(t.path(), &["lint", "--format", "github", "m/a.tf"]);
    let text = out(&o);
    assert!(text.contains("`var.y` is declared but not used"), "{text}");
    assert!(!text.contains("var.x"), "{text}");
    // Both named: still only the unused variable, from `a.tf`.
    let o = sudsy(t.path(), &["lint", "--format", "github", "m"]);
    assert_eq!(out(&o).matches("unused-declarations").count(), 1);
    // Editing the sibling changes the result of the next run.
    write(t.path(), "m/b.tf", "output \"o\" { value = var.y }\n");
    let text = out(&sudsy(t.path(), &["lint", "--format", "github", "m/a.tf"]));
    assert!(text.contains("`var.x` is declared but not used"), "{text}");
    assert!(!text.contains("var.y"), "{text}");
}
