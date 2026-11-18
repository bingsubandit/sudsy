// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const MEMBER_CHAIN_MIN_CALLS: &str = "member_chain_min_calls";
#[allow(dead_code)]
pub const MEMBER_CHAIN_KEEP_ACCESSES: &str = "member_chain_keep_accesses";
#[allow(dead_code)]
pub const MEMBER_CHAIN_BREAK_ON_BLOCK: &str = "member_chain_break_on_block";
#[allow(dead_code)]
pub const MEMBER_CHAIN_HEAD_CALLS: &str = "member_chain_head_calls";
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

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(2),
        core_config::shared::line_width(100),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::member_chain_min_calls(2),
        core_config::shared::member_chain_keep_accesses("head"),
        core_config::shared::member_chain_break_on_block("any"),
        core_config::shared::member_chain_head_calls(&["stream"]),
        core_config::shared::member_chain_short_head(4),
        core_config::shared::member_chain_indent(2),
        core_config::shared::binary_operator_line("start"),
        core_config::shared::binary_indent(2),
        core_config::shared::binary_hug("never"),
        core_config::shared::hug_first_argument(false),
        core_config::shared::hug_last_argument("sole"),
        core_config::shared::indent_nested_binary(true),
        core_config::shared::assignment_layout("break"),
        core_config::shared::assignment_conditional("always"),
        core_config::shared::assignment_indent(2),
        core_config::shared::conditional_indent(2),
        core_config::shared::conditional_layout("indented"),
    ])
    .with_presets(vec![
        core_config::Preset::new("java/google", include_str!("../presets/google.yaml")),
    ])
}
