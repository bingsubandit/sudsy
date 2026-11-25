// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const INDENT_CONVERSION: core_config::OptId = core_config::opt::INDENT_CONVERSION;
#[allow(dead_code)]
pub const CONTINUED_INDENT: core_config::OptId = core_config::opt::CONTINUED_INDENT;
#[allow(dead_code)]
pub const SPACE_AFTER_COLONS: core_config::OptId = core_config::opt::SPACE_AFTER_COLONS;
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentConversion { AsWritten, Spaces, Tabs }
#[allow(dead_code)]
pub fn indent_conversion(o: &core_config::ResolvedOptions) -> IndentConversion {
    match o.text(INDENT_CONVERSION) {
        "spaces" => IndentConversion::Spaces,
        "tabs" => IndentConversion::Tabs,
        _ => IndentConversion::AsWritten,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContinuedIndent { AsWritten, None }
#[allow(dead_code)]
pub fn continued_indent(o: &core_config::ResolvedOptions) -> ContinuedIndent {
    match o.text(CONTINUED_INDENT) {
        "none" => ContinuedIndent::None,
        _ => ContinuedIndent::AsWritten,
    }
}
#[allow(dead_code)]
pub fn space_after_colons(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_AFTER_COLONS, true)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(4),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between lines."),
        core_config::OptionDef::choice(core_config::opt::INDENT_CONVERSION, &["as_written", "spaces", "tabs"], "as_written", "Indentation of a line: as written, or tabs turned into spaces, or spaces into tabs (a tab is `indent_size` columns)."),
        core_config::OptionDef::choice(core_config::opt::CONTINUED_INDENT, &["as_written", "none"], "as_written", "Indentation of a line that continues the one before it with `^`: as written, or none when a blank stands before the `^` (the blanks then separate arguments and are not content)."),
        core_config::OptionDef::boolean(core_config::opt::SPACE_AFTER_COLONS, true, "At least one blank after `::` before a comment that starts with a letter or digit (more blanks stay as written)."),
        ])
        .with_presets(vec![
        ])
    })
}
