// Generated from lang.yaml by `cargo xtask codegen`.

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(2),
        core_config::shared::line_width(100),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
    ])
    .with_presets(vec![
    ])
}
