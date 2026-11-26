// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const SPACE_AROUND_ASSIGNMENT: core_config::OptId = core_config::opt::SPACE_AROUND_ASSIGNMENT;
#[allow(dead_code)]
pub const SPACE_BEFORE_COLON: core_config::OptId = core_config::opt::SPACE_BEFORE_COLON;
#[allow(dead_code)]
pub const KEEP_COMMENT_GAP: core_config::OptId = core_config::opt::KEEP_COMMENT_GAP;
#[allow(dead_code)]
pub const SPACED_ASSIGNMENTS: core_config::OptId = core_config::opt::SPACED_ASSIGNMENTS;
#[allow(dead_code)]
pub const CONDITIONAL_BODY_INDENT: core_config::OptId = core_config::opt::CONDITIONAL_BODY_INDENT;
#[allow(dead_code)]
pub const CONTINUED_LINE_INDENT: core_config::OptId = core_config::opt::CONTINUED_LINE_INDENT;
#[allow(dead_code)]
pub fn space_around_assignment(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_AROUND_ASSIGNMENT, true)
}
#[allow(dead_code)]
pub fn space_before_colon(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_BEFORE_COLON, false)
}
#[allow(dead_code)]
pub fn keep_comment_gap(o: &core_config::ResolvedOptions) -> bool {
    o.flag(KEEP_COMMENT_GAP, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpacedAssignments { All, Plain }
#[allow(dead_code)]
pub fn spaced_assignments(o: &core_config::ResolvedOptions) -> SpacedAssignments {
    match o.text(SPACED_ASSIGNMENTS) {
        "plain" => SpacedAssignments::Plain,
        _ => SpacedAssignments::All,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionalBodyIndent { Keep, Indent }
#[allow(dead_code)]
pub fn conditional_body_indent(o: &core_config::ResolvedOptions) -> ConditionalBodyIndent {
    match o.text(CONDITIONAL_BODY_INDENT) {
        "indent" => ConditionalBodyIndent::Indent,
        _ => ConditionalBodyIndent::Keep,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContinuedLineIndent { Keep, Indent, First }
#[allow(dead_code)]
pub fn continued_line_indent(o: &core_config::ResolvedOptions) -> ContinuedLineIndent {
    match o.text(CONTINUED_LINE_INDENT) {
        "indent" => ContinuedLineIndent::Indent,
        "first" => ContinuedLineIndent::First,
        _ => ContinuedLineIndent::Keep,
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
        core_config::OptionDef::boolean(core_config::opt::KEEP_COMMENT_GAP, false, "A comment after code keeps the blanks the source has before it when they are more than one."),
        core_config::OptionDef::choice(core_config::opt::SPACED_ASSIGNMENTS, &["all", "plain"], "all", "The assignments `space_around_assignment` applies to: `all`; `plain`: those at the start of a line whose name is letters, digits and underscores, with no substitution reference `$(v:a=b)` on the line and no colon in the first word of a value that follows `=` at once (`A=b:c`, unless a URL, a time or a path). Any other (an indented one, one after `export` or `override`, a target-specific one, a name with `-`, `.`, `/` or a reference) keeps its blanks as written."),
        core_config::OptionDef::choice(core_config::opt::CONDITIONAL_BODY_INDENT, &["keep", "indent"], "keep", "The lines inside a conditional (`ifeq` ... `endif`) outside a recipe: `keep` leaves their indentation as written; `indent` indents the body one level (`indent_size`, `indent_style`) per nesting."),
        core_config::OptionDef::choice(core_config::opt::CONTINUED_LINE_INDENT, &["keep", "indent", "first"], "keep", "The lines after the first of a rule, directive, variable value or recipe command written over several lines: `keep` leaves their indentation as written; `indent` indents the lines of a rule or directive one level; `first` gives every line the indentation of the second (a recipe command that has a shell block keyword, and a line that starts inside quotes, keep theirs)."),
        ])
        .with_presets(vec![
        core_config::Preset::new("makefile/checkmake", include_str!("../presets/checkmake.yaml")),
        core_config::Preset::new("makefile/mbake", include_str!("../presets/mbake.yaml")),
        ])
    })
}
