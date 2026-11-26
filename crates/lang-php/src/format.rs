//! PHP formatting is data: `format.rules` (docs/ai/formatting/php.md).
//!
//! concept: rules-file-formatter

use core_config::shared;
use core_format::rules::{LazyRules, RuleFacts};
use core_syntax::grammars;

include!("schema.generated.rs");

#[allow(non_upper_case_globals)]
pub static PhpFormatter: LazyRules =
    LazyRules::new(|| grammars::PHP.get(), include_str!("../format.rules")).facts(RuleFacts {
        // Source spacing, hangs and blank lines stay where no row decides; a `{` ends a hang.
        exact: true,
        source_hang: true,
        keep_blank_lines: true,
        unhang: &["kind=LBrace"],
        // Operators that start a line (`operator_linebreak`).
        op_first: &[
            "kind=Operator&parent=Binary",
            "kind=Question|Colon&parent=Conditional",
        ],
        // Inline HTML and tags around it stay as written; a file ending in text keeps its end.
        one_line_verbatim: true,
        verbatim_if: &["InlineText", "CloseTag", "ShortOpeningTag", "EchoTag"],
        exact_end: &["InlineText"],
        max_blank_after: Some(usize::MAX),
        ..RuleFacts::DEFAULT
    });
