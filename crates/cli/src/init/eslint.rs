//! ESLint, from a legacy JSON or YAML config (`.eslintrc.*`, `package.json` "eslintConfig"): the
//! core and typescript-eslint rules for JavaScript and TypeScript, and eslint-plugin-jsonc for
//! JSON. Flat configs (`eslint.config.*`) are JavaScript and are not read.

use super::out::Out;
use super::stylelint::strings;
use anyhow::{Result, bail};
use serde_json::Value;

pub const FILES: [&str; 4] = [
    ".eslintrc.json",
    ".eslintrc.yaml",
    ".eslintrc.yml",
    ".eslintrc",
];
pub const IGNORE_FILE: &str = ".eslintignore";
/// JavaScript configs; they cannot be read without running them.
pub const JS_CONFIGS: [&str; 10] = [
    "eslint.config.js",
    "eslint.config.mjs",
    "eslint.config.cjs",
    "eslint.config.ts",
    "eslint.config.mts",
    "eslint.config.cts",
    ".eslintrc.js",
    ".eslintrc.cjs",
    ".eslintrc.config.js",
    ".eslintrc.mjs",
];
const J: &[&str] = &["json"];
const SCRIPT_EXTENSIONS: [&str; 6] = [".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs"];

/// Which languages a block of `rules` applies to.
#[derive(Clone, Copy)]
struct Files {
    json: bool,
    js: bool,
}

pub fn migrate(config: &Value, out: &mut Out) -> Result<()> {
    let Some(root) = config.as_object() else {
        bail!("the config is not an object");
    };
    for name in strings(root.get("extends")) {
        extend(name, out);
    }
    for pattern in strings(root.get("ignorePatterns")) {
        out.exclude(pattern);
    }
    // `globals` are the names `undefined-variable` accepts (its `globals` option).
    if let Some(globals) = root.get("globals").and_then(Value::as_object) {
        let names: Vec<Value> = globals.keys().map(|k| Value::from(k.as_str())).collect();
        out.rule_opt_free("js", "undefined-variable", "globals", names.into());
    }
    if let Some(rules) = root.get("rules").and_then(Value::as_object) {
        let all = Files {
            json: true,
            js: true,
        };
        rules_of(rules, all, out);
    }
    // Overrides for JSON files carry the jsonc rules in most setups, those for scripts the rest.
    for entry in root
        .get("overrides")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let files = strings(entry.get("files"));
        let applies = Files {
            json: files.iter().any(|f| f.contains(".json")),
            js: files
                .iter()
                .any(|f| SCRIPT_EXTENSIONS.iter().any(|e| f.contains(e))),
        };
        if (applies.json || applies.js)
            && let Some(rules) = entry.get("rules").and_then(Value::as_object)
        {
            rules_of(rules, applies, out);
        }
    }
    Ok(())
}

/// The recommended configs map to presets; shared configs cannot be followed.
fn extend(name: &str, out: &mut Out) {
    match name {
        "eslint:recommended" => out.extend("js/eslint-recommended"),
        "plugin:@typescript-eslint/recommended" => out.extend("js/typescript-eslint-recommended"),
        "prettier" | "plugin:prettier/recommended" => {}
        _ => out.skip_why(
            name,
            "a shared config is not followed; its rules are not migrated",
        ),
    }
}

/// `(level, options)` of an ESLint rule setting: `"error"`, `2`, `["warn", {...}]`.
pub(super) fn setting(value: &Value) -> Option<(&'static str, &[Value])> {
    let (sev, opts) = match value {
        Value::Array(a) => (a.first()?, a.get(1..).unwrap_or_default()),
        v => (v, [].as_slice()),
    };
    let level = match sev {
        Value::String(s) => match s.as_str() {
            "off" => "off",
            "warn" => "warn",
            "error" => "error",
            _ => return None,
        },
        Value::Number(n) => match n.as_u64()? {
            0 => "off",
            1 => "warn",
            2 => "error",
            _ => return None,
        },
        _ => return None,
    };
    Some((level, opts))
}

fn rules_of(rules: &serde_json::Map<String, Value>, files: Files, out: &mut Out) {
    for (name, value) in rules {
        let Some((level, opts)) = setting(value) else {
            continue;
        };
        let Some(rule) = name.strip_prefix("jsonc/") else {
            if files.js {
                script_rule(name, level, out);
            }
            continue;
        };
        if !files.json {
            continue;
        }
        out.catalog("eslint-plugin-jsonc");
        let Some(entry) = out.entry(name, "not an eslint-plugin-jsonc rule Sudsy knows") else {
            continue;
        };
        let status = entry.sudsy.as_str();
        if let Some(sudsy) = status.strip_prefix("rule: ") {
            if sudsy == "syntax-error" {
                // Always reported by the parser; only turning it off cannot be followed.
                if level == "off" {
                    out.skip_why(name, "syntax errors are always reported");
                }
                continue;
            }
            out.rule(name, "json", sudsy, level);
            if level != "off" {
                rule_options(name, sudsy, opts, out);
            }
        } else if level != "off" {
            if status.starts_with("option: ") {
                format_rule(rule, name, opts, out);
            } else if status != "formatter" {
                out.skip(name);
            }
        }
    }
}

