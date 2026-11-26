// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const INDENT_CONVERSION: core_config::OptId = core_config::opt::INDENT_CONVERSION;
#[allow(dead_code)]
pub const SPACE_AFTER_PERCENT: core_config::OptId = core_config::opt::SPACE_AFTER_PERCENT;
#[allow(dead_code)]
pub const SPACE_AFTER_HASHES: core_config::OptId = core_config::opt::SPACE_AFTER_HASHES;
#[allow(dead_code)]
pub const KEEP_LEADING_BLANK_LINES: core_config::OptId = core_config::opt::KEEP_LEADING_BLANK_LINES;
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentConversion { AsWritten, Spaces, Tabs }
#[allow(dead_code)]
pub fn indent_conversion(o: &core_config::ResolvedOptions) -> IndentConversion {
    match o.text(INDENT_CONVERSION) {
        "as_written" => IndentConversion::AsWritten,
        "tabs" => IndentConversion::Tabs,
        _ => IndentConversion::Spaces,
    }
}
#[allow(dead_code)]
pub fn space_after_percent(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_AFTER_PERCENT, true)
}
#[allow(dead_code)]
pub fn space_after_hashes(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_AFTER_HASHES, true)
}
#[allow(dead_code)]
pub fn keep_leading_blank_lines(o: &core_config::ResolvedOptions) -> bool {
    o.flag(KEEP_LEADING_BLANK_LINES, false)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(4),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between lines."),
        core_config::OptionDef::choice(core_config::opt::INDENT_CONVERSION, &["as_written", "spaces", "tabs"], "spaces", "Indentation of a line: as written, or tabs turned into spaces, or spaces into tabs (a tab is `indent_size` columns)."),
        core_config::OptionDef::boolean(core_config::opt::SPACE_AFTER_PERCENT, true, "At least one blank after the `%` of a control line (`% for x in y:`); more blanks stay as written."),
        core_config::OptionDef::boolean(core_config::opt::SPACE_AFTER_HASHES, true, "At least one blank after `##` before a comment that starts with a letter or digit (more blanks stay as written)."),
        core_config::OptionDef::boolean(core_config::opt::KEEP_LEADING_BLANK_LINES, false, "Blank lines at the start of the file stay."),
        ])
        .with_presets(vec![
        ])
    })
}
