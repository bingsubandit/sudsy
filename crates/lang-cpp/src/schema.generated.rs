// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const COMMENT_KEEP_UNICODE_BLANKS: core_config::OptId = core_config::opt::COMMENT_KEEP_UNICODE_BLANKS;
#[allow(dead_code)]
pub const WRAP_LINES: core_config::OptId = core_config::opt::WRAP_LINES;
#[allow(dead_code)]
pub const CONTINUATION_INDENT: core_config::OptId = core_config::opt::CONTINUATION_INDENT;
#[allow(dead_code)]
pub const ACCESS_MODIFIER_OUTDENT: core_config::OptId = core_config::opt::ACCESS_MODIFIER_OUTDENT;
#[allow(dead_code)]
pub const INDENT_CASE_LABELS: core_config::OptId = core_config::opt::INDENT_CASE_LABELS;
#[allow(dead_code)]
pub const INDENT_GOTO_LABELS: core_config::OptId = core_config::opt::INDENT_GOTO_LABELS;
#[allow(dead_code)]
pub const KEEP_LEADING_BLANK_LINES: core_config::OptId = core_config::opt::KEEP_LEADING_BLANK_LINES;
#[allow(dead_code)]
pub const INDENT_PP_DIRECTIVES: core_config::OptId = core_config::opt::INDENT_PP_DIRECTIVES;
#[allow(dead_code)]
pub const NAMESPACE_INDENTATION: core_config::OptId = core_config::opt::NAMESPACE_INDENTATION;
#[allow(dead_code)]
pub const POINTER_ALIGNMENT: core_config::OptId = core_config::opt::POINTER_ALIGNMENT;
#[allow(dead_code)]
pub const POINTER_QUALIFIER_WORDS: core_config::OptId = core_config::opt::POINTER_QUALIFIER_WORDS;
#[allow(dead_code)]
pub const DERIVE_POINTER_ALIGNMENT: core_config::OptId = core_config::opt::DERIVE_POINTER_ALIGNMENT;
#[allow(dead_code)]
pub const SPACE_AFTER_CAST: core_config::OptId = core_config::opt::SPACE_AFTER_CAST;
#[allow(dead_code)]
pub const EMPTY_LINE_BEFORE_ACCESS_MODIFIER: core_config::OptId = core_config::opt::EMPTY_LINE_BEFORE_ACCESS_MODIFIER;
#[allow(dead_code)]
pub const KEEP_BLANK_LINES_AT_END_OF_BLOCKS: core_config::OptId = core_config::opt::KEEP_BLANK_LINES_AT_END_OF_BLOCKS;
#[allow(dead_code)]
pub const JOIN_CLOSING_ANGLES: core_config::OptId = core_config::opt::JOIN_CLOSING_ANGLES;
#[allow(dead_code)]
pub const EMPTY_BODY_OWN_LINE: core_config::OptId = core_config::opt::EMPTY_BODY_OWN_LINE;
#[allow(dead_code)]
pub const COLLAPSE_EMPTY_BLOCKS: core_config::OptId = core_config::opt::COLLAPSE_EMPTY_BLOCKS;
#[allow(dead_code)]
pub const BRACED_LIST_PADDING: core_config::OptId = core_config::opt::BRACED_LIST_PADDING;
#[allow(dead_code)]
pub const BRACE_WRAPPING: core_config::OptId = core_config::opt::BRACE_WRAPPING;
#[allow(dead_code)]
pub const SPLIT_EMPTY_BRACES: core_config::OptId = core_config::opt::SPLIT_EMPTY_BRACES;
#[allow(dead_code)]
pub const FOR_EACH_MACROS: core_config::OptId = core_config::opt::FOR_EACH_MACROS;
#[allow(dead_code)]
pub const ALIGN_WITH_TABS: core_config::OptId = core_config::opt::ALIGN_WITH_TABS;
#[allow(dead_code)]
pub const INDENT_BRACES: core_config::OptId = core_config::opt::INDENT_BRACES;
#[allow(dead_code)]
pub const SHORT_FUNCTIONS: core_config::OptId = core_config::opt::SHORT_FUNCTIONS;
#[allow(dead_code)]
pub const SHORT_IFS: core_config::OptId = core_config::opt::SHORT_IFS;
#[allow(dead_code)]
pub const SHORT_LOOPS: core_config::OptId = core_config::opt::SHORT_LOOPS;
#[allow(dead_code)]
pub const SHORT_LAMBDAS: core_config::OptId = core_config::opt::SHORT_LAMBDAS;
#[allow(dead_code)]
pub const SHORT_CASE_LABELS: core_config::OptId = core_config::opt::SHORT_CASE_LABELS;
#[allow(dead_code)]
pub const BIN_PACK_ARGUMENTS: core_config::OptId = core_config::opt::BIN_PACK_ARGUMENTS;
#[allow(dead_code)]
pub const BIN_PACK_PARAMETERS: core_config::OptId = core_config::opt::BIN_PACK_PARAMETERS;
#[allow(dead_code)]
pub const ALIGN_AFTER_OPEN_BRACKET: core_config::OptId = core_config::opt::ALIGN_AFTER_OPEN_BRACKET;
#[allow(dead_code)]
pub const KEEP_SPLIT_DECLARATIONS: core_config::OptId = core_config::opt::KEEP_SPLIT_DECLARATIONS;
#[allow(dead_code)]
pub const COLUMN_LIST_MIN_COMMAS: core_config::OptId = core_config::opt::COLUMN_LIST_MIN_COMMAS;
#[allow(dead_code)]
pub const BREAK_BEFORE_BINARY_OPERATORS: core_config::OptId = core_config::opt::BREAK_BEFORE_BINARY_OPERATORS;
#[allow(dead_code)]
pub const BREAK_CONSTRUCTOR_INITIALIZERS: core_config::OptId = core_config::opt::BREAK_CONSTRUCTOR_INITIALIZERS;
#[allow(dead_code)]
pub const CONSTRUCTOR_INITIALIZER_INDENT: core_config::OptId = core_config::opt::CONSTRUCTOR_INITIALIZER_INDENT;
#[allow(dead_code)]
pub const BREAK_AFTER_RETURN_TYPE: core_config::OptId = core_config::opt::BREAK_AFTER_RETURN_TYPE;
#[allow(dead_code)]
pub const SHORT_ENUMS: core_config::OptId = core_config::opt::SHORT_ENUMS;
#[allow(dead_code)]
pub const PENALTY_RETURN_TYPE: core_config::OptId = core_config::opt::PENALTY_RETURN_TYPE;
#[allow(dead_code)]
pub const SPACE_BEFORE_PARENS: core_config::OptId = core_config::opt::SPACE_BEFORE_PARENS;
#[allow(dead_code)]
pub const SPACES_BEFORE_TRAILING_COMMENTS: core_config::OptId = core_config::opt::SPACES_BEFORE_TRAILING_COMMENTS;
#[allow(dead_code)]
pub const ALIGN_TRAILING_COMMENTS: core_config::OptId = core_config::opt::ALIGN_TRAILING_COMMENTS;
#[allow(dead_code)]
pub const BREAK_TEMPLATE_DECLARATIONS: core_config::OptId = core_config::opt::BREAK_TEMPLATE_DECLARATIONS;
#[allow(dead_code)]
pub const ALIGN_ESCAPED_NEWLINES: core_config::OptId = core_config::opt::ALIGN_ESCAPED_NEWLINES;
pub const OPTION_KINDS: core_format::rules::OptionKinds = &[
    ("access_modifier_outdent", "outdent", "", &["AccessSpecifier"]),
    ("indent_case_labels", "outdent_unless", "", &["CaseClause"]),
    ("indent_goto_labels", "flush_unless", "", &["LabelStatement"]),
    ("indent_pp_directives", "tight", "", &["Directive"]),
    ("namespace_indentation", "flat", "", &["NamespaceDeclaration"]),
    ("pointer_alignment", "binary_cast", "", &["Rpar"]),
    ("pointer_alignment", "binary_clear", "", &["Assignment", "ReturnStatement", "Binary", "Ternary", "Index", "Unary", "Arguments", "Parenthesized", "Condition"]),
    ("pointer_alignment", "binary_node", "", &["Binary"]),
    ("pointer_alignment", "binary_operand", "", &["MemberAccess"]),
    ("pointer_alignment", "binary_spaced", "", &["Amp"]),
    ("pointer_alignment", "pointer", "", &["Star", "Amp", "AmpAmp"]),
    ("pointer_alignment", "pointer_bare", "", &["TemplateArguments"]),
    ("pointer_alignment", "pointer_end", "", &["Greater", "GreaterGreater", "Comma", "Rpar", "Ellipsis", "Lsqb"]),
    ("pointer_alignment", "pointer_in", "", &["Parameter", "FunctionDeclaration", "SimpleDeclaration", "TemplateArguments", "Line", "Directive", "DeclaratorGroup", "Condition"]),
    ("pointer_alignment", "pointer_in_loose", "", &["ForStatement", "CatchClause", "Parameter"]),
    ("pointer_alignment", "pointer_in_under", "", &["Cast"]),
    ("pointer_alignment", "pointer_name", "", &["Ident", "Keyword", "Lsqb"]),
    ("pointer_alignment", "pointer_not_name", "", &["Captures"]),
    ("pointer_alignment", "pointer_ref", "", &["Amp", "AmpAmp"]),
    ("pointer_alignment", "pointer_shaped", "", &["ParenGroup"]),
    ("pointer_alignment", "pointer_type", "", &["Ident", "Keyword", "Greater", "GreaterGreater"]),
    ("pointer_alignment", "tight_after", "", &["Ellipsis"]),
    ("pointer_alignment", "tight_behind", "", &["Star", "Amp", "AmpAmp"]),
    ("space_after_cast", "cast_close", "", &["Rpar"]),
    ("space_after_cast", "cast_group", "", &["ParenGroup", "Parenthesized"]),
    ("space_after_cast", "cast_keep", "", &["AmpAmp", "Plus"]),
    ("space_after_cast", "cast_node", "", &["Cast"]),
    ("space_after_cast", "cast_tight", "", &["Star", "Amp", "Minus"]),
    ("empty_line_before_access_modifier", "blank_before", "", &["AccessSpecifier"]),
    ("keep_blank_lines_at_end_of_blocks", "keep_end", "", &["Block"]),
    ("keep_blank_lines_at_end_of_blocks", "keep_end_before", "", &["Keyword", "CatchClause"]),
    ("keep_blank_lines_at_end_of_blocks", "keep_end_under", "", &["NamespaceDeclaration", "EnumSpecifier"]),
    ("join_closing_angles", "close_join", "", &["Greater"]),
    ("join_closing_angles", "close_join_in", "", &["TemplateArguments", "TemplateParameters"]),
    ("empty_body_own_line", "own_empty", "", &["IfStatement", "ForStatement", "WhileStatement"]),
    ("collapse_empty_blocks", "split_in", "", &["IfStatement", "ForStatement", "WhileStatement", "DoStatement", "TryStatement", "CatchClause", "Block"]),
    ("braced_list_padding", "pad_close", "", &["Rbrace"]),
    ("braced_list_padding", "pad_except", "", &["Semi"]),
    ("braced_list_padding", "pad_list", "", &["InitList"]),
    ("braced_list_padding", "pad_open", "", &["Lbrace"]),
    ("brace_wrapping", "attach", "class", &["ClassSpecifier"]),
    ("brace_wrapping", "attach", "control", &["IfStatement", "ForStatement", "WhileStatement", "SwitchStatement", "DoStatement", "TryStatement"]),
    ("brace_wrapping", "attach", "enum", &["EnumSpecifier"]),
    ("brace_wrapping", "attach", "function", &["FunctionDeclaration"]),
    ("brace_wrapping", "attach", "namespace", &["NamespaceDeclaration"]),
    ("brace_wrapping", "attach", "struct", &["ClassSpecifier"]),
    ("brace_wrapping", "attach", "union", &["ClassSpecifier"]),
    ("brace_wrapping", "attach_after", "", &["Rpar", "Rsqb", "Ident", "Keyword", "Greater"]),
    ("brace_wrapping", "attach_apart", "", &["Directive"]),
    ("brace_wrapping", "attach_call", "function", &["Call"]),
    ("brace_wrapping", "attach_in", "", &["Block"]),
    ("for_each_macros", "loop_call", "control", &["Call"]),
    ("short_functions", "fit", "", &["FunctionDeclaration", "Line"]),
    ("short_ifs", "head", "", &["Condition"]),
    ("short_ifs", "join", "", &["IfStatement"]),
    ("short_loops", "head", "", &["Condition"]),
    ("short_loops", "join", "", &["ForStatement", "WhileStatement"]),
    ("short_lambdas", "fit", "", &["Lambda"]),
    ("align_after_open_bracket", "comment_continue", "", &["FunctionDeclaration", "SimpleDeclaration"]),
    ("align_after_open_bracket", "hug_brace_after", "", &["Ident", "Greater"]),
    ("align_after_open_bracket", "hug_brace_hang", "", &["Equal"]),
    ("align_after_open_bracket", "hug_comment_down", "", &["InitList"]),
    ("align_after_open_bracket", "hug_unless", "", &["Lambda", "Block", "InitList"]),
    ("align_after_open_bracket", "name_break", "", &["FunctionDeclaration"]),
    ("align_after_open_bracket", "name_break_after", "", &["Keyword", "Star", "Greater"]),
    ("keep_split_declarations", "verbatim_split", "", &["ParenGroup"]),
    ("short_enums", "fit", "", &["EnumSpecifier"]),
    ("break_template_declarations", "break_after", "", &["TemplateDeclaration"]),
    ("break_template_declarations", "head", "", &["TemplateParameters"]),
    ("align_escaped_newlines", "relay", "", &["Directive"]),
    ("align_escaped_newlines", "relay_head", "", &["ParenGroup"]),
];
#[allow(dead_code)]
pub fn wrap_lines(o: &core_config::ResolvedOptions) -> bool {
    o.flag(WRAP_LINES, true)
}
#[allow(dead_code)]
pub fn continuation_indent(o: &core_config::ResolvedOptions) -> usize {
    o.uint(CONTINUATION_INDENT, 4)
}
#[allow(dead_code)]
pub fn access_modifier_outdent(o: &core_config::ResolvedOptions) -> usize {
    o.uint(ACCESS_MODIFIER_OUTDENT, 2)
}
#[allow(dead_code)]
pub fn indent_case_labels(o: &core_config::ResolvedOptions) -> bool {
    o.flag(INDENT_CASE_LABELS, false)
}
#[allow(dead_code)]
pub fn indent_goto_labels(o: &core_config::ResolvedOptions) -> bool {
    o.flag(INDENT_GOTO_LABELS, true)
}
#[allow(dead_code)]
pub fn keep_leading_blank_lines(o: &core_config::ResolvedOptions) -> bool {
    o.flag(KEEP_LEADING_BLANK_LINES, true)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentPpDirectives { None, Keep }
#[allow(dead_code)]
pub fn indent_pp_directives(o: &core_config::ResolvedOptions) -> IndentPpDirectives {
    match o.text(INDENT_PP_DIRECTIVES) {
        "keep" => IndentPpDirectives::Keep,
        _ => IndentPpDirectives::None,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamespaceIndentation { None, Inner, All }
#[allow(dead_code)]
pub fn namespace_indentation(o: &core_config::ResolvedOptions) -> NamespaceIndentation {
    match o.text(NAMESPACE_INDENTATION) {
        "inner" => NamespaceIndentation::Inner,
        "all" => NamespaceIndentation::All,
        _ => NamespaceIndentation::None,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerAlignment { Right, Left, Middle }
#[allow(dead_code)]
pub fn pointer_alignment(o: &core_config::ResolvedOptions) -> PointerAlignment {
    match o.text(POINTER_ALIGNMENT) {
        "left" => PointerAlignment::Left,
        "middle" => PointerAlignment::Middle,
        _ => PointerAlignment::Right,
    }
}
#[allow(dead_code)]
pub fn derive_pointer_alignment(o: &core_config::ResolvedOptions) -> bool {
    o.flag(DERIVE_POINTER_ALIGNMENT, false)
}
#[allow(dead_code)]
pub fn space_after_cast(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_AFTER_CAST, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyLineBeforeAccessModifier { Never, Leave, LogicalBlock, Always }
#[allow(dead_code)]
pub fn empty_line_before_access_modifier(o: &core_config::ResolvedOptions) -> EmptyLineBeforeAccessModifier {
    match o.text(EMPTY_LINE_BEFORE_ACCESS_MODIFIER) {
        "never" => EmptyLineBeforeAccessModifier::Never,
        "leave" => EmptyLineBeforeAccessModifier::Leave,
        "always" => EmptyLineBeforeAccessModifier::Always,
        _ => EmptyLineBeforeAccessModifier::LogicalBlock,
    }
}
#[allow(dead_code)]
pub fn keep_blank_lines_at_end_of_blocks(o: &core_config::ResolvedOptions) -> bool {
    o.flag(KEEP_BLANK_LINES_AT_END_OF_BLOCKS, false)
}
#[allow(dead_code)]
pub fn join_closing_angles(o: &core_config::ResolvedOptions) -> bool {
    o.flag(JOIN_CLOSING_ANGLES, false)
}
#[allow(dead_code)]
pub fn empty_body_own_line(o: &core_config::ResolvedOptions) -> bool {
    o.flag(EMPTY_BODY_OWN_LINE, false)
}
#[allow(dead_code)]
pub fn collapse_empty_blocks(o: &core_config::ResolvedOptions) -> bool {
    o.flag(COLLAPSE_EMPTY_BLOCKS, false)
}
#[allow(dead_code)]
pub fn braced_list_padding(o: &core_config::ResolvedOptions) -> bool {
    o.flag(BRACED_LIST_PADDING, false)
}
#[allow(dead_code)]
pub fn align_with_tabs(o: &core_config::ResolvedOptions) -> bool {
    o.flag(ALIGN_WITH_TABS, false)
}
#[allow(dead_code)]
pub fn indent_braces(o: &core_config::ResolvedOptions) -> bool {
    o.flag(INDENT_BRACES, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortFunctions { None, Empty, Inline, All }
#[allow(dead_code)]
pub fn short_functions(o: &core_config::ResolvedOptions) -> ShortFunctions {
    match o.text(SHORT_FUNCTIONS) {
        "none" => ShortFunctions::None,
        "empty" => ShortFunctions::Empty,
        "inline" => ShortFunctions::Inline,
        _ => ShortFunctions::All,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortIfs { Never, WithoutElse, All }
#[allow(dead_code)]
pub fn short_ifs(o: &core_config::ResolvedOptions) -> ShortIfs {
    match o.text(SHORT_IFS) {
        "without_else" => ShortIfs::WithoutElse,
        "all" => ShortIfs::All,
        _ => ShortIfs::Never,
    }
}
#[allow(dead_code)]
pub fn short_loops(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SHORT_LOOPS, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortLambdas { None, Empty, Inline, All }
#[allow(dead_code)]
pub fn short_lambdas(o: &core_config::ResolvedOptions) -> ShortLambdas {
    match o.text(SHORT_LAMBDAS) {
        "none" => ShortLambdas::None,
        "empty" => ShortLambdas::Empty,
        "inline" => ShortLambdas::Inline,
        _ => ShortLambdas::All,
    }
}
#[allow(dead_code)]
pub fn short_case_labels(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SHORT_CASE_LABELS, false)
}
#[allow(dead_code)]
pub fn bin_pack_arguments(o: &core_config::ResolvedOptions) -> bool {
    o.flag(BIN_PACK_ARGUMENTS, true)
}
#[allow(dead_code)]
pub fn bin_pack_parameters(o: &core_config::ResolvedOptions) -> bool {
    o.flag(BIN_PACK_PARAMETERS, true)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignAfterOpenBracket { Align, DontAlign, AlwaysBreak }
#[allow(dead_code)]
pub fn align_after_open_bracket(o: &core_config::ResolvedOptions) -> AlignAfterOpenBracket {
    match o.text(ALIGN_AFTER_OPEN_BRACKET) {
        "dont_align" => AlignAfterOpenBracket::DontAlign,
        "always_break" => AlignAfterOpenBracket::AlwaysBreak,
        _ => AlignAfterOpenBracket::Align,
    }
}
#[allow(dead_code)]
pub fn keep_split_declarations(o: &core_config::ResolvedOptions) -> bool {
    o.flag(KEEP_SPLIT_DECLARATIONS, true)
}
#[allow(dead_code)]
pub fn column_list_min_commas(o: &core_config::ResolvedOptions) -> usize {
    o.uint(COLUMN_LIST_MIN_COMMAS, 5)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakBeforeBinaryOperators { None, NonAssignment, All }
#[allow(dead_code)]
pub fn break_before_binary_operators(o: &core_config::ResolvedOptions) -> BreakBeforeBinaryOperators {
    match o.text(BREAK_BEFORE_BINARY_OPERATORS) {
        "non_assignment" => BreakBeforeBinaryOperators::NonAssignment,
        "all" => BreakBeforeBinaryOperators::All,
        _ => BreakBeforeBinaryOperators::None,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakConstructorInitializers { BeforeColon, BeforeComma }
#[allow(dead_code)]
pub fn break_constructor_initializers(o: &core_config::ResolvedOptions) -> BreakConstructorInitializers {
    match o.text(BREAK_CONSTRUCTOR_INITIALIZERS) {
        "before_comma" => BreakConstructorInitializers::BeforeComma,
        _ => BreakConstructorInitializers::BeforeColon,
    }
}
#[allow(dead_code)]
pub fn constructor_initializer_indent(o: &core_config::ResolvedOptions) -> usize {
    o.uint(CONSTRUCTOR_INITIALIZER_INDENT, 4)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakAfterReturnType { None, TopLevel, All, TopLevelDefinitions, AllDefinitions }
#[allow(dead_code)]
pub fn break_after_return_type(o: &core_config::ResolvedOptions) -> BreakAfterReturnType {
    match o.text(BREAK_AFTER_RETURN_TYPE) {
        "top_level" => BreakAfterReturnType::TopLevel,
        "all" => BreakAfterReturnType::All,
        "top_level_definitions" => BreakAfterReturnType::TopLevelDefinitions,
        "all_definitions" => BreakAfterReturnType::AllDefinitions,
        _ => BreakAfterReturnType::None,
    }
}
#[allow(dead_code)]
pub fn short_enums(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SHORT_ENUMS, true)
}
#[allow(dead_code)]
pub fn penalty_return_type(o: &core_config::ResolvedOptions) -> usize {
    o.uint(PENALTY_RETURN_TYPE, 60)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceBeforeParens { Never, ControlStatements, Always }
#[allow(dead_code)]
pub fn space_before_parens(o: &core_config::ResolvedOptions) -> SpaceBeforeParens {
    match o.text(SPACE_BEFORE_PARENS) {
        "never" => SpaceBeforeParens::Never,
        "always" => SpaceBeforeParens::Always,
        _ => SpaceBeforeParens::ControlStatements,
    }
}
#[allow(dead_code)]
pub fn spaces_before_trailing_comments(o: &core_config::ResolvedOptions) -> usize {
    o.uint(SPACES_BEFORE_TRAILING_COMMENTS, 1)
}
#[allow(dead_code)]
pub fn align_trailing_comments(o: &core_config::ResolvedOptions) -> bool {
    o.flag(ALIGN_TRAILING_COMMENTS, true)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakTemplateDeclarations { Yes, Multiline, No }
#[allow(dead_code)]
pub fn break_template_declarations(o: &core_config::ResolvedOptions) -> BreakTemplateDeclarations {
    match o.text(BREAK_TEMPLATE_DECLARATIONS) {
        "yes" => BreakTemplateDeclarations::Yes,
        "no" => BreakTemplateDeclarations::No,
        _ => BreakTemplateDeclarations::Multiline,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignEscapedNewlines { Right, Left, DontAlign }
#[allow(dead_code)]
pub fn align_escaped_newlines(o: &core_config::ResolvedOptions) -> AlignEscapedNewlines {
    match o.text(ALIGN_ESCAPED_NEWLINES) {
        "left" => AlignEscapedNewlines::Left,
        "dont_align" => AlignEscapedNewlines::DontAlign,
        _ => AlignEscapedNewlines::Right,
    }
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::line_width(80),
        core_config::shared::indent_size(2),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::shared::max_blank_lines(1, "Largest number of blank lines kept between statements and comments."),
        core_config::shared::comment_keep_unicode_blanks(true),
        core_config::OptionDef::boolean(core_config::opt::WRAP_LINES, true, "Break lines that are longer than `line_width`. When false, every line keeps the line breaks of the source (only indentation and spacing change)."),
        core_config::OptionDef::int(core_config::opt::CONTINUATION_INDENT, 4, (0, 16), "Columns a wrapped line is indented more than the line it continues, where it is not aligned to a bracket."),
        core_config::OptionDef::int(core_config::opt::ACCESS_MODIFIER_OUTDENT, 2, (0, 16), "Columns `public:`, `private:` and `protected:` are outdented from the members of their class."),
        core_config::OptionDef::boolean(core_config::opt::INDENT_CASE_LABELS, false, "Indent `case` labels one level inside their `switch`."),
        core_config::OptionDef::boolean(core_config::opt::INDENT_GOTO_LABELS, true, "A goto label stands one indent left of the statements of its block; when false it starts at column 0."),
        core_config::OptionDef::boolean(core_config::opt::KEEP_LEADING_BLANK_LINES, true, "Blank lines at the start of the file stay."),
        core_config::OptionDef::choice(core_config::opt::INDENT_PP_DIRECTIVES, &["none", "keep"], "none", "A preprocessor directive starts with `#` directly followed by its keyword (`none`) or keeps the blanks written after the `#` (`keep`)."),
        core_config::OptionDef::choice(core_config::opt::NAMESPACE_INDENTATION, &["none", "inner", "all"], "none", "Indent the contents of namespaces: `none`, only namespaces nested in another (`inner`), or `all`."),
        core_config::OptionDef::choice(core_config::opt::POINTER_ALIGNMENT, &["right", "left", "middle"], "right", "Where the `*` and `&` of a pointer or reference declarator go: next to the name (`right`), next to the type (`left`) or between them (`middle`)."),
        core_config::OptionDef::words(core_config::opt::POINTER_QUALIFIER_WORDS, &["noexcept", "override", "final", "requires"], "Words that are no name when they follow a `*`, `&` or `&&` of a declarator (`T &noexcept`): a blank stands before them."),
        core_config::OptionDef::boolean(core_config::opt::DERIVE_POINTER_ALIGNMENT, false, "Take `pointer_alignment` from the file: the side that most of its declarators of pointers and references use (a `*` or `&` with a blank before and none after against none before and a blank after); `pointer_alignment` stays where both sides are equal."),
        core_config::OptionDef::boolean(core_config::opt::SPACE_AFTER_CAST, false, "A blank after the `)` of a C cast (`(int) x` against `(int)x`)."),
        core_config::OptionDef::choice(core_config::opt::EMPTY_LINE_BEFORE_ACCESS_MODIFIER, &["never", "leave", "logical_block", "always"], "logical_block", "A blank line before `public:`, `private:` and `protected:` (not the first item of a class): removed (`never`), as written (`leave`), added unless a comment stands in front (`logical_block`) or always added."),
        core_config::OptionDef::boolean(core_config::opt::KEEP_BLANK_LINES_AT_END_OF_BLOCKS, false, "A blank line before the closing brace of a block stays."),
        core_config::OptionDef::boolean(core_config::opt::JOIN_CLOSING_ANGLES, false, "Two closing template brackets are written together (`A<B<C>>` against `A<B<C> >`), as C++11 reads them. Only a `>` that closes a template bracket is joined, a comparison or a shift stays apart."),
        core_config::OptionDef::boolean(core_config::opt::EMPTY_BODY_OWN_LINE, false, "The `;` that is the whole body of an `if`, `for` or `while` goes on its own line below the head, indented (`while (x)\\n  ;`)."),
        core_config::OptionDef::boolean(core_config::opt::COLLAPSE_EMPTY_BLOCKS, false, "An empty body is `{}` when its line fits, except the block of `if`, `for`, `while`, `do`, `try` and `catch` and a block inside a block, which has two lines (`if (x) {\\n}`)."),
        core_config::OptionDef::boolean(core_config::opt::BRACED_LIST_PADDING, false, "A blank inside the braces of an initializer list (`{ 1, 2 }` against `{1, 2}`); an empty list is `{}`."),
        core_config::OptionDef::words(core_config::opt::BRACE_WRAPPING, &[], "Constructs whose opening brace goes on its own line: `class`, `struct`, `union`, `enum`, `function`, `namespace`, `extern`, `control` (if, for, while, switch, do, try), `case`, `else` (the `else` after a block starts a line), `catch` (the `catch` after a block starts a line), `while` (the `while` of a do loop starts a line). Empty: every brace stays on the line of its construct."),
        core_config::OptionDef::words(core_config::opt::SPLIT_EMPTY_BRACES, &[], "Words of `brace_wrapping` whose empty body keeps `{` and `}` on two lines when the `{` is on its own line (`void f()\\n{\\n}`), as `SplitEmptyFunction`, `SplitEmptyRecord` and `SplitEmptyNamespace`. Other empty bodies are `{}`."),
        core_config::OptionDef::words(core_config::opt::FOR_EACH_MACROS, &["foreach", "Q_FOREACH", "BOOST_FOREACH"], "Names of macros that are used as loops: `name(args) {` is a `control` brace. Any other `name(args) {` is a function definition for `brace_wrapping`."),
        core_config::OptionDef::boolean(core_config::opt::ALIGN_WITH_TABS, false, "With `indent_style: tab`, the alignment of continuation lines uses tabs for each whole indent width and spaces for the rest."),
        core_config::OptionDef::boolean(core_config::opt::INDENT_BRACES, false, "Indent a brace that is on its own line together with the block it opens."),
        core_config::OptionDef::choice(core_config::opt::SHORT_FUNCTIONS, &["none", "empty", "inline", "all"], "all", "Function bodies kept on the line of the signature when they fit: `none`, `empty` (`{}` only), `inline` (functions defined in a class, and empty ones) or `all`."),
        core_config::OptionDef::choice(core_config::opt::SHORT_IFS, &["never", "without_else", "all"], "never", "An `if` with a simple statement is kept on one line when it fits: `never`, without an `else`, or with or without."),
        core_config::OptionDef::boolean(core_config::opt::SHORT_LOOPS, false, "A `for`, `while` or range-for with a simple statement is kept on one line when it fits."),
        core_config::OptionDef::choice(core_config::opt::SHORT_LAMBDAS, &["none", "empty", "inline", "all"], "all", "Lambda bodies kept on the line of the lambda when they fit: `none`, `empty`, `inline` (a lambda that is an argument) or `all`."),
        core_config::OptionDef::boolean(core_config::opt::SHORT_CASE_LABELS, false, "A `case` label and its simple statement are kept on one line when they fit."),
        core_config::OptionDef::boolean(core_config::opt::BIN_PACK_ARGUMENTS, true, "Call arguments that do not fit on a line are filled; when false they go one per line (or all on one line)."),
        core_config::OptionDef::boolean(core_config::opt::BIN_PACK_PARAMETERS, true, "Function parameters that do not fit on a line are filled; when false they go one per line (or all on one line)."),
        core_config::OptionDef::choice(core_config::opt::ALIGN_AFTER_OPEN_BRACKET, &["align", "dont_align", "always_break"], "align", "Wrapped lines inside brackets line up with the first token after the bracket (`align`), continue with `continuation_indent` (`dont_align`), or always start on a new line after the bracket (`always_break`). Only a list with a comment at a line break is laid out by this option today: its first item goes on the line of the bracket when that line fits; any other list keeps the indentation of the source."),
        core_config::OptionDef::boolean(core_config::opt::KEEP_SPLIT_DECLARATIONS, true, "A declaration head that a preprocessor line splits behind a parenthesized group (`__declspec(dllexport)` `#endif` `int f(...)`) stays as written up to its `{`, as the reference tool leaves a line it cannot parse."),
        core_config::OptionDef::int(core_config::opt::COLUMN_LIST_MIN_COMMAS, 5, (0, 100), "A braced list with at least this many commas is laid out in columns, every cell padded to the widest of its column; 0 turns the layout off."),
        core_config::OptionDef::choice(core_config::opt::BREAK_BEFORE_BINARY_OPERATORS, &["none", "non_assignment", "all"], "none", "A wrapped binary expression breaks after the operator (`none`), before it except for assignments (`non_assignment`) or before every operator (`all`)."),
        core_config::OptionDef::choice(core_config::opt::BREAK_CONSTRUCTOR_INITIALIZERS, &["before_colon", "before_comma"], "before_colon", "Constructor initializer lists that do not fit start on a new line before the colon (`before_colon`, one initializer per line when needed), or put a comma before each initializer, one per line (`before_comma`)."),
        core_config::OptionDef::int(core_config::opt::CONSTRUCTOR_INITIALIZER_INDENT, 4, (0, 16), "Columns a constructor initializer list is indented more than its constructor."),
        core_config::OptionDef::choice(core_config::opt::BREAK_AFTER_RETURN_TYPE, &["none", "top_level", "all", "top_level_definitions", "all_definitions"], "none", "The function name goes on a new line after its return type: never, for declarations and definitions outside classes (`top_level`), for all, or for definitions only (`top_level_definitions`, `all_definitions`)."),
        core_config::OptionDef::boolean(core_config::opt::SHORT_ENUMS, true, "An enum whose body fits stays on one line."),
        core_config::OptionDef::int(core_config::opt::PENALTY_RETURN_TYPE, 60, (0, 1000), "What a return type on a line of its own costs the layout; the higher, the longer the parameter list may wrap before the return type moves."),
        core_config::OptionDef::choice(core_config::opt::SPACE_BEFORE_PARENS, &["never", "control_statements", "always"], "control_statements", "A blank before the parenthesis of `if`, `for`, `while`, `switch` and `catch` (`control_statements`), before every parenthesis (`always`) or never."),
        core_config::OptionDef::int(core_config::opt::SPACES_BEFORE_TRAILING_COMMENTS, 1, (0, 16), "Blanks between the code of a line and its `//` comment."),
        core_config::OptionDef::boolean(core_config::opt::ALIGN_TRAILING_COMMENTS, true, "Align the trailing comments of consecutive lines."),
        core_config::OptionDef::choice(core_config::opt::BREAK_TEMPLATE_DECLARATIONS, &["yes", "multiline", "no"], "multiline", "`template <...>` ends its line: always (`yes`), when the declaration is not on one line (`multiline`) or never."),
        core_config::OptionDef::choice(core_config::opt::ALIGN_ESCAPED_NEWLINES, &["right", "left", "dont_align"], "right", "The backslashes of a multi-line macro: aligned at `line_width` (`right`), one blank after the longest line (`left`) or one blank after each line."),
        ])
        .with_presets(vec![
        core_config::Preset::new("cpp/google", include_str!("../presets/google.yaml")),
        core_config::Preset::new("cpp/chromium", include_str!("../presets/chromium.yaml")),
        core_config::Preset::new("cpp/mozilla", include_str!("../presets/mozilla.yaml")),
        core_config::Preset::new("cpp/webkit", include_str!("../presets/webkit.yaml")),
        core_config::Preset::new("cpp/gnu", include_str!("../presets/gnu.yaml")),
        core_config::Preset::new("cpp/microsoft", include_str!("../presets/microsoft.yaml")),
        ])
    })
}
