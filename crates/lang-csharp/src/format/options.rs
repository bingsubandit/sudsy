//! The C# format options: `brace_newline`, `trailing_commas` and `member_chain_min_calls` of
//! docs/ai/formatting/csharp.md, the shared ones, and the layout choices the presets of
//! the plan's table differ in. Every option has a documented default; the defaults are
//! Microsoft's layout. Presets are config files (`presets/`): they only set these options, and
//! nothing in the formatter knows which preset is in use. The typed getter of each option
//! (an enum per choice) comes from `lang.yaml`; what follows combines them.
//!
//! concept: none (glue: option readers)

use crate::generated::kind as k;
use core_config::{ResolvedOptions, shared};
use core_format::shapes::{ListLayout, TrailingComma};
use core_syntax::SyntaxKind;

include!("../schema.generated.rs");

/// Indentation levels a label is moved out by, `usize::MAX` for column 0.
pub fn label_outdent(options: &ResolvedOptions) -> usize {
    match label_indent(options) {
        LabelIndent::OneLess => 1,
        LabelIndent::FlushLeft => usize::MAX,
        LabelIndent::NoChange => 0,
    }
}

/// Members need a blank line between them when either has a body or something above it.
pub fn separate_members(options: &ResolvedOptions) -> bool {
    blank_lines_between_members(options) != BlankLinesBetweenMembers::Preserve
}

/// Every two members need a blank line between them (but two fields, two `using`s).
pub fn separate_all_members(options: &ResolvedOptions) -> bool {
    blank_lines_between_members(options) == BlankLinesBetweenMembers::Always
}

/// Empty bodies put the closing brace on their own line.
pub fn expand_empty_bodies(options: &ResolvedOptions) -> bool {
    empty_braces(options) == EmptyBraces::Expanded
}

/// Blank lines allowed between the elements of a collection (not object) initializer.
pub fn collection_max_blank_lines(options: &ResolvedOptions) -> usize {
    usize::from(collection_blank_lines(options) != CollectionBlankLines::Remove)
}

pub fn remove_accessor_blank_lines(options: &ResolvedOptions) -> bool {
    accessor_blank_lines(options) == AccessorBlankLines::Remove
}

pub fn fluid_assignments(options: &ResolvedOptions) -> bool {
    options.is(ASSIGNMENT_LAYOUT, "fluid")
}

pub fn switch_arm_as_assignment(options: &ResolvedOptions) -> bool {
    switch_arm_layout(options) == SwitchArmLayout::Assignment
}

/// The brace of a wrapped `new T { ... }` goes on its own line.
pub fn creation_brace_on_new_line(options: &ResolvedOptions) -> bool {
    wrapped_brace_line(options) != WrappedBraceLine::None
        && !options.has(shared::BRACE_SAME_LINE, INITIALIZERS)
}

/// The brace of a wrapped nested initializer or property pattern goes on its own line.
pub fn nested_brace_on_new_line(options: &ResolvedOptions) -> bool {
    wrapped_brace_line(options) == WrappedBraceLine::All
        && !options.has(shared::BRACE_SAME_LINE, INITIALIZERS)
}

/// The brace of a wrapped `new { ... }` (anonymous type) goes on its own line.
pub fn anonymous_type_brace_on_new_line(options: &ResolvedOptions) -> bool {
    !options.has(shared::BRACE_SAME_LINE, "anonymous_types")
}

/// The element name of initializer braces in `brace_same_line`.
const INITIALIZERS: &str = "object_collection_array_initializers";

/// Initializers and collection expressions.
pub fn initializer_layout(options: &ResolvedOptions) -> ListLayout {
    match initializer_collapse(options) {
        InitializerCollapse::Always => ListLayout::Fit,
        InitializerCollapse::Preserve => ListLayout::PreserveBreak,
    }
}

pub fn expand_crowded_initializers(options: &ResolvedOptions) -> bool {
    initializer_expand(options) == InitializerExpand::Crowded
}

pub fn hug_only_sole_lambda(options: &ResolvedOptions) -> bool {
    lambda_argument_hug(options) == LambdaArgumentHug::Sole
}

pub fn hanging_parameter_attributes(options: &ResolvedOptions) -> bool {
    parameter_attribute_layout(options) == ParameterAttributeLayout::Hanging
}

pub fn keeps_attribute_line(options: &ResolvedOptions) -> bool {
    member_attribute_line(options) == MemberAttributeLine::Keep
}

pub fn constructor_initializer_on_new_line(options: &ResolvedOptions) -> bool {
    constructor_initializer_line(options) == ConstructorInitializerLine::NewLine
}

pub fn hanging_base_list(options: &ResolvedOptions) -> bool {
    base_list_layout(options) == BaseListLayout::Hanging
}

pub fn hug_closing_paren(options: &ResolvedOptions) -> bool {
    closing_paren_line(options) == ClosingParenLine::SameLine
}

pub fn blank_after_block(options: &ResolvedOptions) -> bool {
    blank_lines_after_block(options) == BlankLinesAfterBlock::Require
}

pub fn blank_before_comment(options: &ResolvedOptions) -> bool {
    blank_line_before_comment(options) == BlankLineBeforeComment::Require
}

pub fn attach_comments(options: &ResolvedOptions) -> bool {
    blank_line_after_comment(options) == BlankLineAfterComment::Remove
}

pub fn using_order(options: &ResolvedOptions) -> (crate::using_sort::Order, bool) {
    (
        crate::using_sort::Order::formatter(sort_usings_system_first(options)),
        separate_using_groups(options),
    )
}

pub fn do_while_on_closing_line(options: &ResolvedOptions) -> bool {
    do_while_line(options) == DoWhileLine::SameLine
}

/// Does this `else`, `catch` or `finally` clause follow the closing brace of the block before?
pub fn clauses_on_closing_line(options: &ResolvedOptions, clause: SyntaxKind) -> bool {
    let own = match clause {
        k::ELSE_CLAUSE => match else_line(options) {
            ElseLine::Inherit => None,
            line => Some(line == ElseLine::SameLine),
        },
        k::CATCH_CLAUSE => match catch_line(options) {
            CatchLine::Inherit => None,
            line => Some(line == CatchLine::SameLine),
        },
        _ => match finally_line(options) {
            FinallyLine::Inherit => None,
            line => Some(line == FinallyLine::SameLine),
        },
    };
    own.unwrap_or_else(|| clause_keyword_line(options) == ClauseKeywordLine::SameLine)
}

/// What stands between the braces of an empty body.
pub fn empty_body_text(options: &ResolvedOptions) -> &'static str {
    if empty_braces(options) == EmptyBraces::Tight {
        ""
    } else {
        " "
    }
}

/// How a list that may carry a trailing comma is printed.
pub fn trailing_comma(options: &ResolvedOptions) -> TrailingComma {
    match trailing_commas(options) {
        TrailingCommas::Never => TrailingComma::Never,
        TrailingCommas::Multiline => TrailingComma::WhenBroken,
        TrailingCommas::Preserve => TrailingComma::AsSource,
    }
}
