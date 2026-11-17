//! The rules of Python: pattern files in `rules/*.yaml`, and the `check` family they name for
//! ruff's rules that read the syntax tree and the scope model. `cargo xtask new-rule python <id>
//! --rust` adds a module under `rules/` and lists it here.

mod binding;
mod format;
mod lints;
mod scope;
mod stmts;
mod tree;

use crate::generated::kind as k;
use core_rules::{Family, Params, Rule, RuleMeta};

pub static RULES: &[&dyn Rule] = &[];

pub static FAMILIES: &[Family] = &[Family {
    name: "check",
    build,
}];

pub(super) use core_rules::checks::{Entry, NodeCheck as Check};

const CALL: &[core_syntax::SyntaxKind] = &[k::EXPR_CALL];
const UNARY: &[core_syntax::SyntaxKind] = &[k::EXPR_UNARY_OP];
const IF_EXPR: &[core_syntax::SyntaxKind] = &[k::EXPR_IF];
const HANDLER: &[core_syntax::SyntaxKind] = &[k::EXCEPT_HANDLER_EXCEPT_HANDLER];
const BIN_OP: &[core_syntax::SyntaxKind] = &[k::EXPR_BIN_OP];
const MODULE: &[core_syntax::SyntaxKind] = &[k::MOD_MODULE];

