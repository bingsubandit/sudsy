// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const SEMI: &str = "semi";
#[allow(dead_code)]
pub const QUOTE_STYLE: &str = "quote_style";
#[allow(dead_code)]
pub const QUOTE_PROPS: &str = "quote_props";
#[allow(dead_code)]
pub const TRAILING_COMMAS: &str = "trailing_commas";
#[allow(dead_code)]
pub const MEMBER_CHAIN_STYLE: &str = "member_chain_style";
#[allow(dead_code)]
pub const MEMBER_CHAIN_MIN_CALLS: &str = "member_chain_min_calls";
#[allow(dead_code)]
pub const MEMBER_CHAIN_SHORT_HEAD: &str = "member_chain_short_head";
#[allow(dead_code)]
pub const MEMBER_CHAIN_INDENT: &str = "member_chain_indent";
#[allow(dead_code)]
pub const BINARY_OPERATOR_LINE: &str = "binary_operator_line";
#[allow(dead_code)]
pub const BINARY_INDENT: &str = "binary_indent";
#[allow(dead_code)]
pub const BINARY_HUG: &str = "binary_hug";
#[allow(dead_code)]
pub const HUG_FIRST_ARGUMENT: &str = "hug_first_argument";
#[allow(dead_code)]
pub const HUG_LAST_ARGUMENT: &str = "hug_last_argument";
#[allow(dead_code)]
pub const INDENT_NESTED_BINARY: &str = "indent_nested_binary";
#[allow(dead_code)]
pub const ASSIGNMENT_LAYOUT: &str = "assignment_layout";
#[allow(dead_code)]
pub const ASSIGNMENT_CONDITIONAL: &str = "assignment_conditional";
#[allow(dead_code)]
pub const ASSIGNMENT_INDENT: &str = "assignment_indent";
#[allow(dead_code)]
pub const CONDITIONAL_INDENT: &str = "conditional_indent";
#[allow(dead_code)]
pub const CONDITIONAL_LAYOUT: &str = "conditional_layout";
#[allow(dead_code)]
pub const ARROW_PARENS: &str = "arrow_parens";
#[allow(dead_code)]
pub const BRACKET_SAME_LINE: &str = "bracket_same_line";
#[allow(dead_code)]
pub const JSX_SINGLE_QUOTE: &str = "jsx_single_quote";
#[allow(dead_code)]
pub fn semi(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SEMI, true)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteStyle { Double, Single }
#[allow(dead_code)]
pub fn quote_style(o: &core_config::ResolvedOptions) -> QuoteStyle {
    match o.text(QUOTE_STYLE) {
        "single" => QuoteStyle::Single,
        _ => QuoteStyle::Double,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteProps { AsNeeded, Consistent, Preserve }
#[allow(dead_code)]
pub fn quote_props(o: &core_config::ResolvedOptions) -> QuoteProps {
    match o.text(QUOTE_PROPS) {
        "consistent" => QuoteProps::Consistent,
        "preserve" => QuoteProps::Preserve,
        _ => QuoteProps::AsNeeded,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingCommas { All, Es5, None }
#[allow(dead_code)]
pub fn trailing_commas(o: &core_config::ResolvedOptions) -> TrailingCommas {
    match o.text(TRAILING_COMMAS) {
        "es5" => TrailingCommas::Es5,
        "none" => TrailingCommas::None,
        _ => TrailingCommas::All,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowParens { Always, Avoid }
#[allow(dead_code)]
pub fn arrow_parens(o: &core_config::ResolvedOptions) -> ArrowParens {
    match o.text(ARROW_PARENS) {
        "avoid" => ArrowParens::Avoid,
        _ => ArrowParens::Always,
    }
}
#[allow(dead_code)]
pub fn bracket_same_line(o: &core_config::ResolvedOptions) -> bool {
    o.flag(BRACKET_SAME_LINE, false)
}
#[allow(dead_code)]
pub fn jsx_single_quote(o: &core_config::ResolvedOptions) -> bool {
    o.flag(JSX_SINGLE_QUOTE, false)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::line_width(80),
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::boolean("semi", true, "End statements with `;`. When false a statement that starts with `(`, `[`, a backtick, `+`, `-`, `/` or `<` gets a `;` in front, as a class property does before a member that starts with `[`, `(` or `*`."),
        core_config::OptionDef::choice("quote_style", &["double", "single"], "double", "Quotes of strings (not JSX attributes). A string with more of the preferred quote than of the other keeps the other one, to need fewer escapes."),
        core_config::OptionDef::choice("quote_props", &["as_needed", "consistent", "preserve"], "as_needed", "Quotes around property names of objects and destructuring patterns: `as_needed` removes quotes that are not required (`\"a\": 1` becomes `a: 1`), `consistent` quotes every name of an object when one of its names requires quotes and removes them otherwise, `preserve` keeps what the source has. Numeric names are left as written."),
        core_config::OptionDef::choice("trailing_commas", &["all", "es5", "none"], "all", "Trailing comma of a list that breaks over several lines: `all` after every list including function parameters and call arguments, `es5` after objects, arrays, destructuring and import/export lists only, `none` never."),
        core_config::shared::member_chain_style("grouped"),
        core_config::shared::member_chain_min_calls(3),
        core_config::shared::member_chain_short_head(2),
        core_config::shared::member_chain_indent(1),
        core_config::shared::binary_operator_line("end"),
        core_config::shared::binary_indent(1),
        core_config::shared::binary_hug("statement"),
        core_config::shared::hug_first_argument(true),
        core_config::shared::hug_last_argument("last"),
        core_config::shared::indent_nested_binary(true),
        core_config::shared::assignment_layout("fluid"),
        core_config::shared::assignment_conditional("binary_test"),
        core_config::shared::assignment_indent(1),
        core_config::shared::conditional_indent(1),
        core_config::shared::conditional_layout("chained"),
        core_config::shared::bracket_spacing(true),
        core_config::OptionDef::choice("arrow_parens", &["always", "avoid"], "always", "Parentheses around the lone parameter of an arrow function: `always` writes `(x) => x`, `avoid` writes `x => x` when the parameter is a plain name (no type, default, rest or destructuring)."),
        core_config::OptionDef::boolean("bracket_same_line", false, "JSX: put the `>` of a multi-line element on the line of its last attribute instead of on a line of its own."),
        core_config::OptionDef::boolean("jsx_single_quote", false, "Single quotes for JSX attribute strings (a string with more of the preferred quote than of the other keeps the other one)."),
        ])
        .with_presets(vec![
        core_config::Preset::new("js/eslint-recommended", include_str!("../presets/eslint-recommended.yaml")),
        core_config::Preset::new("js/typescript-eslint-recommended", include_str!("../presets/typescript-eslint-recommended.yaml")),
        ])
    })
}
