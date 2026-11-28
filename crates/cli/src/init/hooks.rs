//! The Claude Code hooks `sudsy init` adds with the agent block: `sudsy hook post-edit` after each
//! Edit, Write or MultiEdit and `sudsy hook stop` at a stop, in `.claude/settings.json`. The file is
//! merged as text (`json_edit.rs`): other settings and hooks stay, a Sudsy hook already there is left
//! as it is, a file that is not plain JSON is left alone with a note.

use super::editor;
use super::json_edit;
use super::plan::Plan;
use serde_json::{Map, Value, json};
use std::path::Path;

pub const SETTINGS: &str = ".claude/settings.json";

/// The event, its tool matcher and the argument of `sudsy hook`.
const HOOKS: [(&str, Option<&str>, &str); 2] = [
    ("PostToolUse", Some("Edit|Write|MultiEdit"), "post-edit"),
    ("Stop", None, "stop"),
];

/// How a hook calls Sudsy: `sudsy` when it is on the PATH, else the npm package pinned to this
/// version (never `@latest`, so a release does not change a project's checks unasked).
pub fn program() -> String {
    let names = ["sudsy", "sudsy.exe", "sudsy.cmd"];
    let on_path = std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| names.iter().any(|n| dir.join(n).is_file()))
    });
    if on_path {
        "sudsy".into()
    } else {
        format!("npx -y @sudsylint/sudsy@{}", env!("CARGO_PKG_VERSION"))
    }
}

/// Whether a matcher group of `groups` already runs `sudsy hook <arg>`.
fn has_sudsy(groups: &[Value], arg: &str) -> bool {
    let wanted = format!("hook {arg}");
    groups
        .iter()
        .filter_map(|g| g.get("hooks")?.as_array())
        .flatten()
        .filter_map(|h| h.get("command")?.as_str())
        .any(|c| c.contains("sudsy") && c.contains(&wanted))
}

fn group(program: &str, matcher: Option<&str>, arg: &str) -> Value {
    let hooks = json!([{"type": "command", "command": format!("{program} hook {arg}")}]);
    match matcher {
        Some(m) => json!({"matcher": m, "hooks": hooks}),
        None => json!({"hooks": hooks}),
    }
}

/// `text` (its root object `map`) with both hooks; `Ok(None)` when it has them, `Err` with a note
/// when its `hooks` are not the objects and lists Claude Code reads.
pub fn merged(
    text: &str,
    map: &Map<String, Value>,
    program: &str,
) -> Result<Option<String>, String> {
    let root = json_edit::root(text);
    let Some(hooks) = map.get("hooks") else {
        let all: Map<String, Value> = HOOKS
            .iter()
            .map(|(event, m, arg)| (event.to_string(), json!([group(program, *m, arg)])))
            .collect();
        return Ok(Some(json_edit::add_member(
            text,
            root,
            "hooks",
            &Value::Object(all),
        )));
    };
    let Value::Object(hooks) = hooks else {
        return Err("`hooks` is not an object".into());
    };
    let (mut text, mut changed) = (text.to_string(), false);
    for (event, matcher, arg) in HOOKS {
        let at = json_edit::member(&text, json_edit::root(&text), "hooks").expect("member");
        let entry = group(program, matcher, arg);
        text = match hooks.get(event) {
            None => json_edit::add_member(&text, at, event, &json!([entry])),
            Some(Value::Array(groups)) if has_sudsy(groups, arg) => continue,
            Some(Value::Array(_)) => {
                let list = json_edit::member(&text, at, event).expect("member");
                json_edit::add_item(&text, list, &entry)
            }
            Some(_) => return Err(format!("`hooks.{event}` is not a list")),
        };
        changed = true;
    }
    Ok(changed.then_some(text))
}

/// Adds the settings file with the hooks to `plan`, or a note on why not.
pub fn add(dir: &Path, plan: &mut Plan) {
    let manual = "add `sudsy hook post-edit` (PostToolUse, Edit|Write|MultiEdit) and `sudsy hook stop` (Stop) by hand";
    let Some((text, map)) = editor::read(dir, SETTINGS) else {
        plan.notes
            .push(format!("{SETTINGS} is not plain JSON; {manual}"));
        return;
    };
    match merged(&text, &map, &program()) {
        Ok(Some(text)) => editor::push(
            dir,
            plan,
            SETTINGS,
            text,
            "Claude Code hooks: lint after each edit, fix and format at a stop",
        ),
        Ok(None) => plan
            .notes
            .push(format!("{SETTINGS} already has the Sudsy hooks")),
        Err(why) => plan.notes.push(format!("{SETTINGS}: {why}; {manual}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merge(text: &str) -> Result<Option<String>, String> {
        let map = serde_json::from_str(text).unwrap();
        merged(text, &map, "sudsy")
    }

    #[test]
    fn a_new_file_gets_both_hooks() {
        let out = merge("{}\n").unwrap().unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v["hooks"]["PostToolUse"][0],
            json!({"matcher": "Edit|Write|MultiEdit", "hooks": [{"type": "command", "command": "sudsy hook post-edit"}]})
        );
        assert_eq!(
            v["hooks"]["Stop"][0]["hooks"][0]["command"],
            "sudsy hook stop"
        );
        assert_eq!(merge(&out).unwrap(), None, "a second run adds nothing");
    }

    #[test]
    fn other_settings_and_hooks_stay() {
        let text = "{\n    \"model\": \"x\",\n    \"hooks\": {\n        \"Stop\": [\n            {\"hooks\": [{\"type\": \"command\", \"command\": \"make test\"}]}\n        ]\n    }\n}\n";
        let out = merge(text).unwrap().unwrap();
        assert!(out.starts_with("{\n    \"model\": \"x\",\n    \"hooks\": {\n        \"Stop\": [\n            {\"hooks\": [{\"type\": \"command\", \"command\": \"make test\"}]},\n            {\n                \"hooks\": ["), "{out}");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(
            v["hooks"]["PostToolUse"][0]["matcher"],
            "Edit|Write|MultiEdit"
        );
        assert_eq!(merge(&out).unwrap(), None);
    }

    #[test]
    fn a_sudsy_hook_already_there_is_left_as_it_is() {
        let text = r#"{"hooks": {"PostToolUse": [{"matcher": "Edit", "hooks": [{"type": "command", "command": "npx sudsy hook post-edit"}]}]}}"#;
        let v: Value = serde_json::from_str(&merge(text).unwrap().unwrap()).unwrap();
        assert_eq!(v["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(
            v["hooks"]["Stop"][0]["hooks"][0]["command"],
            "sudsy hook stop"
        );
    }

    #[test]
    fn hooks_of_another_shape_are_not_touched() {
        assert!(merge(r#"{"hooks": []}"#).is_err());
        assert!(merge(r#"{"hooks": {"Stop": {}}}"#).is_err());
    }
}
