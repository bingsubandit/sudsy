//! Option schemas: every provider declares its format options once (name, type, default,
//! allowed values, description). Validation, resolution, docs and tests derive from it.
//!
//! concept: none (infrastructure: option schemas)

use crate::error::unknown_name;
use crate::model::OptionValue;
use crate::presets::Preset;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptionKind {
    Bool,
    Int {
        min: i64,
        max: i64,
    },
    Enum(&'static [&'static str]),
    /// A list of C# identifiers-like words (preprocessor symbols).
    Words,
    /// A list of lists of words (sets of preprocessor symbols).
    WordSets,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OptionDef {
    pub name: &'static str,
    pub kind: OptionKind,
    pub default: OptionValue,
    pub description: &'static str,
}

impl OptionDef {
    pub fn boolean(name: &'static str, default: bool, description: &'static str) -> Self {
        OptionDef {
            name,
            kind: OptionKind::Bool,
            default: OptionValue::Bool(default),
            description,
        }
    }

    pub fn int(
        name: &'static str,
        default: i64,
        range: (i64, i64),
        description: &'static str,
    ) -> Self {
        OptionDef {
            name,
            kind: OptionKind::Int {
                min: range.0,
                max: range.1,
            },
            default: OptionValue::Int(default),
            description,
        }
    }

    /// A list of words (letters, digits, `_`).
    pub fn words(name: &'static str, default: &[&str], description: &'static str) -> Self {
        OptionDef {
            name,
            kind: OptionKind::Words,
            default: OptionValue::List(
                default
                    .iter()
                    .map(|w| OptionValue::String((*w).into()))
                    .collect(),
            ),
            description,
        }
    }

    /// A list of lists of words, empty by default.
    pub fn word_sets(name: &'static str, description: &'static str) -> Self {
        OptionDef {
            name,
            kind: OptionKind::WordSets,
            default: OptionValue::List(Vec::new()),
            description,
        }
    }

