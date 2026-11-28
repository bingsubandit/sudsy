//! The human report (source lines, fix replacements, fix ids, collapse by rule), `--format
//! concise`, `--rule` and `fix --apply ID`. Uses the project of `lint.rs`.

use super::lint::{err, out, project, read, sudsy, write};
use std::path::Path;

/// `(rule, id)` of every fix with an id, from the JSON report of `lint`.
fn ids(dir: &Path, args: &[&str]) -> Vec<(String, String)> {
    let mut all = vec!["lint", "--format", "json"];
    all.extend(args);
    let value: serde_json::Value = serde_json::from_str(&out(&sudsy(dir, &all))).unwrap();
    let mut found = Vec::new();
    for d in value["diagnostics"].as_array().unwrap() {
        for f in d["fixes"].as_array().unwrap() {
            if let Some(id) = f["id"].as_str() {
                found.push((d["rule"].as_str().unwrap().to_string(), id.to_string()));
            }
        }
    }
    found
}

fn id_of(dir: &Path, file: &str, rule: &str) -> String {
    ids(dir, &[file])
        .into_iter()
        .find(|(r, _)| r == rule)
        .unwrap_or_else(|| panic!("no fix id of {rule} in {file}"))
        .1
}

#[test]
fn concise_is_one_line_per_finding() {
    let t = project();
    let o = sudsy(
        t.path(),
        &["lint", "--format", "concise", "a.json", "b.yaml"],
    );
    assert_eq!(o.status.code(), Some(1));
    insta::assert_snapshot!(out(&o));
}

#[test]
fn a_multi_line_fix_shows_a_few_lines_of_each_side() {
    let t = project();
    write(t.path(), "c.yaml", "e: 1\nd: 1\nc: 1\nb: 1\na: 1\n");
    let o = sudsy(t.path(), &["lint", "c.yaml"]);
    insta::assert_snapshot!(out(&o));
}

#[test]
fn a_long_report_is_collapsed_by_rule_and_rule_lists_one() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    ambiguous-booleans: warn\n",
    );
    let doc = |n: usize| -> String { (0..n).map(|i| format!("k{i:03}: yes\n")).collect() };
    write(t.path(), "a.yaml", &doc(99));
    write(t.path(), "b.json", "{\"a\": 1, \"a\": 2}\n");
    // 100 findings: in full.
    let full = out(&sudsy(t.path(), &["lint", "a.yaml", "b.json"]));
    assert!(full.contains("k098: \"yes\""), "{full}");
    // 101: one line per rule, errors first.
    write(t.path(), "a.yaml", &doc(100));
    let o = sudsy(t.path(), &["lint", "a.yaml", "b.json"]);
    assert_eq!(o.status.code(), Some(1));
    insta::assert_snapshot!(out(&o));
    // `--rule` lists that rule in full and keeps only its findings; JSON never collapses.
    let one = out(&sudsy(
        t.path(),
        &["lint", "--rule", "ambiguous-booleans", "a.yaml", "b.json"],
    ));
    assert_eq!(one.matches("warning[ambiguous-booleans]").count(), 100);
    assert!(!one.contains("no-duplicate-keys"), "{one}");
    assert_eq!(ids(t.path(), &["a.yaml"]).len(), 100);
}

#[test]
fn fix_apply_applies_the_named_fixes_only_then_formats() {
    let t = project();
    write(t.path(), "c.json", "{\"y\":1,\"x\":2}\n");
    let quote = id_of(t.path(), "b.yaml", "ambiguous-booleans");
    let sort = id_of(t.path(), "c.json", "sorted-keys");
    // One id: only that fix in that file, then the file is formatted; the others stay.
    let o = sudsy(t.path(), &["fix", "--apply", &quote]);
    assert_eq!(o.status.code(), Some(1), "{}", err(&o));
    assert_eq!(read(t.path(), "b.yaml"), "---\nz: \"yes\"\na: 1\n");
    assert_eq!(read(t.path(), "c.json"), "{\"y\":1,\"x\":2}\n");
    assert!(err(&o).contains("1 fix(es) applied"), "{}", err(&o));
    // Several ids at once, in one call; the id of the sort in b.yaml is the one printed now.
    let sort_b = id_of(t.path(), "b.yaml", "sorted-keys");
    let o = sudsy(t.path(), &["fix", "--apply", &format!("{sort},{sort_b}")]);
    assert!(err(&o).contains("2 fix(es) applied"), "{}", err(&o));
    assert_eq!(read(t.path(), "b.yaml"), "---\na: 1\nz: \"yes\"\n");
    assert_eq!(read(t.path(), "c.json"), "{ \"x\": 2, \"y\": 1 }\n");
}

#[test]
fn an_unknown_or_stale_fix_id_is_reported_with_the_current_ones() {
    let t = project();
    let quote = id_of(t.path(), "b.yaml", "ambiguous-booleans");
    // A line added above keeps the id.
    write(t.path(), "b.yaml", "---\nx: 1\nz: yes\na: 1\n");
    assert_eq!(id_of(t.path(), "b.yaml", "ambiguous-booleans"), quote);
    // An edit of the finding's line changes it: the old id is stale.
    write(t.path(), "b.yaml", "---\nx: 1\nz: yes # c\na: 1\n");
    let now = id_of(t.path(), "b.yaml", "ambiguous-booleans");
    assert_ne!(now, quote);
    let o = sudsy(
        t.path(),
        &["fix", "--apply", &format!("{quote},nope00"), "b.yaml"],
    );
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(read(t.path(), "b.yaml"), "---\nx: 1\nz: yes # c\na: 1\n");
    let e = err(&o);
    assert!(
        e.contains(&format!("no fix has the ids `{quote}`, `nope00`"))
            && e.contains(&format!("{now} (ambiguous-booleans, b.yaml:3)")),
        "{e}"
    );
}

#[test]
fn a_suggestion_is_applied_by_id_only() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "plugins:\n  patterns: [\"rules/*.yaml\"]\n",
    );
    write(
        t.path(),
        "rules/no-http.yaml",
        "id: no-http\nlanguage: yaml\nmessage: use https\nseverity: warn\nrule:\n  pattern: \"url: $U\"\n  regex: \"http:\"\nfix: \"url: https://example.org\"\nfix-kind: suggestion\n",
    );
    write(t.path(), "app.yaml", "---\nurl: http://example.org\n");
    let lint = out(&sudsy(t.path(), &["lint", "app.yaml"]));
    assert!(lint.contains("    2 | url: https://example.org"), "{lint}");
    sudsy(t.path(), &["fix", "--unsafe", "app.yaml"]);
    assert_eq!(read(t.path(), "app.yaml"), "---\nurl: http://example.org\n");
    let id = id_of(t.path(), "app.yaml", "no-http");
    let o = sudsy(t.path(), &["fix", "--apply", &id, "app.yaml"]);
    assert_eq!(o.status.code(), Some(0), "{}", err(&o));
    assert_eq!(
        read(t.path(), "app.yaml"),
        "---\nurl: https://example.org\n"
    );
}
