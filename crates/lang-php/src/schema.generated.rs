// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const TRAILING_COMMAS: &str = "trailing_commas";
#[allow(dead_code)]
pub const DECLARATION_BRACE: &str = "declaration_brace";
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingCommas { Never, Multiline }
#[allow(dead_code)]
pub fn trailing_commas(o: &core_config::ResolvedOptions) -> TrailingCommas {
    match o.text(TRAILING_COMMAS) {
        "never" => TrailingCommas::Never,
        _ => TrailingCommas::Multiline,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclarationBrace { NextLine, SameLine }
#[allow(dead_code)]
pub fn declaration_brace(o: &core_config::ResolvedOptions) -> DeclarationBrace {
    match o.text(DECLARATION_BRACE) {
        "same_line" => DeclarationBrace::SameLine,
        _ => DeclarationBrace::NextLine,
    }
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(4),
        core_config::shared::line_width(120),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::choice("trailing_commas", &["never", "multiline"], "multiline", "Trailing comma after the last argument, parameter or `match` arm of a list that is laid out on several lines: `multiline` adds one, `never` removes it."),
        core_config::OptionDef::choice("declaration_brace", &["next_line", "same_line"], "next_line", "Where the `{` of a class, interface, trait, enum, function or method goes (PER Coding Style: its own line). Control structures, closures and anonymous classes keep it on the line."),
        ])
        .with_presets(vec![
        core_config::Preset::new("php/phpstan", include_str!("../presets/phpstan.yaml")),
        ])
    })
}
