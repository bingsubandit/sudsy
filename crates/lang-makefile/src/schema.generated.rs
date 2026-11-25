// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const SPACE_AROUND_ASSIGNMENT: core_config::OptId = core_config::opt::SPACE_AROUND_ASSIGNMENT;
#[allow(dead_code)]
pub const SPACE_BEFORE_COLON: core_config::OptId = core_config::opt::SPACE_BEFORE_COLON;
#[allow(dead_code)]
pub const CONDITIONAL_INDENT: core_config::OptId = core_config::opt::CONDITIONAL_INDENT;
#[allow(dead_code)]
pub const CONTINUATION_INDENT: core_config::OptId = core_config::opt::CONTINUATION_INDENT;
#[allow(dead_code)]
pub fn space_around_assignment(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_AROUND_ASSIGNMENT, true)
}
#[allow(dead_code)]
pub fn space_before_colon(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_BEFORE_COLON, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionalIndent { Keep, Indent }
#[allow(dead_code)]
pub fn conditional_indent(o: &core_config::ResolvedOptions) -> ConditionalIndent {
    match o.text(CONDITIONAL_INDENT) {
        "indent" => ConditionalIndent::Indent,
        _ => ConditionalIndent::Keep,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContinuationIndent { Keep, Indent }
#[allow(dead_code)]
pub fn continuation_indent(o: &core_config::ResolvedOptions) -> ContinuationIndent {
    match o.text(CONTINUATION_INDENT) {
        "indent" => ContinuationIndent::Indent,
        _ => ContinuationIndent::Keep,
    }
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::boolean(core_config::opt::SPACE_AROUND_ASSIGNMENT, true, "Write `A := b` rather than `A:=b`."),
        core_config::OptionDef::boolean(core_config::opt::SPACE_BEFORE_COLON, false, "Write `target : prerequisite` rather than `target: prerequisite`."),
        core_config::shared::max_blank_lines(2, "Largest number of blank lines kept between statements and comments."),
        core_config::OptionDef::choice(core_config::opt::CONDITIONAL_INDENT, &["keep", "indent"], "keep", "The lines inside a conditional (`ifeq` ... `endif`) outside a recipe: `keep` leaves their indentation as written; `indent` indents the body one level (`indent_size`, `indent_style`) per nesting."),
        core_config::OptionDef::choice(core_config::opt::CONTINUATION_INDENT, &["keep", "indent"], "keep", "The lines after the first of a rule or directive written over several lines: `keep` leaves their indentation as written; `indent` indents them one level."),
        ])
        .with_presets(vec![
        core_config::Preset::new("makefile/checkmake", include_str!("../presets/checkmake.yaml")),
        core_config::Preset::new("makefile/mbake", include_str!("../presets/mbake.yaml")),
        ])
    })
}