/// A core rule, or a typescript-eslint rule (`@typescript-eslint/<id>`).
fn script_rule(name: &str, level: &str, out: &mut Out) {
    let (tool, id) = match name.strip_prefix("@typescript-eslint/") {
        Some(id) => ("typescript-eslint", id),
        None if name.contains('/') => {
            out.skip_why(name, "a plugin Sudsy has no equivalent for");
            return;
        }
        None => ("eslint", name),
    };
    out.catalog(tool);
    let Some(entry) = out.entry(id, "not a rule of that tool Sudsy knows") else {
        return;
    };
    if entry.rules().next().is_some() {
        // One upstream rule split over several ids: the level applies to each.
        for sudsy in entry.rules() {
            out.rule(id, "js", sudsy, level);
        }
    } else if level != "off" && entry.sudsy != "formatter" {
        out.skip(id);
    }
}

fn rule_options(name: &str, sudsy: &str, opts: &[Value], out: &mut Out) {
    let Some(first) = opts.first() else { return };
    // `sorted-keys` and `sorted-array-values` take an array of option objects in ESLint.
    let object = first.as_object();
    match (sudsy, object) {
        ("key-name-casing", Some(o)) => {
            let styles = [
                "camelCase",
                "PascalCase",
                "snake_case",
                "kebab-case",
                "SCREAMING_SNAKE_CASE",
            ];
            let allowed: Vec<Value> = styles
                .iter()
                .filter(|s| {
                    o.get(**s)
                        .and_then(Value::as_bool)
                        .unwrap_or(**s == "camelCase")
                })
                .map(|s| Value::from(*s))
                .collect();
            out.rule_opt(name, "json", sudsy, "allowed", allowed.into());
            if let Some(ignores) = o.get("ignores") {
                out.rule_opt(name, "json", sudsy, "ignored_keys", ignores.clone());
            }
        }
        ("sorted-keys" | "sorted-array-values", _) => {
            // Either `"asc", { minKeys }` or `{ pathPattern, order, minKeys }, ...`.
            let (order, object) = match first {
                Value::String(_) => (Some(first.clone()), opts.get(1).and_then(Value::as_object)),
                _ => (object.and_then(|o| o.get("order").cloned()), object),
            };
            let scoped = first.is_object() && opts.len() > 1
                || object.is_some_and(|o| {
                    o.contains_key("pathPattern") || o.contains_key("hasProperties")
                });
            if scoped {
                out.skip_why(
                    &format!("{name} options"),
                    "per-path option sets are not supported; set the options by hand",
                );
                return;
            }
            sort_options(name, sudsy, object, order.as_ref(), out);
        }
        _ => {}
    }
}

fn sort_options(
    name: &str,
    sudsy: &str,
    object: Option<&serde_json::Map<String, Value>>,
    order: Option<&Value>,
    out: &mut Out,
) {
    let ty = match order {
        Some(Value::String(s)) => Some(s.as_str()),
        Some(Value::Object(o)) => o.get("type").and_then(Value::as_str),
        _ => None,
    };
    if let Some(dir @ ("asc" | "desc")) = ty {
        out.rule_opt(name, "json", sudsy, "direction", dir.into());
    }
    if let Some(Value::Object(o)) = order {
        let natural = o.get("natural").and_then(Value::as_bool) == Some(true);
        let insensitive = o.get("caseSensitive").and_then(Value::as_bool) == Some(false);
        let compare = if natural {
            "natural"
        } else if insensitive {
            "case_insensitive"
        } else {
            ""
        };
        if !compare.is_empty() {
            out.rule_opt(name, "json", sudsy, "compare", compare.into());
        }
    }
    if let Some(o) = object {
        let min = if sudsy == "sorted-keys" {
            "minKeys"
        } else {
            "minValues"
        };
        if let Some(n) = o.get(min) {
            let opt = if sudsy == "sorted-keys" {
                "min_keys"
            } else {
                "min_values"
            };
            out.rule_opt(name, "json", sudsy, opt, n.clone());
        }
        if let Some(b) = o.get("allowLineSeparatedGroups") {
            out.rule_opt(name, "json", sudsy, "blank_line_groups", b.clone());
        }
    }
}

fn format_rule(rule: &str, id: &str, opts: &[Value], out: &mut Out) {
    let first = opts.first().and_then(Value::as_str);
    match rule {
        "array-bracket-spacing" => out.opt(id, J, "array_bracket_spacing", first == Some("always")),
        "object-curly-spacing" => out.opt(id, J, "bracket_spacing", first == Some("always")),
        "comma-dangle" => {
            let never = first == Some("never")
                || opts
                    .first()
                    .and_then(Value::as_object)
                    .is_some_and(|o| o.values().all(|v| v == "never"));
            out.opt(
                id,
                J,
                "trailing_commas",
                if never { "never" } else { "multiline" },
            );
        }
        "indent" => match opts.first() {
            Some(Value::String(s)) if s == "tab" => out.opt_free(J, "indent_style", "tab"),
            Some(Value::Number(n)) if n.is_u64() => out.opt(id, J, "indent_size", n.clone()),
            _ => {}
        },
        _ => out.skip(id),
    }
}