    pub fn choice(
        name: &'static str,
        values: &'static [&'static str],
        default: &'static str,
        description: &'static str,
    ) -> Self {
        debug_assert!(values.contains(&default));
        OptionDef {
            name,
            kind: OptionKind::Enum(values),
            default: OptionValue::String(default.into()),
            description,
        }
    }

    /// Every value worth testing: both booleans, all enum values, and the bounds and default of
    /// an integer range.
    pub fn sample_values(&self) -> Vec<OptionValue> {
        match &self.kind {
            OptionKind::Bool => vec![OptionValue::Bool(false), OptionValue::Bool(true)],
            OptionKind::Enum(values) => values
                .iter()
                .map(|v| OptionValue::String((*v).into()))
                .collect(),
            OptionKind::WordSets => vec![
                OptionValue::List(Vec::new()),
                OptionValue::List(vec![OptionValue::List(vec![OptionValue::String(
                    "DEBUG".into(),
                )])]),
            ],
            OptionKind::Words => vec![
                OptionValue::List(Vec::new()),
                OptionValue::List(vec![OptionValue::String("DEBUG".into())]),
            ],
            OptionKind::Int { min, max } => {
                let default = match self.default {
                    OptionValue::Int(d) => d,
                    _ => *min,
                };
                let mut v = vec![*min, default, (*max).min(default.saturating_mul(4))];
                v.sort_unstable();
                v.dedup();
                v.into_iter().map(OptionValue::Int).collect()
            }
        }
    }

    pub(crate) fn check(&self, value: &OptionValue) -> Result<(), String> {
        match (&self.kind, value) {
            (OptionKind::Bool, OptionValue::Bool(_)) => Ok(()),
            (OptionKind::Int { min, max }, OptionValue::Int(n)) if (min..=max).contains(&n) => {
                Ok(())
            }
            (OptionKind::Int { min, max }, OptionValue::Int(_)) => {
                Err(format!("must be between {min} and {max}"))
            }
            (OptionKind::Int { .. }, _) => Err("must be an integer".into()),
            (OptionKind::Bool, _) => Err("must be true or false".into()),
            (OptionKind::Enum(values), OptionValue::String(s)) if values.contains(&s.as_str()) => {
                Ok(())
            }
            (OptionKind::Words, OptionValue::List(items)) if items.iter().all(is_word) => Ok(()),
            (OptionKind::WordSets, OptionValue::List(sets))
                if sets.iter().all(
                    |s| matches!(s, OptionValue::List(items) if items.iter().all(is_word)),
                ) =>
            {
                Ok(())
            }
            (OptionKind::WordSets, _) => {
                Err("must be a list of lists of words made of letters, digits and `_`".into())
            }
            (OptionKind::Words, _) => {
                Err("must be a list of words made of letters, digits and `_`".into())
            }
            (OptionKind::Enum(values), _) => Err(format!("must be one of: {}", values.join(", "))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("format option `{option}`: {message}")]
pub struct OptionError {
    pub option: String,
    pub message: String,
}

/// The options of one provider.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OptionSchema {
    defs: Vec<OptionDef>,
    presets: Vec<Preset>,
}

impl OptionSchema {
    pub fn new(defs: Vec<OptionDef>) -> Self {
        OptionSchema {
            defs,
            presets: Vec::new(),
        }
    }

    /// The presets (`extends: preset:...`) of this provider.
    pub fn with_presets(mut self, presets: Vec<Preset>) -> Self {
        self.presets = presets;
        self
    }

    pub fn presets(&self) -> &[Preset] {
        &self.presets
    }

    pub fn defs(&self) -> &[OptionDef] {
        &self.defs
    }

    pub fn get(&self, name: &str) -> Option<&OptionDef> {
        self.defs.iter().find(|d| d.name == name)
    }

    pub fn defaults(&self) -> ResolvedOptions {
        ResolvedOptions(
            self.defs
                .iter()
                .map(|d| (d.name.to_string(), d.default.clone()))
                .collect(),
        )
    }

    /// Checks `value` against the declaration of `name` and stores it.
    pub fn apply(
        &self,
        into: &mut ResolvedOptions,
        name: &str,
        value: &OptionValue,
    ) -> Result<(), OptionError> {
        let def = self.get(name).ok_or_else(|| OptionError {
            option: name.into(),
            message: unknown_name("option", name, self.defs.iter().map(|d| d.name)).replacen(
                &format!("unknown option `{name}`"),
                "unknown",
                1,
            ),
        })?;
        def.check(value).map_err(|message| OptionError {
            option: name.into(),
            message,
        })?;
        into.0.insert(name.into(), value.clone());
        Ok(())
    }
}

/// Validated option values for one file: every option of the schema has a value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedOptions(BTreeMap<String, OptionValue>);

impl ResolvedOptions {
    pub fn set(&mut self, name: &str, value: OptionValue) {
        self.0.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<&OptionValue> {
        self.0.get(name)
    }

    pub fn bool(&self, name: &str) -> bool {
        match self.0.get(name) {
            Some(OptionValue::Bool(b)) => *b,
            other => panic!("option `{name}` is not a boolean: {other:?}"),
        }
    }

    pub fn int(&self, name: &str) -> i64 {
        match self.0.get(name) {
            Some(OptionValue::Int(n)) => *n,
            other => panic!("option `{name}` is not an integer: {other:?}"),
        }
    }

    /// The value of `shared::max_blank_lines`.
    pub fn max_blank_lines(&self) -> usize {
        self.int(shared::MAX_BLANK_LINES) as usize
    }

    /// The strings of a list option; empty when the option is absent.
    pub fn strings(&self, name: &str) -> Vec<String> {
        match self.0.get(name) {
            Some(OptionValue::List(items)) => items
                .iter()
                .filter_map(|i| match i {
                    OptionValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The lists of strings of a list-of-lists option; empty when the option is absent.
    pub fn string_lists(&self, name: &str) -> Vec<Vec<String>> {
        match self.0.get(name) {
            Some(OptionValue::List(sets)) => sets
                .iter()
                .map(|set| match set {
                    OptionValue::List(items) => items
                        .iter()
                        .filter_map(|i| match i {
                            OptionValue::String(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn str(&self, name: &str) -> &str {
        match self.0.get(name) {
            Some(OptionValue::String(s)) => s,
            other => panic!("option `{name}` is not a string: {other:?}"),
        }
    }

    /// The string of an option; empty when the option is absent or not a string.
    pub fn text(&self, name: &str) -> &str {
        match self.0.get(name) {
            Some(OptionValue::String(s)) => s,
            _ => "",
        }
    }

    /// Does the string option have this value?
    pub fn is(&self, name: &str, value: &str) -> bool {
        self.text(name) == value
    }

    /// An integer option as a count; `fallback` when it is absent or negative.
    pub fn uint(&self, name: &str, fallback: usize) -> usize {
        match self.0.get(name) {
            Some(OptionValue::Int(n)) => usize::try_from(*n).unwrap_or(fallback),
            _ => fallback,
        }
    }

    /// A boolean option; `fallback` when it is absent.
    pub fn flag(&self, name: &str, fallback: bool) -> bool {
        match self.0.get(name) {
            Some(OptionValue::Bool(b)) => *b,
            _ => fallback,
        }
    }

    /// Does the list option hold this string?
    pub fn has(&self, name: &str, item: &str) -> bool {
        matches!(self.0.get(name), Some(OptionValue::List(items))
            if items.iter().any(|i| matches!(i, OptionValue::String(s) if s == item)))
    }

    /// Does the opening brace of this element start a line? `brace_newline` says where braces go
    /// in general (`all` or `none`), the `brace_same_line` list names the elements that keep
    /// theirs on the line anyway.
    pub fn brace_on_new_line(&self, element: &str) -> bool {
        self.is(shared::BRACE_NEWLINE, "all") && !self.has(shared::BRACE_SAME_LINE, element)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &OptionValue)> {
        self.0.iter()
    }
}

/// Definitions of the options every language shares (the top-level `format:` section). A provider
/// includes the ones it supports, with its own default.
pub mod shared {
    use super::OptionDef;

    pub const LINE_WIDTH: &str = "line_width";
    pub const INDENT_SIZE: &str = "indent_size";
    pub const INDENT_STYLE: &str = "indent_style";
    pub const FINAL_NEWLINE: &str = "final_newline";
    pub const END_OF_LINE: &str = "end_of_line";
    pub const BRACKET_SPACING: &str = "bracket_spacing";
    pub const ARRAY_BRACKET_SPACING: &str = "array_bracket_spacing";
    /// Not a top-level `format:` option: only providers that list it have it.
    pub const MAX_BLANK_LINES: &str = "max_blank_lines";
    pub const BRACE_NEWLINE: &str = "brace_newline";
    pub const BRACE_SAME_LINE: &str = "brace_same_line";
    pub const NAMES: [&str; 7] = [
        LINE_WIDTH,
        INDENT_SIZE,
        INDENT_STYLE,
        FINAL_NEWLINE,
        END_OF_LINE,
        BRACKET_SPACING,
        ARRAY_BRACKET_SPACING,
    ];

    pub fn line_width(default: i64) -> OptionDef {
        OptionDef::int(
            LINE_WIDTH,
            default,
            (20, 1000),
            "Preferred maximum line width in columns.",
        )
    }

    pub fn indent_size(default: i64) -> OptionDef {
        OptionDef::int(
            INDENT_SIZE,
            default,
            (1, 16),
            "Columns per indentation level (also the width of a tab).",
        )
    }

    pub fn indent_style() -> OptionDef {
        OptionDef::choice(
            INDENT_STYLE,
            &["space", "tab"],
            "space",
            "Indent with spaces or tabs.",
        )
    }

    pub fn final_newline(default: bool) -> OptionDef {
        OptionDef::boolean(FINAL_NEWLINE, default, "End the file with a newline.")
    }

    pub fn max_blank_lines(default: i64, description: &'static str) -> OptionDef {
        OptionDef::int(MAX_BLANK_LINES, default, (0, 10), description)
    }

    pub fn end_of_line() -> OptionDef {
        OptionDef::choice(
            END_OF_LINE,
            &["auto", "lf", "crlf"],
            "auto",
            "Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first              line break.",
        )
    }

    pub fn bracket_spacing(default: bool) -> OptionDef {
        OptionDef::boolean(
            BRACKET_SPACING,
            default,
            "Spaces inside the braces of a one-line mapping or object: `{ a: 1 }` or `{a: 1}`.",
        )
    }

    pub fn member_chain_style(default: &'static str) -> OptionDef {
        OptionDef::choice(
            "member_chain_style",
            &["fluent", "grouped", "wrapped", "soft"],
            default,
            "Layout of a chain of member accesses and calls that does not fit: `soft` keeps the accesses in front of the first call with the head and, once there are `member_chain_min_groups` groups after it, puts every access that follows a call or an index on a line of its own, without indentation of its own; `fluent` breaks before every dot once there are `member_chain_min_calls` calls; `grouped` keeps the head, its calls and indexes in the first group and puts a call group per line after it (a chain of up to two groups stays on one line); `wrapped` keeps the accesses in front of the first call with the head and puts every later dot on a line of its own, a call after a block or argument list that ends on a line of its own staying on that line.",
        )
    }

    pub fn member_chain_force_links(default: i64) -> OptionDef {
        OptionDef::int(
            "member_chain_force_links",
            default,
            (0, 1000),
            "A chain of member accesses and calls with at least this many dots always breaks (`wrapped` style; 0: never).",
        )
    }

    pub fn member_chain_min_groups(default: i64) -> OptionDef {
        OptionDef::int(
            "member_chain_min_groups",
            default,
            (0, 1000),
            "A chain of member accesses and calls with fewer groups after the one on the head's line is never broken at the dots (`soft` style).",
        )
    }

    pub fn member_chain_min_calls(default: i64) -> OptionDef {
        OptionDef::int(
            "member_chain_min_calls",
            default,
            (1, 1000),
            "A chain of member accesses and calls with at least this many calls is broken one call per line when it does not fit on a line (`fluent` style).",
        )
    }

    pub fn member_chain_keep_accesses(default: &'static str) -> OptionDef {
        OptionDef::choice(
            "member_chain_keep_accesses",
            &["never", "head", "always"],
            default,
            "In a broken member chain an access stays on the line of the call after it (`.a.b()` is one group) instead of starting a line of its own: `always`, or `head` in the first group only (which stays on the line of the head when the head name is short or a type name).",
        )
    }

    pub fn member_chain_break_on_block(default: &'static str) -> OptionDef {
        OptionDef::choice(
            "member_chain_break_on_block",
            &["never", "inner", "any"],
            default,
            "Break a member chain one call per line when it has an argument with a block body (a lambda or an anonymous class) even if the chain would fit: `inner` when a call other than the last has one, `any` when any call has one.",
        )
    }

    pub fn type_use_annotations(default: &[&str]) -> OptionDef {
        OptionDef::words(
            "type_use_annotations",
            default,
            "Annotations that belong to the type after them, not to the declaration (`@Nullable String name`): the fully qualified names the file imports them under, `*` standing for any text. They stay on the line of the declaration.",
        )
    }

    pub fn member_chain_head_calls(default: &[&str]) -> OptionDef {
        OptionDef::words(
            "member_chain_head_calls",
            default,
            "Calls that stay on the line of the head of a broken member chain when they are among its first two groups (`list.stream()`).",
        )
    }

    pub fn member_chain_short_head(default: i64) -> OptionDef {
        OptionDef::int(
            "member_chain_short_head",
            default,
            (0, 100),
            "A name of at most this many characters at the head of a broken member chain stays on the line of the first call (`x.foo()`); a capitalized name, `this`, `base`, `super` and a keyword type always do.",
        )
    }

    pub fn member_chain_indent(default: i64) -> OptionDef {
        OptionDef::int(
            "member_chain_indent",
            default,
            (0, 4),
            "Levels of indentation of the lines of a broken member chain.",
        )
    }

    pub fn assignment_layout(default: &'static str) -> OptionDef {
        OptionDef::choice(
            "assignment_layout",
            &["compact", "fluid", "break"],
            default,
            "Where the value after `=` (or `=>`) goes when the line is too long: `compact` moves a binary expression, a conditional or a query to the next line and leaves any other value on the line; `fluid` also moves strings, collection expressions and property initializers, and any other value whose first line does not fit after the operator (a string that spans lines stays on the operator's line; the body of an expression lambda moves below `=>`; chains `a = b = c` get layouts of their own); `break` moves every value that does not fit.",
        )
    }

    pub fn assignment_conditional(default: &'static str) -> OptionDef {
        OptionDef::choice(
            "assignment_conditional",
            &["always", "binary_test"],
            default,
            "A conditional (`a ? b : c`) as the value of an assignment moves below the operator when it does not fit: `always`, or `binary_test` only when its test is an operator chain.",
        )
    }

    pub fn assignment_indent(default: i64) -> OptionDef {
        OptionDef::int(
            "assignment_indent",
            default,
            (1, 4),
            "Levels of indentation of a value that moves below the operator of an assignment.",
        )
    }

    pub fn conditional_layout(default: &'static str) -> OptionDef {
        OptionDef::choice(
            "conditional_layout",
            &["indented", "chained"],
            default,
            "Layout of a conditional (`a ? b : c`) that does not fit: `indented` puts the branches one level below the test, each conditional a group of its own; `chained` aligns the branches after `? ` and `: ` and breaks a chain of conditionals in the `else` branch as one.",
        )
    }

    pub fn conditional_indent(default: i64) -> OptionDef {
        OptionDef::int(
            "conditional_indent",
            default,
            (1, 4),
            "Levels of indentation of the branches of a broken conditional in the `indented` layout.",
        )
    }

    pub fn binary_operator_line(default: &'static str) -> OptionDef {
        OptionDef::choice(
            "binary_operator_line",
            &["start", "end"],
            default,
            "Where an operator of a chain (`a + b + c`) that does not fit goes: `start` breaks before it and puts one operand per line; `end` keeps it at the end of the line and moves an operand down only when it does not fit (a chain that mixes node kinds breaks at every operator).",
        )
    }

    pub fn binary_indent(default: i64) -> OptionDef {
        OptionDef::int(
            "binary_indent",
            default,
            (1, 4),
            "Levels of indentation of the continuation lines of a broken operator chain.",
        )
    }

    pub fn binary_hug(default: &'static str) -> OptionDef {
        OptionDef::choice(
            "binary_hug",
            &["never", "assignment", "statement"],
            default,
            "A broken operator chain leaves the indentation of its continuation lines to the layout around it: `assignment` for the value of an assignment, declaration or entry, `statement` also for the value of `return` and `throw`.",
        )
    }

    pub fn hug_first_argument(default: bool) -> OptionDef {
        OptionDef::boolean(
            "hug_first_argument",
            default,
            "A function that is the first of two arguments stays on the call's line while its body breaks (`setTimeout(() => {...}, 500)`), when the other argument is short.",
        )
    }

    pub fn hug_last_argument(default: &'static str) -> OptionDef {
        OptionDef::choice(
            "hug_last_argument",
            &["never", "sole", "last"],
            default,
            "A function, object or array that is the last argument stays on the call's line while its body breaks: `last` for the last of any number of arguments, `sole` only for an argument that is alone, `never` not at all.",
        )
    }

    pub fn initializer_fill_width(default: i64) -> OptionDef {
        OptionDef::int(
            "initializer_fill_width",
            default,
            (0, 100),
            "The elements of an array initializer are filled (as many as fit on a line) when each is shorter than this many characters in the source; 0: never.",
        )
    }

    pub fn indent_nested_binary(default: bool) -> OptionDef {
        OptionDef::boolean(
            "indent_nested_binary",
            default,
            "Indent the operands of a binary expression that is itself an operand of a binary expression with an operator of another precedence (`a && b || c`).",
        )
    }

    pub fn array_bracket_spacing(default: bool) -> OptionDef {
        OptionDef::boolean(
            ARRAY_BRACKET_SPACING,
            default,
            "Spaces inside the brackets of a one-line sequence or array: `[ 1, 2 ]` or `[1, 2]`.",
        )
    }
}
impl OptionSchema {
    /// The preset named `language/name`.
    pub fn find_preset(&self, name: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.name == name)
    }
}

fn is_word(value: &OptionValue) -> bool {
    matches!(value, OptionValue::String(s)
        if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> OptionSchema {
        OptionSchema::new(vec![
            shared::line_width(80),
            shared::indent_size(2),
            OptionDef::choice("mode", &["a", "b"], "a", "A mode."),
        ])
    }

    #[test]
    fn defaults_and_checked_values() {
        let s = schema();
        let mut r = s.defaults();
        assert_eq!(r.int("line_width"), 80);
        s.apply(&mut r, "mode", &OptionValue::String("b".into()))
            .unwrap();
        assert_eq!(r.str("mode"), "b");
        for (name, value, needle) in [
            (
                "nope",
                OptionValue::Bool(true),
                "format option `nope`: unknown",
            ),
            (
                "line_widht",
                OptionValue::Int(90),
                "did you mean `line_width`",
            ),
            ("mode", OptionValue::String("c".into()), "one of: a, b"),
            ("line_width", OptionValue::Int(5), "between 20 and 1000"),
            ("line_width", OptionValue::Bool(true), "integer"),
        ] {
            let e = s.apply(&mut r, name, &value).unwrap_err();
            assert!(e.to_string().contains(needle), "{e}");
        }
    }
}