const CHECKS: &[Entry] = &[
    ("unused-import", MODULE, scope::unused_import),
    (
        "redefined-while-unused",
        MODULE,
        scope::redefined_while_unused,
    ),
    (
        "import-shadowed-by-loop-var",
        MODULE,
        scope::import_shadowed_by_loop_var,
    ),
    ("undefined-name", MODULE, scope::undefined_name),
    (
        "undefined-local-with-import-star-usage",
        MODULE,
        scope::star_usage,
    ),
    (
        "forward-annotation-syntax-error",
        MODULE,
        scope::forward_annotation_syntax_error,
    ),
    ("undefined-export", MODULE, scope::undefined_export),
    ("undefined-local", MODULE, scope::undefined_local),
    ("unused-variable", MODULE, scope::unused_variable),
    ("unused-annotation", MODULE, scope::unused_annotation),
    (
        "undefined-local-with-import-star",
        MODULE,
        scope::import_star,
    ),
    (
        "undefined-local-with-nested-import-star-usage",
        MODULE,
        scope::nested_import_star,
    ),
    (
        "module-import-not-at-top-of-file",
        MODULE,
        stmts::module_import_not_at_top,
    ),
    (
        "multiple-statements-on-one-line-colon",
        stmts::COMPOUND,
        stmts::colon_statement,
    ),
    (
        "useless-semicolon",
        tree::STATEMENTS,
        stmts::useless_semicolon,
    ),
    (
        "multiple-statements-on-one-line-semicolon",
        tree::STATEMENTS,
        stmts::statement_semicolon,
    ),
    ("ambiguous-variable-name", MODULE, stmts::ambiguous_variable),
    ("ambiguous-class-name", MODULE, stmts::ambiguous_class),
    ("ambiguous-function-name", MODULE, stmts::ambiguous_function),
    ("is-literal", &[k::EXPR_COMPARE], stmts::is_literal),
    (
        "type-comparison",
        &[k::EXPR_COMPARE],
        stmts::type_comparison,
    ),
    (
        "f-string-missing-placeholders",
        &[k::EXPR_F_STRING],
        stmts::f_string_missing_placeholders,
    ),
    ("bare-except", &[k::STMT_TRY], stmts::bare_except),
    (
        "default-except-not-last",
        &[k::STMT_TRY],
        stmts::default_except_not_last,
    ),
    (
        "lambda-assignment",
        &[k::STMT_ASSIGN, k::STMT_ANN_ASSIGN],
        stmts::lambda_assignment,
    ),
    (
        "invalid-print-syntax",
        &[k::EXPR_BIN_OP],
        stmts::invalid_print_syntax,
    ),
    (
        "multiple-starred-expressions",
        &[k::STMT_ASSIGN, k::STMT_ANN_ASSIGN],
        stmts::multiple_starred,
    ),
    (
        "break-outside-loop",
        &[k::STMT_BREAK],
        stmts::break_outside_loop,
    ),
    (
        "continue-outside-loop",
        &[k::STMT_CONTINUE],
        stmts::continue_outside_loop,
    ),
    (
        "return-outside-function",
        &[k::STMT_RETURN],
        stmts::return_outside_function,
    ),
    (
        "yield-outside-function",
        &[k::EXPR_YIELD, k::EXPR_YIELD_FROM, k::EXPR_AWAIT],
        stmts::yield_outside_function,
    ),
    (
        "get-attr-with-constant",
        CALL,
        lints::get_attr_with_constant,
    ),
    ("print", CALL, lints::print),
    ("pprint", CALL, lints::pprint),
    (
        "zip-without-explicit-strict",
        CALL,
        lints::zip_without_strict,
    ),
    (
        "unnecessary-collection-call",
        CALL,
        lints::unnecessary_collection_call,
    ),
    (
        "in-dict-keys",
        &[k::EXPR_COMPARE, k::COMPREHENSION, k::STMT_FOR],
        lints::in_dict_keys,
    ),
    ("negate-equal-op", UNARY, lints::negate_equal_op),
    ("negate-not-equal-op", UNARY, lints::negate_not_equal_op),
    ("double-negation", UNARY, lints::double_negation),
    (
        "if-expr-with-true-false",
        IF_EXPR,
        lints::if_expr_true_false,
    ),
    (
        "if-expr-with-false-true",
        IF_EXPR,
        lints::if_expr_false_true,
    ),
    (
        "if-expr-with-twisted-arms",
        IF_EXPR,
        lints::if_expr_twisted_arms,
    ),
    (
        "multiple-starts-ends-with",
        &[k::EXPR_BOOL_OP],
        lints::multiple_starts_ends_with,
    ),
    ("assert-false", &[k::STMT_ASSERT], lints::assert_false),
    (
        "set-attr-with-constant",
        &[k::STMT_EXPR],
        lints::set_attr_with_constant,
    ),
    (
        "unnecessary-placeholder",
        lints::BODIES,
        lints::unnecessary_placeholder,
    ),
    (
        "useless-object-inheritance",
        &[k::STMT_CLASS_DEF],
        lints::useless_object_inheritance,
    ),
    (
        "os-error-alias",
        &[k::STMT_RAISE, k::EXCEPT_HANDLER_EXCEPT_HANDLER],
        lints::os_error_alias_check,
    ),
    (
        "mutable-argument-default",
        &[k::PARAMETER_WITH_DEFAULT],
        lints::mutable_argument_default,
    ),
    (
        "redundant-tuple-in-exception-handler",
        HANDLER,
        lints::redundant_tuple_in_handler,
    ),
    (
        "raise-without-from-inside-except",
        HANDLER,
        lints::raise_without_from,
    ),
    (
        "unicode-literal",
        &[k::STRING_LITERAL],
        lints::unicode_literal,
    ),
    ("percent-format-invalid-format", BIN_OP, format::percent),
    (
        "percent-format-unsupported-format-character",
        BIN_OP,
        format::percent,
    ),
    (
        "percent-format-positional-count-mismatch",
        BIN_OP,
        format::percent,
    ),
    (
        "percent-format-mixed-positional-and-named",
        BIN_OP,
        format::percent,
    ),
    ("percent-format-expected-mapping", BIN_OP, format::percent),
    (
        "percent-format-extra-named-arguments",
        BIN_OP,
        format::percent,
    ),
    ("percent-format-missing-argument", BIN_OP, format::percent),
    ("percent-format-expected-sequence", BIN_OP, format::percent),
    (
        "percent-format-star-requires-sequence",
        BIN_OP,
        format::percent,
    ),
    ("string-dot-format-invalid-format", CALL, format::dot_format),
    (
        "string-dot-format-mixing-automatic",
        CALL,
        format::dot_format,
    ),
    (
        "string-dot-format-extra-positional-arguments",
        CALL,
        format::dot_format,
    ),
    (
        "string-dot-format-extra-named-arguments",
        CALL,
        format::dot_format,
    ),
    (
        "string-dot-format-missing-arguments",
        CALL,
        format::dot_format,
    ),
    (
        "invalid-escape-sequence",
        &[
            k::STRING_LITERAL,
            k::BYTES_LITERAL,
            k::F_STRING,
            k::T_STRING,
        ],
        format::invalid_escape,
    ),
    (
        "multi-value-repeated-key-literal",
        &[k::EXPR_DICT],
        format::dict_keys,
    ),
    (
        "multi-value-repeated-key-variable",
        &[k::EXPR_DICT],
        format::dict_keys,
    ),
    (
        "invalid-syntax",
        &[k::PARAMETERS, k::ARGUMENTS, k::STMT_NONLOCAL, k::EXPR_AWAIT],
        stmts::invalid_syntax,
    ),
    (
        "late-future-import",
        &[k::STMT_IMPORT_FROM],
        stmts::late_future_import,
    ),
    (
        "future-feature-not-defined",
        &[k::STMT_IMPORT_FROM],
        stmts::future_feature_not_defined,
    ),
];

fn build(meta: &'static RuleMeta, p: Params) -> Result<Box<dyn Rule>, String> {
    core_rules::checks::build(CHECKS, meta, p)
}
