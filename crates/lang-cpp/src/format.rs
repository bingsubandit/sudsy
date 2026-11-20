//! C and C++ formatting is data: `format.rules` (docs/ai/formatting/cpp.md).
//!
//! concept: rules-file-formatter

use core_config::shared;
use core_format::rules::{LazyRules, RuleFacts};
use core_syntax::grammars;

include!("schema.generated.rs");

#[allow(non_upper_case_globals)]
pub static CppFormatter: LazyRules =
    LazyRules::new(|| grammars::CPP.get(), include_str!("../format.rules"))
        .option_kinds(OPTION_KINDS)
        .facts(RuleFacts {
            // Source line breaks and blanks stay where no row decides; the `{` of a block ends a hang.
            exact: false,
            source_hang: true,
            keep_blank_lines: true,
            unhang: &["kind=Lbrace&parent=Block"],
            op_first: &[],
            one_line_verbatim: false,
            verbatim_if: &[],
            exact_end: &[],
            max_blank_after: None,
            keep_comment_gap: true,
        });
