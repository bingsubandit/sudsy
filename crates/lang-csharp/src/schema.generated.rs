// Generated from lang.yaml by `cargo xtask codegen`.
#[allow(dead_code)]
pub const TRAILING_COMMAS: &str = "trailing_commas";
#[allow(dead_code)]
pub const MEMBER_CHAIN_MIN_CALLS: &str = "member_chain_min_calls";
#[allow(dead_code)]
pub const MEMBER_CHAIN_KEEP_ACCESSES: &str = "member_chain_keep_accesses";
#[allow(dead_code)]
pub const MEMBER_CHAIN_BREAK_ON_BLOCK: &str = "member_chain_break_on_block";
#[allow(dead_code)]
pub const MEMBER_CHAIN_SHORT_HEAD: &str = "member_chain_short_head";
#[allow(dead_code)]
pub const MEMBER_CHAIN_INDENT: &str = "member_chain_indent";
#[allow(dead_code)]
pub const BINARY_OPERATOR_LINE: &str = "binary_operator_line";
#[allow(dead_code)]
pub const BINARY_INDENT: &str = "binary_indent";
#[allow(dead_code)]
pub const BINARY_HUG: &str = "binary_hug";
#[allow(dead_code)]
pub const INDENT_NESTED_BINARY: &str = "indent_nested_binary";
#[allow(dead_code)]
pub const ASSIGNMENT_LAYOUT: &str = "assignment_layout";
#[allow(dead_code)]
pub const ASSIGNMENT_CONDITIONAL: &str = "assignment_conditional";
#[allow(dead_code)]
pub const ASSIGNMENT_INDENT: &str = "assignment_indent";
#[allow(dead_code)]
pub const CONDITIONAL_INDENT: &str = "conditional_indent";
#[allow(dead_code)]
pub const CONDITIONAL_LAYOUT: &str = "conditional_layout";
#[allow(dead_code)]
pub const PREPROCESSOR_SYMBOLS: &str = "preprocessor_symbols";
#[allow(dead_code)]
pub const PREPROCESSOR_SYMBOL_SETS: &str = "preprocessor_symbol_sets";
#[allow(dead_code)]
pub const DERIVE_SYMBOL_SETS: &str = "derive_symbol_sets";
#[allow(dead_code)]
pub const BLANK_LINES_BETWEEN_MEMBERS: &str = "blank_lines_between_members";
#[allow(dead_code)]
pub const ACCESSOR_BLANK_LINES: &str = "accessor_blank_lines";
#[allow(dead_code)]
pub const COLLECTION_BLANK_LINES: &str = "collection_blank_lines";
#[allow(dead_code)]
pub const INLINE_ACCESSOR_BODIES: &str = "inline_accessor_bodies";
#[allow(dead_code)]
pub const PRESERVE_SINGLE_LINE_BLOCKS: &str = "preserve_single_line_blocks";
#[allow(dead_code)]
pub const PRESERVE_SINGLE_LINE_STATEMENTS: &str = "preserve_single_line_statements";
#[allow(dead_code)]
pub const INDENT_SWITCH_SECTION_BLOCK: &str = "indent_switch_section_block";
#[allow(dead_code)]
pub const SWITCH_ARM_LAYOUT: &str = "switch_arm_layout";
#[allow(dead_code)]
pub const WRAPPED_BRACE_LINE: &str = "wrapped_brace_line";
#[allow(dead_code)]
pub const INITIALIZER_COLLAPSE: &str = "initializer_collapse";
#[allow(dead_code)]
pub const PRESERVE_QUERY_BREAKS: &str = "preserve_query_breaks";
#[allow(dead_code)]
pub const INITIALIZER_EXPAND: &str = "initializer_expand";
#[allow(dead_code)]
pub const LAMBDA_ARGUMENT_HUG: &str = "lambda_argument_hug";
#[allow(dead_code)]
pub const CONSTRUCTOR_INITIALIZER_LINE: &str = "constructor_initializer_line";
#[allow(dead_code)]
pub const MEMBER_ATTRIBUTE_LINE: &str = "member_attribute_line";
#[allow(dead_code)]
pub const PARAMETER_ATTRIBUTE_LAYOUT: &str = "parameter_attribute_layout";
#[allow(dead_code)]
pub const BASE_LIST_LAYOUT: &str = "base_list_layout";
#[allow(dead_code)]
pub const UNBREAKABLE_SINGLE_ITEM_LISTS: &str = "unbreakable_single_item_lists";
#[allow(dead_code)]
pub const DO_WHILE_LINE: &str = "do_while_line";
#[allow(dead_code)]
pub const CLAUSE_KEYWORD_LINE: &str = "clause_keyword_line";
#[allow(dead_code)]
pub const ELSE_LINE: &str = "else_line";
#[allow(dead_code)]
pub const CATCH_LINE: &str = "catch_line";
#[allow(dead_code)]
pub const FINALLY_LINE: &str = "finally_line";
#[allow(dead_code)]
pub const LABEL_INDENT: &str = "label_indent";
#[allow(dead_code)]
pub const EMPTY_BRACES: &str = "empty_braces";
#[allow(dead_code)]
pub const CLOSING_PAREN_LINE: &str = "closing_paren_line";
#[allow(dead_code)]
pub const BLANK_LINES_AFTER_BLOCK: &str = "blank_lines_after_block";
#[allow(dead_code)]
pub const BLANK_LINE_BEFORE_COMMENT: &str = "blank_line_before_comment";
#[allow(dead_code)]
pub const BLANK_LINE_AFTER_COMMENT: &str = "blank_line_after_comment";
#[allow(dead_code)]
pub const BLANK_COMPACT_KINDS: &str = "blank_compact_kinds";
#[allow(dead_code)]
pub const BLANK_AFTER_BLOCK_KINDS: &str = "blank_after_block_kinds";
#[allow(dead_code)]
pub const BLANK_DIRECTIVES_SKIP: &str = "blank_directives_skip";
#[allow(dead_code)]
pub const BLANK_DIRECTIVES_ATTACH: &str = "blank_directives_attach";
#[allow(dead_code)]
pub const SPACE_AFTER_DELEGATE: &str = "space_after_delegate";
#[allow(dead_code)]
pub const SPACE_BETWEEN_EMPTY_FOR_CLAUSES: &str = "space_between_empty_for_clauses";
#[allow(dead_code)]
pub const SPACE_AFTER_EMPTY_LAST_FOR_CLAUSE: &str = "space_after_empty_last_for_clause";
#[allow(dead_code)]
pub const SORT_USINGS: &str = "sort_usings";
#[allow(dead_code)]
pub const SORT_USINGS_SYSTEM_FIRST: &str = "sort_usings_system_first";
#[allow(dead_code)]
pub const SEPARATE_USING_GROUPS: &str = "separate_using_groups";
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingCommas { Preserve, Never, Multiline }
#[allow(dead_code)]
pub fn trailing_commas(o: &core_config::ResolvedOptions) -> TrailingCommas {
    match o.text(TRAILING_COMMAS) {
        "never" => TrailingCommas::Never,
        "multiline" => TrailingCommas::Multiline,
        _ => TrailingCommas::Preserve,
    }
}
#[allow(dead_code)]
pub fn derive_symbol_sets(o: &core_config::ResolvedOptions) -> bool {
    o.flag(DERIVE_SYMBOL_SETS, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlankLinesBetweenMembers { Preserve, Separate, Always }
#[allow(dead_code)]
pub fn blank_lines_between_members(o: &core_config::ResolvedOptions) -> BlankLinesBetweenMembers {
    match o.text(BLANK_LINES_BETWEEN_MEMBERS) {
        "separate" => BlankLinesBetweenMembers::Separate,
        "always" => BlankLinesBetweenMembers::Always,
        _ => BlankLinesBetweenMembers::Preserve,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessorBlankLines { Preserve, Remove }
#[allow(dead_code)]
pub fn accessor_blank_lines(o: &core_config::ResolvedOptions) -> AccessorBlankLines {
    match o.text(ACCESSOR_BLANK_LINES) {
        "remove" => AccessorBlankLines::Remove,
        _ => AccessorBlankLines::Preserve,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionBlankLines { Preserve, Remove }
#[allow(dead_code)]
pub fn collection_blank_lines(o: &core_config::ResolvedOptions) -> CollectionBlankLines {
    match o.text(COLLECTION_BLANK_LINES) {
        "remove" => CollectionBlankLines::Remove,
        _ => CollectionBlankLines::Preserve,
    }
}
#[allow(dead_code)]
pub fn inline_accessor_bodies(o: &core_config::ResolvedOptions) -> bool {
    o.flag(INLINE_ACCESSOR_BODIES, false)
}
#[allow(dead_code)]
pub fn preserve_single_line_blocks(o: &core_config::ResolvedOptions) -> bool {
    o.flag(PRESERVE_SINGLE_LINE_BLOCKS, false)
}
#[allow(dead_code)]
pub fn preserve_single_line_statements(o: &core_config::ResolvedOptions) -> bool {
    o.flag(PRESERVE_SINGLE_LINE_STATEMENTS, false)
}
#[allow(dead_code)]
pub fn indent_switch_section_block(o: &core_config::ResolvedOptions) -> bool {
    o.flag(INDENT_SWITCH_SECTION_BLOCK, true)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchArmLayout { Inline, Assignment }
#[allow(dead_code)]
pub fn switch_arm_layout(o: &core_config::ResolvedOptions) -> SwitchArmLayout {
    match o.text(SWITCH_ARM_LAYOUT) {
        "assignment" => SwitchArmLayout::Assignment,
        _ => SwitchArmLayout::Inline,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WrappedBraceLine { Creation, All, None }
#[allow(dead_code)]
pub fn wrapped_brace_line(o: &core_config::ResolvedOptions) -> WrappedBraceLine {
    match o.text(WRAPPED_BRACE_LINE) {
        "all" => WrappedBraceLine::All,
        "none" => WrappedBraceLine::None,
        _ => WrappedBraceLine::Creation,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitializerCollapse { Preserve, Always }
#[allow(dead_code)]
pub fn initializer_collapse(o: &core_config::ResolvedOptions) -> InitializerCollapse {
    match o.text(INITIALIZER_COLLAPSE) {
        "always" => InitializerCollapse::Always,
        _ => InitializerCollapse::Preserve,
    }
}
#[allow(dead_code)]
pub fn preserve_query_breaks(o: &core_config::ResolvedOptions) -> bool {
    o.flag(PRESERVE_QUERY_BREAKS, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitializerExpand { Fit, Crowded }
#[allow(dead_code)]
pub fn initializer_expand(o: &core_config::ResolvedOptions) -> InitializerExpand {
    match o.text(INITIALIZER_EXPAND) {
        "crowded" => InitializerExpand::Crowded,
        _ => InitializerExpand::Fit,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LambdaArgumentHug { Last, Sole }
#[allow(dead_code)]
pub fn lambda_argument_hug(o: &core_config::ResolvedOptions) -> LambdaArgumentHug {
    match o.text(LAMBDA_ARGUMENT_HUG) {
        "sole" => LambdaArgumentHug::Sole,
        _ => LambdaArgumentHug::Last,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructorInitializerLine { SameLine, NewLine }
#[allow(dead_code)]
pub fn constructor_initializer_line(o: &core_config::ResolvedOptions) -> ConstructorInitializerLine {
    match o.text(CONSTRUCTOR_INITIALIZER_LINE) {
        "new_line" => ConstructorInitializerLine::NewLine,
        _ => ConstructorInitializerLine::SameLine,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberAttributeLine { OwnLine, Keep }
#[allow(dead_code)]
pub fn member_attribute_line(o: &core_config::ResolvedOptions) -> MemberAttributeLine {
    match o.text(MEMBER_ATTRIBUTE_LINE) {
        "keep" => MemberAttributeLine::Keep,
        _ => MemberAttributeLine::OwnLine,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterAttributeLayout { Inline, Hanging }
#[allow(dead_code)]
pub fn parameter_attribute_layout(o: &core_config::ResolvedOptions) -> ParameterAttributeLayout {
    match o.text(PARAMETER_ATTRIBUTE_LAYOUT) {
        "hanging" => ParameterAttributeLayout::Hanging,
        _ => ParameterAttributeLayout::Inline,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseListLayout { Inline, Hanging }
#[allow(dead_code)]
pub fn base_list_layout(o: &core_config::ResolvedOptions) -> BaseListLayout {
    match o.text(BASE_LIST_LAYOUT) {
        "hanging" => BaseListLayout::Hanging,
        _ => BaseListLayout::Inline,
    }
}
#[allow(dead_code)]
pub fn unbreakable_single_item_lists(o: &core_config::ResolvedOptions) -> bool {
    o.flag(UNBREAKABLE_SINGLE_ITEM_LISTS, false)
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoWhileLine { NewLine, SameLine }
#[allow(dead_code)]
pub fn do_while_line(o: &core_config::ResolvedOptions) -> DoWhileLine {
    match o.text(DO_WHILE_LINE) {
        "same_line" => DoWhileLine::SameLine,
        _ => DoWhileLine::NewLine,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClauseKeywordLine { NewLine, SameLine }
#[allow(dead_code)]
pub fn clause_keyword_line(o: &core_config::ResolvedOptions) -> ClauseKeywordLine {
    match o.text(CLAUSE_KEYWORD_LINE) {
        "same_line" => ClauseKeywordLine::SameLine,
        _ => ClauseKeywordLine::NewLine,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElseLine { Inherit, NewLine, SameLine }
#[allow(dead_code)]
pub fn else_line(o: &core_config::ResolvedOptions) -> ElseLine {
    match o.text(ELSE_LINE) {
        "new_line" => ElseLine::NewLine,
        "same_line" => ElseLine::SameLine,
        _ => ElseLine::Inherit,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatchLine { Inherit, NewLine, SameLine }
#[allow(dead_code)]
pub fn catch_line(o: &core_config::ResolvedOptions) -> CatchLine {
    match o.text(CATCH_LINE) {
        "new_line" => CatchLine::NewLine,
        "same_line" => CatchLine::SameLine,
        _ => CatchLine::Inherit,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinallyLine { Inherit, NewLine, SameLine }
#[allow(dead_code)]
pub fn finally_line(o: &core_config::ResolvedOptions) -> FinallyLine {
    match o.text(FINALLY_LINE) {
        "new_line" => FinallyLine::NewLine,
        "same_line" => FinallyLine::SameLine,
        _ => FinallyLine::Inherit,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelIndent { NoChange, OneLess, FlushLeft }
#[allow(dead_code)]
pub fn label_indent(o: &core_config::ResolvedOptions) -> LabelIndent {
    match o.text(LABEL_INDENT) {
        "one_less" => LabelIndent::OneLess,
        "flush_left" => LabelIndent::FlushLeft,
        _ => LabelIndent::NoChange,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyBraces { Space, Tight, Expanded }
#[allow(dead_code)]
pub fn empty_braces(o: &core_config::ResolvedOptions) -> EmptyBraces {
    match o.text(EMPTY_BRACES) {
        "tight" => EmptyBraces::Tight,
        "expanded" => EmptyBraces::Expanded,
        _ => EmptyBraces::Space,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosingParenLine { OwnLine, SameLine }
#[allow(dead_code)]
pub fn closing_paren_line(o: &core_config::ResolvedOptions) -> ClosingParenLine {
    match o.text(CLOSING_PAREN_LINE) {
        "same_line" => ClosingParenLine::SameLine,
        _ => ClosingParenLine::OwnLine,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlankLinesAfterBlock { Preserve, Require }
#[allow(dead_code)]
pub fn blank_lines_after_block(o: &core_config::ResolvedOptions) -> BlankLinesAfterBlock {
    match o.text(BLANK_LINES_AFTER_BLOCK) {
        "require" => BlankLinesAfterBlock::Require,
        _ => BlankLinesAfterBlock::Preserve,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlankLineBeforeComment { Preserve, Require }
#[allow(dead_code)]
pub fn blank_line_before_comment(o: &core_config::ResolvedOptions) -> BlankLineBeforeComment {
    match o.text(BLANK_LINE_BEFORE_COMMENT) {
        "require" => BlankLineBeforeComment::Require,
        _ => BlankLineBeforeComment::Preserve,
    }
}
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlankLineAfterComment { Preserve, Remove }
#[allow(dead_code)]
pub fn blank_line_after_comment(o: &core_config::ResolvedOptions) -> BlankLineAfterComment {
    match o.text(BLANK_LINE_AFTER_COMMENT) {
        "remove" => BlankLineAfterComment::Remove,
        _ => BlankLineAfterComment::Preserve,
    }
}
#[allow(dead_code)]
pub fn space_after_delegate(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_AFTER_DELEGATE, true)
}
#[allow(dead_code)]
pub fn space_between_empty_for_clauses(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_BETWEEN_EMPTY_FOR_CLAUSES, false)
}
#[allow(dead_code)]
pub fn space_after_empty_last_for_clause(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SPACE_AFTER_EMPTY_LAST_FOR_CLAUSE, false)
}
#[allow(dead_code)]
pub fn sort_usings(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SORT_USINGS, false)
}
#[allow(dead_code)]
pub fn sort_usings_system_first(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SORT_USINGS_SYSTEM_FIRST, true)
}
#[allow(dead_code)]
pub fn separate_using_groups(o: &core_config::ResolvedOptions) -> bool {
    o.flag(SEPARATE_USING_GROUPS, false)
}

#[rustfmt::skip]
pub fn option_schema() -> core_config::OptionSchema {
    core_config::OptionSchema::lazy(|| {
        core_config::OptionSchema::new(vec![
        core_config::shared::line_width(120),
        core_config::shared::indent_size(4),
        shared::indent_style(),
        core_config::shared::final_newline(true),
        shared::end_of_line(),
        core_config::OptionDef::choice("brace_newline", &["all", "none"], "all", "Opening braces of types, members, blocks and lambdas: `all` puts each on its own line (Allman); `none` keeps it on the line of the declaration or statement."),
        core_config::OptionDef::words("brace_same_line", &[], "Elements whose opening brace stays on the line of the declaration or statement although `brace_newline` is `all`: `types`, `methods`, `properties`, `accessors`, `anonymous_methods`, `anonymous_types`, `control_blocks`, `lambdas`, `object_collection_array_initializers` (types include namespaces, methods include constructors, operators and local functions, properties include indexers and events, control blocks include `switch` statements and bare blocks)."),
        core_config::OptionDef::choice("trailing_commas", &["preserve", "never", "multiline"], "preserve", "Trailing comma after the last element of initializers, collection expressions, enums and switch expressions: `preserve` keeps what the source has; `never` removes it; `multiline` adds one when the list is on several lines and removes it otherwise."),
        core_config::shared::member_chain_min_calls(3),
        core_config::shared::member_chain_keep_accesses("never"),
        core_config::shared::member_chain_break_on_block("never"),
        core_config::shared::member_chain_short_head(4),
        core_config::shared::member_chain_indent(1),
        core_config::shared::binary_operator_line("start"),
        core_config::shared::binary_indent(1),
        core_config::shared::binary_hug("assignment"),
        core_config::shared::indent_nested_binary(false),
        core_config::shared::assignment_layout("compact"),
        core_config::shared::assignment_conditional("always"),
        core_config::shared::assignment_indent(1),
        core_config::shared::conditional_indent(1),
        core_config::shared::conditional_layout("indented"),
        core_config::OptionDef::words("preprocessor_symbols", &[], "Conditional compilation symbols (`DEBUG`, `NET8_0`) defined when parsing, which decide the active branch of `#if`. Branches that are inactive under these symbols stay as they are unless `preprocessor_symbol_sets` or `derive_symbol_sets` format them."),
        core_config::OptionDef::word_sets("preprocessor_symbol_sets", "Further sets of conditional compilation symbols. The file is also parsed with each set (a set replaces `preprocessor_symbols`), and the `#if` branches that are inactive under `preprocessor_symbols` but active under a set are formatted from that parse. Empty (the default) leaves inactive branches as they are."),
        core_config::OptionDef::boolean("derive_symbol_sets", false, "Also parse with sets of symbols taken from the `#if` and `#elif` conditions of the file (the empty set, all symbols, then each symbol, then pairs and so on, at most 16 sets, after those of `preprocessor_symbol_sets`) until every branch has been active once, and format the branches that are inactive under `preprocessor_symbols` from those parses. A branch no set activates stays as it is."),
        core_config::OptionDef::choice("blank_lines_between_members", &["preserve", "separate", "always"], "preserve", "Blank lines between the members of a type, namespace or enum: `preserve` keeps the source's (at most one) and never adds one; `separate` also requires one between two members when either has a body, an attribute or a comment above it; `always` requires one between any two members of a type or namespace and between the `using` directives and what follows them, except between two fields or two `using` directives (enum members as `separate`)."),
        core_config::OptionDef::choice("accessor_blank_lines", &["preserve", "remove"], "preserve", "Blank lines between the accessors of a property that is laid out on several lines: `preserve` keeps the source's (at most one); `remove` drops them."),
        core_config::OptionDef::choice("collection_blank_lines", &["preserve", "remove"], "preserve", "Blank lines between the elements of a collection initializer, array initializer or collection expression: `preserve` keeps the source's (at most one); `remove` drops them. The members of an object initializer keep theirs."),
        core_config::OptionDef::boolean("inline_accessor_bodies", false, "Keep the body of an accessor on its line (`get { return x; }`) when it is one simple statement without comments and fits; otherwise the braces go on their own lines."),
        core_config::OptionDef::boolean("preserve_single_line_blocks", false, "A block written on one line (`void M() { return; }`, `get { return x; }`,              `if (a)` followed by `{ b(); }`) stays on one line, as `{ a; b; }`, when it has no comment and no label in it. Type, namespace, enum and `switch` bodies written on one line stay as well."),
        core_config::OptionDef::boolean("preserve_single_line_statements", false, "Statements written on one line stay there: `a(); b();` in a sequence, `case 1:              break;`, and a whole `if`, `while`, `for`, `foreach`, `do`, `try`, `using`, `lock`,              `fixed`, `checked` or `unsafe` statement (with its blocks) written on one line,              unless it has a comment, a `switch` or a label in it."),
        core_config::OptionDef::boolean("indent_switch_section_block", true, "Indent a block that is the only statement of a `case` section below its label; when false the block starts at the label's indentation."),
        core_config::OptionDef::choice("switch_arm_layout", &["inline", "assignment"], "inline", "Layout of a switch expression arm: `inline` keeps `pattern => value` as one run of tokens; `assignment` lays out the value like the right-hand side of `=` (see `assignment_layout`)."),
        core_config::OptionDef::choice("wrapped_brace_line", &["creation", "all", "none"], "creation", "Opening brace of an initializer or property pattern that is laid out on several lines: `creation` puts it on its own line for `new T { ... }` only; `all` also for an initializer nested in an assignment (`Prop = { ... }`) and for property patterns; `none` keeps it on the line of the expression."),
        core_config::OptionDef::choice("initializer_collapse", &["preserve", "always"], "preserve", "Initializers and collection expressions: `preserve` keeps one expanded when the source has a line break in it; `always` puts one on a single line whenever it fits."),
        core_config::OptionDef::boolean("preserve_query_breaks", false, "A query expression with a line break in the source keeps one clause per line even when it fits on a line."),
        core_config::OptionDef::choice("initializer_expand", &["fit", "crowded"], "fit", "Initializers (and anonymous types) that fit on a line: `fit` keeps them on it; `crowded` puts one item per line when there are three or more member assignments, or two or more items one of which creates an object with an initializer."),
        core_config::OptionDef::choice("lambda_argument_hug", &["last", "sole"], "last", "A lambda argument whose body can hang from the call's line: `last` hugs a block lambda that is the last argument; `sole` hugs a lambda (block or expression) only when it is the only argument."),
        core_config::OptionDef::choice("constructor_initializer_line", &["same_line", "new_line"], "same_line", "The `: base(...)` or `: this(...)` initializer of a constructor: `same_line` keeps it after the parameter list; `new_line` puts it on its own indented line."),
        core_config::OptionDef::choice("member_attribute_line", &["own_line", "keep"], "own_line", "An attribute list in front of a member, statement or type: `own_line` puts it on a line of its own; `keep` leaves one that the source writes on the same line as what it annotates there (`[SerializeField] private int speed;`)."),
        core_config::OptionDef::choice("parameter_attribute_layout", &["inline", "hanging"], "inline", "A parameter with attribute lists: `inline` keeps them in front of the parameter;              `hanging` keeps them there when the parameter fits and otherwise puts the rest of              the parameter on an indented line below them."),
        core_config::OptionDef::choice("base_list_layout", &["inline", "hanging"], "inline", "The base list of a type (`: A, B`): `inline` keeps it on the line of the type name; `hanging` keeps it there when it fits and otherwise starts a line, with one base type per line."),
        core_config::OptionDef::boolean("unbreakable_single_item_lists", false, "Never break a list that holds one simple item: an attribute list with one attribute, a type argument list with one name, an index that is one name or literal."),
        core_config::OptionDef::choice("do_while_line", &["new_line", "same_line"], "new_line", "The `while` of a `do` statement: `new_line` starts a line; `same_line` follows the closing brace (`} while (x);`)."),
        core_config::OptionDef::choice("clause_keyword_line", &["new_line", "same_line"], "new_line", "`else`, `catch` and `finally` after a block: `new_line` starts a line; `same_line` follows the closing brace (`} else {`) unless a comment sits between them."),
        core_config::OptionDef::choice("else_line", &["inherit", "new_line", "same_line"], "inherit", "`else` after a block: `new_line` starts a line; `same_line` follows the closing brace; `inherit` takes `clause_keyword_line`."),
        core_config::OptionDef::choice("catch_line", &["inherit", "new_line", "same_line"], "inherit", "`catch` after a block: `new_line` starts a line; `same_line` follows the closing brace; `inherit` takes `clause_keyword_line`."),
        core_config::OptionDef::choice("finally_line", &["inherit", "new_line", "same_line"], "inherit", "`finally` after a block: `new_line` starts a line; `same_line` follows the closing brace; `inherit` takes `clause_keyword_line`."),
        core_config::OptionDef::choice("label_indent", &["no_change", "one_less", "flush_left"], "no_change", "Indentation of a `goto` label: `no_change` keeps the indentation of the statements around it; `one_less` indents it one level less; `flush_left` puts it in column 0."),
        core_config::OptionDef::choice("empty_braces", &["space", "tight", "expanded"], "space", "Braces with nothing between them: `space` prints `{ }`, `tight` prints `{}`, `expanded` puts the closing brace of an empty body on the line after the opening one (initializers, and lambdas and anonymous methods, stay `{ }`)."),
        core_config::OptionDef::choice("closing_paren_line", &["own_line", "same_line"], "own_line", "The `)` of a parameter, argument or attribute argument list that does not fit on one line: `own_line` puts it on a line of its own after the last item; `same_line` right after the last item."),
        core_config::OptionDef::choice("blank_lines_after_block", &["preserve", "require"], "preserve", "Blank line after a statement, member or switch section that ends with a closing brace: `preserve` keeps the source's (at most one); `require` adds one before the next statement, member or section."),
        core_config::OptionDef::choice("blank_line_before_comment", &["preserve", "require"], "preserve", "Blank line between a statement, member or switch section and a `//` comment on the lines before the next one (not `////`): `preserve` keeps the source's; `require` adds one."),
        core_config::OptionDef::choice("blank_line_after_comment", &["preserve", "remove"], "preserve", "Blank line between the last `//` or `///` comment in front of code and that code: `preserve` keeps the source's (at most one); `remove` drops it. Comments at the start of a file and `////` comments keep theirs."),
        core_config::OptionDef::words("blank_compact_kinds", &["FieldDeclaration", "UsingDirective", "ExternAliasDirective"], "Node kinds of which two in a row need no blank line between them when `blank_lines_between_members` is `always`."),
        core_config::OptionDef::words("blank_after_block_kinds", &["GetAccessorDeclaration", "SetAccessorDeclaration", "InitAccessorDeclaration", "AddAccessorDeclaration", "RemoveAccessorDeclaration", "UnknownAccessorDeclaration"], "Node kinds that get a blank line before them after a closing brace when `blank_lines_between_members` is `always`."),
        core_config::OptionDef::words("blank_directives_skip", &["#if", "#el", "#endif", "#endregion"], "Preprocessor directives (by their first characters) in front of a member or statement after which the blank line before it is the source's: the item may start after the directive, so the blank line options add none."),
        core_config::OptionDef::words("blank_directives_attach", &["#pragma"], "Preprocessor directives (by their first characters) that follow an item with no blank line between and have a blank line after them (`#pragma warning disable` after a member): they belong to that item, and the blank line options add no blank line before them."),
        core_config::OptionDef::boolean("space_after_delegate", true, "A space between `delegate` and the parameter list of an anonymous method (`delegate (int x) { }`); when false `delegate(int x) { }`."),
        core_config::OptionDef::boolean("space_between_empty_for_clauses", false, "A space between the two semicolons of a `for` with an empty condition (`for (; ;)`); when false `for (;;)`."),
        core_config::OptionDef::boolean("space_after_empty_last_for_clause", false, "A space before the `)` of a `for` whose last clause is empty (`for (int i = 0; i < n; )`); when false `for (int i = 0; i < n;)`."),
        core_config::OptionDef::boolean("sort_usings", false, "Sort `using` directives like the `using-order` rule with `compare: culture_ignore_case`, `static_position: after` and `alias_position: after` (System first, then the others, `using static`, aliases), and join the groups: blank lines between directives go, comments above a directive move with it. `global using` directives come first, `using global::X` behind the System ones. `#if` regions stay as they are behind the sorted directives; a list with another preprocessor directive is left alone."),
        core_config::OptionDef::boolean("sort_usings_system_first", true, "With `sort_usings`: `System` namespaces come before the others (`using static System.*` before other `using static`)."),
        core_config::OptionDef::boolean("separate_using_groups", false, "With `sort_usings`: one blank line between the System usings and the others (only with `sort_usings_system_first`)."),
        ])
        .with_presets(vec![
        core_config::Preset::new("csharp/dotnet-runtime", include_str!("../presets/dotnet-runtime.yaml")),
        core_config::Preset::new("csharp/google", include_str!("../presets/google.yaml")),
        core_config::Preset::new("csharp/csharpier-compat", include_str!("../presets/csharpier-compat.yaml")),
        core_config::Preset::new("csharp/stylecop", include_str!("../presets/stylecop.yaml")),
        core_config::Preset::new("csharp/unity", include_str!("../presets/unity.yaml")),
        ])
    })
}
