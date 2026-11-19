//! The option schema and presets of a language, from the `options` and `presets` keys of its
//! `lang.yaml`: `src/schema.generated.rs` holds one const per option name and `option_schema()`,
//! which the crate includes where its formatter lives (docs/ai/architecture/adding-a-language.md).

use crate::lang_crate::Lang;
use anyhow::{Result, bail};
use serde_json::Value;

/// `shared::` constructors with no argument.
const SHARED_NO_ARG: [&str; 2] = ["indent_style", "end_of_line"];
/// Names `shared::NAMES` already has a const for.
const SHARED_NAMES: [&str; 10] = [
    "max_blank_lines",
    "line_width",
    "indent_size",
    "indent_style",
    "final_newline",
    "end_of_line",
    "bracket_spacing",
    "array_bracket_spacing",
    "brace_newline",
    "brace_same_line",
];

fn lit(v: &Value) -> String {
    match v {
        Value::String(s) => format!("{s:?}"),
        Value::Array(items) => format!(
            "&[{}]",
            items.iter().map(lit).collect::<Vec<_>>().join(", ")
        ),
        other => other.to_string(),
    }
}

fn strs(v: &Value) -> String {
    let items = v.as_array().into_iter().flatten();
    format!("&[{}]", items.map(lit).collect::<Vec<_>>().join(", "))
}

/// One `OptionDef` expression and the option's name.
fn def(entry: &Value) -> Result<(String, String)> {
    if let Some(name) = entry.as_str() {
        if !SHARED_NO_ARG.contains(&name) {
            bail!("option `{name}` needs a value");
        }
        return Ok((name.into(), format!("shared::{name}()")));
    }
    let Some((name, body)) = entry.as_object().and_then(|m| m.iter().next()) else {
        bail!("an option is a name or a one-key map: {entry}");
    };
    let expr = match body {
        // A shared option with its default (and the description, for `max_blank_lines`).
        Value::Array(args) => format!(
            "shared::{name}({})",
            args.iter().map(lit).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(m) if !m.contains_key("doc") => bail!("option `{name}` has no `doc`"),
        Value::Object(m) => {
            let doc = lit(&m["doc"]);
            let default = m.get("default").map(lit).unwrap_or_default();
            if let Some(values) = m.get("values") {
                format!(
                    "OptionDef::choice({name:?}, {}, {default}, {doc})",
                    strs(values)
                )
            } else if let Some(r) = m.get("range").and_then(Value::as_array) {
                format!(
                    "OptionDef::int({name:?}, {default}, ({}, {}), {doc})",
                    lit(&r[0]),
                    lit(&r[1])
                )
            } else if m.get("kind").and_then(Value::as_str) == Some("words") {
                let words = m
                    .get("default")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(lit).collect::<Vec<_>>().join(", "))
                    .unwrap_or_default();
                format!("OptionDef::words({name:?}, &[{words}], {doc})")
            } else if m.get("kind").and_then(Value::as_str) == Some("word-sets") {
                format!("OptionDef::word_sets({name:?}, {doc})")
            } else {
                format!("OptionDef::boolean({name:?}, {default}, {doc})")
            }
        }
        scalar => format!("shared::{name}({})", lit(scalar)),
    };
    let expr = match expr.starts_with("shared::") {
        true => expr,
        false => format!("core_config::{expr}"),
    };
    Ok((
        name.clone(),
        expr.replace("shared::", "core_config::shared::"),
    ))
}

fn pascal(name: &str) -> String {
    let word = |w: &str| w[..1].to_uppercase() + &w[1..];
    name.split('_')
        .filter(|w| !w.is_empty())
        .map(word)
        .collect()
}

/// The typed getter of an option declared in full: an enum for a choice, `bool` or `usize`.
/// A getter never panics: an option the resolved set lacks (code embedded in another language
/// reads the host's options) has its default.
fn getter(name: &str, body: &Value) -> String {
    let Some(m) = body.as_object() else {
        return String::new();
    };
    let konst = name.to_uppercase();
    let o = "o: &core_config::ResolvedOptions";
    let allow = "#[allow(dead_code)]\n";
    if let Some(values) = m.get("values").and_then(Value::as_array) {
        let words: Vec<&str> = values.iter().filter_map(Value::as_str).collect();
        let plain = |w: &&str| {
            w.starts_with(|c: char| c.is_ascii_lowercase())
                && w.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        };
        if words.is_empty() || !words.iter().all(plain) {
            return String::new();
        }
        let ty = pascal(name);
        let default = m.get("default").and_then(Value::as_str).unwrap_or(words[0]);
        let variants = words
            .iter()
            .map(|w| pascal(w))
            .collect::<Vec<_>>()
            .join(", ");
        let arms: String = words
            .iter()
            .filter(|w| **w != default)
            .map(|w| format!("        {w:?} => {ty}::{},\n", pascal(w)))
            .collect();
        let derive = "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n";
        return format!(
            "{allow}{derive}pub enum {ty} {{ {variants} }}\n{allow}pub fn {name}({o}) -> {ty} {{\n    match o.text({konst}) {{\n{arms}        _ => {ty}::{},\n    }}\n}}\n",
            pascal(default)
        );
    }
    let default = m.get("default").map(lit).unwrap_or_default();
    if m.contains_key("range") {
        let default = if default.is_empty() {
            "0".into()
        } else {
            default
        };
        return format!(
            "{allow}pub fn {name}({o}) -> usize {{\n    o.uint({konst}, {default})\n}}\n"
        );
    }
    if m.contains_key("kind") {
        return String::new();
    }
    let default = if default.is_empty() {
        "false".into()
    } else {
        default
    };
    format!("{allow}pub fn {name}({o}) -> bool {{\n    o.flag({konst}, {default})\n}}\n")
}

/// `src/schema.generated.rs` of a language with an `options` key.
pub fn generate(name: &str, l: &Lang) -> Result<Option<String>> {
    let Some(options) = l.yaml.get("options").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut consts = String::new();
    let mut defs = String::new();
    let mut getters = String::new();
    for entry in options {
        let (opt, expr) = def(entry)?;
        if !SHARED_NAMES.contains(&opt.as_str()) {
            consts += &format!(
                "#[allow(dead_code)]\npub const {}: &str = {opt:?};\n",
                opt.to_uppercase()
            );
            if let Some(body) = entry.as_object().and_then(|m| m.values().next()) {
                getters += &getter(&opt, body);
            }
        }
        defs += &format!("        {expr},\n");
    }
    let presets = l.yaml.get("presets").and_then(Value::as_array);
    let presets = presets.into_iter().flatten().filter_map(Value::as_str);
    let presets: String = presets
        .map(|p| format!("        core_config::Preset::new(\"{name}/{p}\", include_str!(\"../presets/{p}.yaml\")),\n"))
        .collect();
    Ok(Some(format!(
        "// Generated from lang.yaml by `cargo xtask codegen`.\n{consts}{getters}\n#[rustfmt::skip]\npub fn option_schema() -> core_config::OptionSchema {{\n    core_config::OptionSchema::lazy(|| {{\n        core_config::OptionSchema::new(vec![\n{defs}        ])\n        .with_presets(vec![\n{presets}        ])\n    }})\n}}\n"
    )))
}
