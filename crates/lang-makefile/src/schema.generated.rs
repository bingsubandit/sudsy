// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const SPACE_AROUND_ASSIGNMENT: &str = "space_around_assignment";
#[allow(dead_code)]
pub const SPACE_BEFORE_COLON: &str = "space_before_colon";
#[allow(dead_code)]
pub fn space_around_assignment(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_AROUND_ASSIGNMENT, true)
}
#[allow(dead_code)]
pub fn space_before_colon(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_BEFORE_COLON, false)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::boolean("space_around_assignment", true, "Write `A := b` rather than `A:=b`."),
        core_config::OptionDef::boolean("space_before_colon", false, "Write `target : prerequisite` rather than `target: prerequisite`."),
        core_config::shared::max_blank_lines(2, "Largest number of blank lines kept between statements and comments."),
        ])
        .with_presets(vec![
        core_config::Preset::new("makefile/checkmake", include_str!("../presets/checkmake.yaml")),
        ])
    })
}
