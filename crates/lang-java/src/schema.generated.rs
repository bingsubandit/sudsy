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
        core_config::OptionDef::int("member_chain_min_calls", 2, (1, 1000), "A chain of member accesses and calls with at least this many calls is broken one call per line when it does not fit on a line."),
        core_config::OptionDef::choice("member_chain_keep_accesses", &["never", "head", "always"], "head", "In a broken member chain an access stays on the line of the call after it (`.a.b()` is one group) instead of starting a line of its own: `always`, or `head` in the first group only (which stays on the line of the head when the head name is short or a type name)."),
        core_config::OptionDef::choice("member_chain_break_on_block", &["never", "inner", "any"], "any", "Break a member chain one call per line when it has an argument with a block body (a lambda or an anonymous class) even if the chain would fit: `inner` when a call other than the last has one, `any` when any call has one."),
        core_config::OptionDef::words("member_chain_head_calls", &["stream"], "Calls that stay on the line of the head of a broken member chain when they are among its first two groups (`list.stream()`)."),
        core_config::OptionDef::int("member_chain_short_head", 4, (0, 100), "A name of at most this many characters at the head of a broken member chain stays on the line of the first call (`x.foo()`); a capitalized name, `this`, `super` and a keyword type always do."),
        core_config::OptionDef::int("member_chain_indent", 2, (1, 4), "Levels of indentation of the lines of a broken member chain."),
    ])
    .with_presets(vec![
        core_config::Preset::new("java/google", include_str!("../presets/google.yaml")),
    ])
}
