//! The rules of JavaScript and TypeScript: pattern files in `rules/*.yaml`, and the `check`
//! family they name for ESLint's core rules that read the syntax tree. `cargo xtask new-rule js
//! <id> --rust` adds a module under `rules/` and lists it here.
//!
//! concept: none (glue: rule registration)

mod binder;
mod dupes;
mod imports;
mod literals;
mod misc;
mod reach;
mod scope;
mod stmts;
mod ts;

use crate::generated::kind as k;
use core_rules::tree::significant_range;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::roles::{code_text, code_tokens as own_code, has_own_token as has_own, un};
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

pub static RULES: &[&dyn Rule] = &[];

pub static FAMILIES: &[Family] = &[FAMILY, SCOPE, REGEX, core_rules::simplify::FAMILY];

/// The scope rules, shared with the other languages (`core_rules::scope_rules`).
const SCOPE: Family = Family {
    name: "scope",
    build: |meta, p| core_rules::scope_rules::build(&crate::semantic::LANG, meta, p),
};

/// The regex rules, shared with the other languages (`core_rules::regex_source`); the scope
/// model follows the names of variables into `new RegExp(name)`.
const REGEX: Family = Family {
    name: "regex",
    build: |meta, p| core_rules::regex_source::build(Some(&crate::semantic::LANG), meta, p),
};

const FAMILY: Family = Family {
    name: "check",
    build,
};

pub(super) use core_rules::checks::{Entry, NodeCheck as Check};

const CHECKS: &[Entry] = &[
    (
        "dupe-class-members",
        &[k::CLASS_BODY],
        dupes::dupe_class_members,
    ),
    ("dupe-else-if", &[k::IF_STATEMENT], dupes::dupe_else_if),
    ("useless-catch", &[k::TRY_STATEMENT], misc::useless_catch),
    (
        "use-isnan",
        &[k::BINARY_EXPRESSION, k::SWITCH_STATEMENT, k::SWITCH_CASE],
        misc::use_isnan,
    ),
    ("valid-typeof", &[k::BINARY_EXPRESSION], misc::valid_typeof),
    (
        "unsafe-finally",
        &[
            k::RETURN_STATEMENT,
            k::THROW_STATEMENT,
            k::BREAK_STATEMENT,
            k::CONTINUE_STATEMENT,
        ],
        stmts::unsafe_finally,
    ),
    (
        "getter-return",
        &[k::METHOD_DEFINITION, k::OBJECT_PROPERTY],
        reach::getter_return,
    ),
    ("require-yield", &[k::FUNCTION], stmts::require_yield),
    (
        "irregular-whitespace",
        &[k::PROGRAM],
        literals::irregular_whitespace,
    ),
    (
        "loss-of-precision",
        &[k::NUMERIC_LITERAL],
        literals::loss_of_precision,
    ),
    (
        "unexpected-multiline",
        &[
            k::COMPUTED_MEMBER_EXPRESSION,
            k::CALL_EXPRESSION,
            k::TAGGED_TEMPLATE_EXPRESSION,
        ],
        stmts::unexpected_multiline,
    ),
    (
        "duplicate-enum-values",
        &[k::TS_ENUM_DECLARATION],
        stmts::duplicate_enum_values,
    ),
    ("const-assign", &[k::PROGRAM], scope::const_assign),
    ("class-assign", &[k::PROGRAM], scope::class_assign),
    ("func-assign", &[k::PROGRAM], scope::func_assign),
    ("import-assign", &[k::PROGRAM], scope::import_assign),
    ("prefer-const", &[k::PROGRAM], binder::prefer_const),
    (
        "prefer-rest-params",
        &[k::PROGRAM],
        binder::prefer_rest_params,
    ),
    (
        "unsafe-function-type",
        &[k::PROGRAM],
        binder::unsafe_function,
    ),
    ("wrapper-object-types", &[k::PROGRAM], binder::wrapper_types),
    (
        "require-imports",
        &[k::CALL_EXPRESSION, k::TS_EXTERNAL_MODULE_REFERENCE],
        imports::require_imports,
    ),
    (
        "type-imports",
        &[k::PROGRAM, k::TS_IMPORT_TYPE],
        imports::type_imports,
    ),
    ("ban-ts-comment", &[k::PROGRAM], ts::ban_ts_comment),
    (
        "extra-non-null",
        &[k::TS_NON_NULL_EXPRESSION],
        ts::extra_non_null,
    ),
    (
        "misused-new",
        &[
            k::METHOD_DEFINITION,
            k::TS_CONSTRUCT_SIGNATURE_DECLARATION,
            k::TS_METHOD_SIGNATURE,
        ],
        ts::misused_new,
    ),
    (
        "prefer-as-const",
        &[k::TS_LITERAL_TYPE],
        ts::prefer_as_const,
    ),
    (
        "inferrable-types",
        &[
            k::VARIABLE_DECLARATOR,
            k::FORMAL_PARAMETER,
            k::PROPERTY_DEFINITION,
            k::ACCESSOR_PROPERTY,
        ],
        ts::inferrable_types,
    ),
    ("array-type", &[k::TS_TYPE_REFERENCE], ts::array_type),
    (
        "triple-slash-reference",
        &[k::PROGRAM],
        ts::triple_slash_reference,
    ),
    (
        "adjacent-overloads",
        &[
            k::CLASS_BODY,
            k::PROGRAM,
            k::TS_MODULE_BLOCK,
            k::TS_INTERFACE_BODY,
            k::FUNCTION_BODY,
            k::BLOCK_STATEMENT,
            k::TS_TYPE_LITERAL,
        ],
        ts::adjacent_overloads,
    ),
];

fn build(meta: &'static RuleMeta, p: Params) -> Result<Box<dyn Rule>, String> {
    core_rules::checks::build(CHECKS, meta, p)
}

/// The span of `n` without its surrounding trivia.
fn span(ctx: &RuleContext<'_>, n: &SyntaxNode) -> TextRange {
    significant_range(n, ctx.file.grammar)
}

/// The first token of `n` that is code (the node's own first token may be leading trivia).
fn code_token(n: &SyntaxNode, g: &Grammar) -> Option<SyntaxToken> {
    (n.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .find(|t| !g.is_trivia(t.kind()))
}

/// The operator of a unary, binary, logical or assignment expression.
fn operator(n: &SyntaxNode) -> String {
    let ops = |t: &SyntaxToken| {
        matches!(
            t.kind(),
            k::OPERATOR | k::KEYWORD | k::L_ANGLE | k::R_ANGLE | k::PIPE
        )
    };
    own_code(n)
        .find(ops)
        .map_or_else(String::new, |t| t.text().to_string())
}

/// The condition of an `if`, `while`, `do`, `for` or `?:`, as written (parentheses included).
fn test_of(n: &SyntaxNode) -> Option<SyntaxNode> {
    match n.kind() {
        k::DO_WHILE_STATEMENT => n.children().nth(1),
        k::FOR_STATEMENT => {
            let mut semicolons = 0;
            for e in n.children_with_tokens() {
                match e {
                    SyntaxElement::Token(t) if t.kind() == k::SEMICOLON => semicolons += 1,
                    SyntaxElement::Node(c) if semicolons == 1 => return Some(c),
                    _ => {}
                }
            }
            None
        }
        _ => n.children().next(),
    }
}
