// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const MEMBER_CHAIN_STYLE: &str = "member_chain_style";
#[allow(dead_code)]
pub const MEMBER_CHAIN_MIN_GROUPS: &str = "member_chain_min_groups";
#[allow(dead_code)]
pub const MEMBER_CHAIN_KEEP_ACCESSES: &str = "member_chain_keep_accesses";
#[allow(dead_code)]
pub const MEMBER_CHAIN_INDENT: &str = "member_chain_indent";
#[allow(dead_code)]
pub const QUOTE_STYLE: &str = "quote_style";
#[allow(dead_code)]
pub const MAGIC_TRAILING_COMMA: &str = "magic_trailing_comma";
#[allow(dead_code)]
pub const TARGET_VERSION: &str = "target_version";
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteStyle { Double, Single, Preserve }
#[allow(dead_code)]
pub fn quote_style(o: &core_config::ResolvedOptions) -> QuoteStyle {
    match o.text(QUOTE_STYLE) {
        "single" => QuoteStyle::Single,
        "preserve" => QuoteStyle::Preserve,
        _ => QuoteStyle::Double,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagicTrailingComma { Respect, Ignore }
#[allow(dead_code)]
pub fn magic_trailing_comma(o: &core_config::ResolvedOptions) -> MagicTrailingComma {
    match o.text(MAGIC_TRAILING_COMMA) {
        "ignore" => MagicTrailingComma::Ignore,
        _ => MagicTrailingComma::Respect,
    }
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::line_width(88),
        core_config::shared::indent_size(4),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::member_chain_style("soft"),
        core_config::shared::member_chain_min_groups(2),
        core_config::shared::member_chain_keep_accesses("always"),
        core_config::shared::member_chain_indent(0),
        core_config::OptionDef::choice("quote_style", &["double", "single", "preserve"], "double", "Quotes of strings (and of f-string and t-string literals): `double` or `single` rewrite a string to that quote unless the other one needs fewer escapes, `preserve` keeps what the source has."),
        core_config::OptionDef::choice("magic_trailing_comma", &["respect", "ignore"], "respect", "`respect` keeps a collection, call or parameter list that has a trailing comma expanded one item per line; `ignore` joins it when it fits and removes the comma."),
        core_config::OptionDef::choice("target_version", &["3.9", "3.10", "3.11", "3.12", "3.13", "3.14", "3.15"], "3.14", "The Python version whose syntax the file must follow: newer syntax is a syntax error (`lazy import` needs 3.15)."),
        ])
        .with_presets(vec![
        core_config::Preset::new("python/ruff", include_str!("../presets/ruff.yaml")),
        ])
    })
}
