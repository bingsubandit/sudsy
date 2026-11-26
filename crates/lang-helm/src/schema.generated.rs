// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const ALIGN_CONTROL_ACTIONS: core_config::OptId = core_config::opt::ALIGN_CONTROL_ACTIONS;
#[allow(dead_code)]
pub fn align_control_actions(o: &core_config::ResolvedOptions) -> bool {
    o.flag(ALIGN_CONTROL_ACTIONS, true)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::boolean(core_config::opt::ALIGN_CONTROL_ACTIONS, true, "Own-line control actions with a left trim (`{{- if }}`, `{{- else }}`, `{{- end }}`) stand at the indent of the content they open; `else` and `end` stand where their opener stands. The trim removes the blanks in front of the action, so the output text does not change."),
        ])
        .with_presets(vec![
        ])
    })
}
