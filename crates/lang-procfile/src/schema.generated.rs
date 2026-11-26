// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const KEEP_FINAL_NEWLINE: core_config::OptId = core_config::opt::KEEP_FINAL_NEWLINE;
#[allow(dead_code)]
pub fn keep_final_newline(o: &core_config::ResolvedOptions) -> bool {
    o.flag(KEEP_FINAL_NEWLINE, true)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::final_newline(true),
        core_config::OptionDef::boolean(core_config::opt::KEEP_FINAL_NEWLINE, true, "A file that does not end with a line break stays without one (the final newline is not added)."),
        core_config::OptionDef::choice(core_config::opt::END_OF_LINE, &["auto", "lf", "crlf"], "lf", "Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break."),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between processes and comments."),
        ])
        .with_presets(vec![
        ])
    })
}
