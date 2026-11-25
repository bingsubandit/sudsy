// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const COMMENT_COLUMN: core_config::OptId = core_config::opt::COMMENT_COLUMN;
#[allow(dead_code)]
pub const HEREDOC_INDENT: core_config::OptId = core_config::opt::HEREDOC_INDENT;
#[allow(dead_code)]
pub const BINARY_OPERATOR_LINE: core_config::OptId = core_config::opt::BINARY_OPERATOR_LINE;
#[allow(dead_code)]
pub const SPACE_REDIRECTS: core_config::OptId = core_config::opt::SPACE_REDIRECTS;
#[allow(dead_code)]
pub const INDENT_CASE_ARMS: core_config::OptId = core_config::opt::INDENT_CASE_ARMS;
#[allow(dead_code)]
pub const CASE_LAST_TERMINATOR: core_config::OptId = core_config::opt::CASE_LAST_TERMINATOR;
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentColumn { Keep, Align }
#[allow(dead_code)]
pub fn comment_column(o: &core_config::ResolvedOptions) -> CommentColumn {
    match o.text(COMMENT_COLUMN) {
        "align" => CommentColumn::Align,
        _ => CommentColumn::Keep,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeredocIndent { Keep, Reindent }
#[allow(dead_code)]
pub fn heredoc_indent(o: &core_config::ResolvedOptions) -> HeredocIndent {
    match o.text(HEREDOC_INDENT) {
        "reindent" => HeredocIndent::Reindent,
        _ => HeredocIndent::Keep,
    }
}
#[allow(dead_code)]
pub fn space_redirects(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_REDIRECTS, false)
}
#[allow(dead_code)]
pub fn indent_case_arms(o: &core_config::ResolvedOptions) -> bool {
    o.flag(INDENT_CASE_ARMS, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseLastTerminator { Keep, Add }
#[allow(dead_code)]
pub fn case_last_terminator(o: &core_config::ResolvedOptions) -> CaseLastTerminator {
    match o.text(CASE_LAST_TERMINATOR) {
        "add" => CaseLastTerminator::Add,
        _ => CaseLastTerminator::Keep,
    }
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(4),
        core_config::OptionDef::choice(core_config::opt::INDENT_STYLE, &["space", "tab"], "tab", "Indent with spaces or tabs."),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between statements and comments."),
        core_config::OptionDef::choice(core_config::opt::COMMENT_COLUMN, &["keep", "align"], "keep", "A comment after code: `keep` leaves the blanks before it as written (at least one); `align` writes one blank, or pads it so that the trailing comments of consecutive lines start in one column."),
        core_config::OptionDef::choice(core_config::opt::HEREDOC_INDENT, &["keep", "reindent"], "keep", "The body and the delimiter of a `<<-` heredoc: `keep` leaves them as written; `reindent` drops their leading tabs (which `<<-` ignores) and indents the body one level and the delimiter as deep as the command (tab indentation only)."),
        core_config::shared::binary_operator_line("end"),
        core_config::OptionDef::boolean(core_config::opt::SPACE_REDIRECTS, false, "A blank after the operator of a redirect that opens a file (`> file`, `>> file`, `< file`; shfmt's `-sr`); `>&2` and `2>&1` never get one."),
        core_config::OptionDef::boolean(core_config::opt::INDENT_CASE_ARMS, false, "The arms of a `case` one level inside it (shfmt's `-ci`)."),
        core_config::OptionDef::choice(core_config::opt::CASE_LAST_TERMINATOR, &["keep", "add"], "keep", "The last arm of a `case` without `;;`: `keep` leaves it as written, `add` ends it with `;;`."),
        ])
        .with_presets(vec![
        core_config::Preset::new("shell/shellcheck", include_str!("../presets/shellcheck.yaml")),
        core_config::Preset::new("shell/shfmt", include_str!("../presets/shfmt.yaml")),
        ])
    })
}
