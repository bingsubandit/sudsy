//! The editor and CI files `sudsy init` can write: the VS Code recommendation and format on save,
//! and a GitHub Actions job. An existing file is merged when it is plain JSON and left alone
//! otherwise; nothing is overwritten.

use super::json_edit;
use super::plan::{Action, File, Plan};
use serde_json::{Map, Value, json};
use std::path::Path;

const EXTENSION: &str = "sudsy.sudsy";

const WORKFLOW: &str = "name: sudsy
on:
  - push
  - pull_request
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - run: npx @sudsylint/sudsy check --format github
";

/// The text of `dir/rel` and its root object: `{}` when the file is new, `None` when it exists
/// and is not plain JSON (comments are allowed in these files, which this does not read).
pub(super) fn read(dir: &Path, rel: &str) -> Option<(String, Map<String, Value>)> {
    match std::fs::read_to_string(dir.join(rel)) {
        Err(_) => Some(("{}\n".to_string(), Map::new())),
        Ok(text) => match serde_json::from_str(&text) {
            Ok(Value::Object(map)) => Some((text, map)),
            _ => None,
        },
    }
}

pub(super) fn push(dir: &Path, plan: &mut Plan, rel: &str, text: String, note: &str) {
    let path = dir.join(rel);
    let action = if path.exists() {
        Action::Update
    } else {
        Action::Create
    };
    plan.files.push(File {
        path,
        text,
        action,
        note: note.to_string(),
    });
}

/// `.vscode/extensions.json` and, with `format_on_save`, `.vscode/settings.json` for the VS Code
/// language `ids`. What is there stays as it is; only the new entries are appended.
pub fn add(dir: &Path, ids: &[&str], format_on_save: bool, plan: &mut Plan) {
    let rel = ".vscode/extensions.json";
    match read(dir, rel) {
        None => plan.notes.push(format!(
            "{rel} has comments; add \"{EXTENSION}\" to its recommendations by hand"
        )),
        Some((text, map)) => {
            let root = json_edit::root(&text);
            let item = json!(EXTENSION);
            let edited = match map.get("recommendations") {
                None => Some(json_edit::add_member(
                    &text,
                    root,
                    "recommendations",
                    &json!([EXTENSION]),
                )),
                Some(Value::Array(list)) if list.contains(&item) => None,
                Some(Value::Array(_)) => {
                    let at = json_edit::member(&text, root, "recommendations").expect("member");
                    Some(json_edit::add_item(&text, at, &item))
                }
                Some(_) => {
                    plan.notes.push(format!(
                        "{rel}: `recommendations` is not a list; left alone"
                    ));
                    None
                }
            };
            if let Some(text) = edited {
                push(dir, plan, rel, text, "recommends the Sudsy extension");
            }
        }
    }
    if !format_on_save || ids.is_empty() {
        return;
    }
    let rel = ".vscode/settings.json";
    let Some((mut text, map)) = read(dir, rel) else {
        plan.notes.push(format!("{rel} has comments; set `editor.defaultFormatter` to \"{EXTENSION}\" and `editor.formatOnSave` per language by hand"));
        return;
    };
    let mut changed = false;
    for id in ids {
        let key = format!("[{id}]");
        let root = json_edit::root(&text);
        match map.get(&key) {
            None => {
                let entry =
                    json!({"editor.defaultFormatter": EXTENSION, "editor.formatOnSave": true});
                text = json_edit::add_member(&text, root, &key, &entry);
                changed = true;
            }
            Some(Value::Object(entry)) => match entry.get("editor.defaultFormatter") {
                None => {
                    let at = json_edit::member(&text, root, &key).expect("member");
                    text = json_edit::add_member(
                        &text,
                        at,
                        "editor.defaultFormatter",
                        &json!(EXTENSION),
                    );
                    if !entry.contains_key("editor.formatOnSave") {
                        text =
                            json_edit::add_member(&text, at, "editor.formatOnSave", &json!(true));
                    }
                    changed = true;
                }
                Some(v) if v == EXTENSION => {}
                Some(other) => plan.notes.push(format!(
                    "{rel}: [{id}] already formats with {other}; left alone"
                )),
            },
            Some(_) => {}
        }
    }
    if changed {
        push(dir, plan, rel, text, "formats on save with Sudsy");
    }
}

/// `.github/workflows/sudsy.yml`, unless it exists.
pub fn ci(dir: &Path, plan: &mut Plan) {
    let rel = ".github/workflows/sudsy.yml";
    if dir.join(rel).exists() {
        plan.notes.push(format!("{rel} exists; left alone"));
        return;
    }
    plan.files.push(File {
        path: dir.join(rel),
        text: WORKFLOW.into(),
        action: Action::Create,
        note: "runs `sudsy check` on push and pull request".into(),
    });
}
