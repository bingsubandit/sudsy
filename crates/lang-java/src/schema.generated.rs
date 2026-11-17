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
    ])
    .with_presets(vec![
        core_config::Preset::new("java/google", include_str!("../presets/google.yaml")),
    ])
}
