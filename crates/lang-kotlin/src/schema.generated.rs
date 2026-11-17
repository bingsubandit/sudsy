// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const TRAILING_COMMAS: &str = "trailing_commas";
#[allow(dead_code)]
pub const FUNCTION_SIGNATURE_WRAP_MIN_PARAMETERS: &str = "function_signature_wrap_min_parameters";
#[allow(dead_code)]
pub const CLASS_SIGNATURE_WRAP_MIN_PARAMETERS: &str = "class_signature_wrap_min_parameters";
#[allow(dead_code)]
pub const ASSIGNMENT_LAYOUT: &str = "assignment_layout";
#[allow(dead_code)]
pub const ASSIGNMENT_CONDITIONAL: &str = "assignment_conditional";
#[allow(dead_code)]
pub const ASSIGNMENT_INDENT: &str = "assignment_indent";

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(4),
        core_config::shared::line_width(140),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::choice("trailing_commas", &["never", "multiline"], "multiline", "Trailing comma after the last argument or parameter of a call or declaration that is laid out on several lines: `multiline` adds one, `never` removes it."),
        core_config::OptionDef::int("function_signature_wrap_min_parameters", 2, (1, 1000), "A function or constructor with at least this many parameters puts each on a line of its own, even when the signature fits."),
        core_config::OptionDef::int("class_signature_wrap_min_parameters", 1, (1, 1000), "A class with at least this many primary-constructor parameters puts each on a line of its own, even when the signature fits."),
        core_config::shared::assignment_layout("break"),
        core_config::shared::assignment_conditional("always"),
        core_config::shared::assignment_indent(1),
    ])
    .with_presets(vec![
    ])
}
