// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const TRAILING_COMMAS: &str = "trailing_commas";
#[allow(dead_code)]
pub const FUNCTION_SIGNATURE_WRAP_MIN_PARAMETERS: &str = "function_signature_wrap_min_parameters";
#[allow(dead_code)]
pub const CLASS_SIGNATURE_WRAP_MIN_PARAMETERS: &str = "class_signature_wrap_min_parameters";
#[allow(dead_code)]
pub const CLASS_SIGNATURE_WRAP_MIN_SUPERTYPES: &str = "class_signature_wrap_min_supertypes";
#[allow(dead_code)]
pub const ASSIGNMENT_LAYOUT: &str = "assignment_layout";
#[allow(dead_code)]
pub const ASSIGNMENT_CONDITIONAL: &str = "assignment_conditional";
#[allow(dead_code)]
pub const ASSIGNMENT_INDENT: &str = "assignment_indent";
#[allow(dead_code)]
pub const ASSIGNMENT_LEFT_MIN_PARAMETERS: &str = "assignment_left_min_parameters";
#[allow(dead_code)]
pub const ASSIGNMENT_LEFT_UNLESS_KINDS: &str = "assignment_left_unless_kinds";
#[allow(dead_code)]
pub const ASSIGNMENT_KEEP_BREAK_KINDS: &str = "assignment_keep_break_kinds";
#[allow(dead_code)]
pub const ANNOTATION_INLINE: &str = "annotation_inline";
#[allow(dead_code)]
pub const SEMICOLON_KEEP_PARENTS: &str = "semicolon_keep_parents";
#[allow(dead_code)]
pub const TRAILING_COMMA_BEFORE: &str = "trailing_comma_before";
#[allow(dead_code)]
pub const TRAILING_COMMA_LAST_ENTRY: &str = "trailing_comma_last_entry";
#[allow(dead_code)]
pub const SINGLE_LINE_BODY_KINDS: &str = "single_line_body_kinds";
#[allow(dead_code)]
pub const RAW_STRING_CALLS: &str = "raw_string_calls";
#[allow(dead_code)]
pub const CTOR_OWN_LINE_KINDS: &str = "ctor_own_line_kinds";
#[allow(dead_code)]
pub const COMMENT_TRIM_LINES: &str = "comment_trim_lines";
#[allow(dead_code)]
pub const COMMENT_REINDENT_LINES: &str = "comment_reindent_lines";
#[allow(dead_code)]
pub const FORMAT_RULES: &str = "format_rules";
#[allow(dead_code)]
pub const REDUNDANT_RETURN_TYPES: &str = "redundant_return_types";
#[allow(dead_code)]
pub const CURLY_STYLE: &str = "curly_style";
#[allow(dead_code)]
pub const SETTLE_KEYWORDS: &str = "settle_keywords";
#[allow(dead_code)]
pub const SIGNATURE_WRAP_ANNOTATED: &str = "signature_wrap_annotated";
#[allow(dead_code)]
pub const ANNOTATION_SOURCE_KINDS: &str = "annotation_source_kinds";
#[allow(dead_code)]
pub const MEMBER_CHAIN_STYLE: &str = "member_chain_style";
#[allow(dead_code)]
pub const MEMBER_CHAIN_MIN_CALLS: &str = "member_chain_min_calls";
#[allow(dead_code)]
pub const MEMBER_CHAIN_FORCE_LINKS: &str = "member_chain_force_links";
#[allow(dead_code)]
pub const MEMBER_CHAIN_INDENT: &str = "member_chain_indent";
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingCommas { Never, Multiline }
#[allow(dead_code)]
pub fn trailing_commas(o: &core_config::ResolvedOptions) -> TrailingCommas {
    match o.text(TRAILING_COMMAS) {
        "never" => TrailingCommas::Never,
        _ => TrailingCommas::Multiline,
    }
}
#[allow(dead_code)]
pub fn function_signature_wrap_min_parameters(o: &core_config::ResolvedOptions) -> usize {
    o.uint(FUNCTION_SIGNATURE_WRAP_MIN_PARAMETERS, 2)
}
#[allow(dead_code)]
pub fn class_signature_wrap_min_parameters(o: &core_config::ResolvedOptions) -> usize {
    o.uint(CLASS_SIGNATURE_WRAP_MIN_PARAMETERS, 1)
}
#[allow(dead_code)]
pub fn class_signature_wrap_min_supertypes(o: &core_config::ResolvedOptions) -> usize {
    o.uint(CLASS_SIGNATURE_WRAP_MIN_SUPERTYPES, 2)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::indent_size(4),
        core_config::shared::line_width(140),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::choice("trailing_commas", &["never", "multiline"], "multiline", "Trailing comma after the last argument or parameter of a call or declaration that is laid out on several lines: `multiline` adds one, `never` removes it."),
        core_config::OptionDef::int("function_signature_wrap_min_parameters", 2, (1, 1000), "A function or constructor with at least this many parameters puts each on a line of its own, even when the signature fits."),
        core_config::OptionDef::int("class_signature_wrap_min_parameters", 1, (1, 1000), "A class with at least this many primary-constructor parameters puts each on a line of its own, even when the signature fits."),
        core_config::OptionDef::int("class_signature_wrap_min_supertypes", 2, (1, 1000), "A class or object with at least this many supertypes puts each on a line of its own."),
        core_config::shared::assignment_layout("multiline"),
        core_config::shared::assignment_conditional("always"),
        core_config::shared::assignment_indent(1),
        core_config::shared::assignment_left_min_parameters(2),
        core_config::shared::assignment_left_unless_kinds(&["Colon"]),
        core_config::shared::assignment_keep_break_kinds(&["Property", "BinaryExpression"]),
        core_config::shared::annotation_inline("single"),
        core_config::shared::semicolon_keep_parents(&["ClassBody", "EnumEntry"]),
        core_config::shared::trailing_comma_before(&["WhenEntry", "->"]),
        core_config::shared::trailing_comma_last_entry(&["ClassBody", "EnumEntry"]),
        core_config::shared::single_line_body_kinds(&["EnumEntry"]),
        core_config::shared::raw_string_calls(&["trimIndent", "trimMargin"]),
        core_config::shared::ctor_own_line_kinds(&["AnnotationEntry"]),
        core_config::shared::comment_trim_lines(true),
        core_config::shared::comment_reindent_lines(true),
        core_config::shared::format_rules(&["curly", "redundant-return-type", "comment-space"]),
        core_config::shared::redundant_return_types(&["Unit"]),
        core_config::shared::curly_style("chain"),
        core_config::shared::settle_keywords(&["if", "else", "for", "while"]),
        core_config::shared::signature_wrap_annotated(true),
        core_config::shared::annotation_source_kinds(&["ValueParameter"]),
        core_config::shared::member_chain_style("wrapped"),
        core_config::shared::member_chain_min_calls(2),
        core_config::shared::member_chain_force_links(4),
        core_config::shared::member_chain_indent(1),
        ])
        .with_presets(vec![
        core_config::Preset::new("kotlin/ktlint", include_str!("../presets/ktlint.yaml")),
        ])
    })
}
