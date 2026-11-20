// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const TRAILING_COMMAS: core_config::OptId = core_config::opt::TRAILING_COMMAS;
#[allow(dead_code)]
pub const FUNCTION_SIGNATURE_WRAP_MIN_PARAMETERS: core_config::OptId = core_config::opt::FUNCTION_SIGNATURE_WRAP_MIN_PARAMETERS;
#[allow(dead_code)]
pub const CLASS_SIGNATURE_WRAP_MIN_PARAMETERS: core_config::OptId = core_config::opt::CLASS_SIGNATURE_WRAP_MIN_PARAMETERS;
#[allow(dead_code)]
pub const CLASS_SIGNATURE_WRAP_MIN_SUPERTYPES: core_config::OptId = core_config::opt::CLASS_SIGNATURE_WRAP_MIN_SUPERTYPES;
#[allow(dead_code)]
pub const ASSIGNMENT_LAYOUT: core_config::OptId = core_config::opt::ASSIGNMENT_LAYOUT;
#[allow(dead_code)]
pub const ASSIGNMENT_CONDITIONAL: core_config::OptId = core_config::opt::ASSIGNMENT_CONDITIONAL;
#[allow(dead_code)]
pub const ASSIGNMENT_INDENT: core_config::OptId = core_config::opt::ASSIGNMENT_INDENT;
#[allow(dead_code)]
pub const ASSIGNMENT_LEFT_MIN_PARAMETERS: core_config::OptId = core_config::opt::ASSIGNMENT_LEFT_MIN_PARAMETERS;
#[allow(dead_code)]
pub const ASSIGNMENT_LEFT_UNLESS_KINDS: core_config::OptId = core_config::opt::ASSIGNMENT_LEFT_UNLESS_KINDS;
#[allow(dead_code)]
pub const ASSIGNMENT_KEEP_BREAK_KINDS: core_config::OptId = core_config::opt::ASSIGNMENT_KEEP_BREAK_KINDS;
#[allow(dead_code)]
pub const ANNOTATION_INLINE: core_config::OptId = core_config::opt::ANNOTATION_INLINE;
#[allow(dead_code)]
pub const SEMICOLON_KEEP_PARENTS: core_config::OptId = core_config::opt::SEMICOLON_KEEP_PARENTS;
#[allow(dead_code)]
pub const TRAILING_COMMA_BEFORE: core_config::OptId = core_config::opt::TRAILING_COMMA_BEFORE;
#[allow(dead_code)]
pub const TRAILING_COMMA_LAST_ENTRY: core_config::OptId = core_config::opt::TRAILING_COMMA_LAST_ENTRY;
#[allow(dead_code)]
pub const SINGLE_LINE_BODY_KINDS: core_config::OptId = core_config::opt::SINGLE_LINE_BODY_KINDS;
#[allow(dead_code)]
pub const RAW_STRING_CALLS: core_config::OptId = core_config::opt::RAW_STRING_CALLS;
#[allow(dead_code)]
pub const CTOR_OWN_LINE_KINDS: core_config::OptId = core_config::opt::CTOR_OWN_LINE_KINDS;
#[allow(dead_code)]
pub const WRAPPED_BODY_INDENT_WORDS: core_config::OptId = core_config::opt::WRAPPED_BODY_INDENT_WORDS;
#[allow(dead_code)]
pub const COMMENT_TRIM_LINES: core_config::OptId = core_config::opt::COMMENT_TRIM_LINES;
#[allow(dead_code)]
pub const COMMENT_REINDENT_LINES: core_config::OptId = core_config::opt::COMMENT_REINDENT_LINES;
#[allow(dead_code)]
pub const FORMAT_RULES: core_config::OptId = core_config::opt::FORMAT_RULES;
#[allow(dead_code)]
pub const REDUNDANT_RETURN_TYPES: core_config::OptId = core_config::opt::REDUNDANT_RETURN_TYPES;
#[allow(dead_code)]
pub const CURLY_STYLE: core_config::OptId = core_config::opt::CURLY_STYLE;
#[allow(dead_code)]
pub const SETTLE_KEYWORDS: core_config::OptId = core_config::opt::SETTLE_KEYWORDS;
#[allow(dead_code)]
pub const SIGNATURE_WRAP_ANNOTATED: core_config::OptId = core_config::opt::SIGNATURE_WRAP_ANNOTATED;
#[allow(dead_code)]
pub const ANNOTATION_SOURCE_KINDS: core_config::OptId = core_config::opt::ANNOTATION_SOURCE_KINDS;
#[allow(dead_code)]
pub const MEMBER_CHAIN_STYLE: core_config::OptId = core_config::opt::MEMBER_CHAIN_STYLE;
#[allow(dead_code)]
pub const MEMBER_CHAIN_MIN_CALLS: core_config::OptId = core_config::opt::MEMBER_CHAIN_MIN_CALLS;
#[allow(dead_code)]
pub const MEMBER_CHAIN_FORCE_LINKS: core_config::OptId = core_config::opt::MEMBER_CHAIN_FORCE_LINKS;
#[allow(dead_code)]
pub const MEMBER_CHAIN_INDENT: core_config::OptId = core_config::opt::MEMBER_CHAIN_INDENT;
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
        core_config::OptionDef::choice(core_config::opt::TRAILING_COMMAS, &["never", "multiline"], "multiline", "Trailing comma after the last argument or parameter of a call or declaration that is laid out on several lines: `multiline` adds one, `never` removes it."),
        core_config::OptionDef::int(core_config::opt::FUNCTION_SIGNATURE_WRAP_MIN_PARAMETERS, 2, (1, 1000), "A function or constructor with at least this many parameters puts each on a line of its own, even when the signature fits."),
        core_config::OptionDef::int(core_config::opt::CLASS_SIGNATURE_WRAP_MIN_PARAMETERS, 1, (1, 1000), "A class with at least this many primary-constructor parameters puts each on a line of its own, even when the signature fits."),
        core_config::OptionDef::int(core_config::opt::CLASS_SIGNATURE_WRAP_MIN_SUPERTYPES, 2, (1, 1000), "A class or object with at least this many supertypes puts each on a line of its own."),
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
        core_config::shared::wrapped_body_indent_words(&["constructor"]),
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
