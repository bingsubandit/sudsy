// Generated from lang.yaml by `cargo xtask codegen`.

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(4),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between lines."),
        ])
        .with_presets(vec![
        core_config::Preset::new("gradle/modernize", include_str!("../presets/modernize.yaml")),
        core_config::Preset::new("gradle/codenarc", include_str!("../presets/codenarc.yaml")),
        ])
    })
}
