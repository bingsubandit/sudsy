//! What Java says to the shared scope rules (`core_rules::scope_rules`): names are bare tokens, so
//! every name that is not a declaration is a use, and the rows below list the declarations.

use core_rules::{ScopeLang, ScopeRule};
use core_semantics::scopes::{Declares, Flags, Flow, Place, Row, Sel};

const V: Flags = Flags::VALUE;

const fn names(kind: &'static str, flags: Flags, tokens: &'static [(Sel, Flow)]) -> Row {
    Row {
        kind,
        declares: Some(Declares {
            flags,
            place: Place::Scope,
            ..Declares::NONE
        }),
        tokens,
        ..Row::NONE
    }
}

const FIRST: &[(Sel, Flow)] = &[(Sel::First, Flow::Declare)];
const LAST: &[(Sel, Flow)] = &[(Sel::Last, Flow::Declare)];

const ROWS: &[Row] = &[
    Row {
        kind: "ImportDeclaration",
        inert: true,
        ..Row::NONE
    },
    Row {
        kind: "TypeOrStaticImportDeclaration",
        declares: Some(Declares {
            flags: V.union(Flags::TYPE).union(Flags::IMPORT),
            name_last: true,
            skip_token: "*",
            ..Declares::NONE
        }),
        ..Row::NONE
    },
    names("LocalVariableDeclarationCore", V.union(Flags::LET), &[]),
    // Fields are declared so that their declarators do not land in a local's declaration.
    names("FieldDeclaration", V, &[]),
    names("ConstantDeclaration", V, &[]),
    Row {
        kind: "VariableDeclarator",
        tokens: FIRST,
        ..Row::NONE
    },
    names("FormalParameter", V.union(Flags::PARAMETER), LAST),
    names("SpreadParameter", V.union(Flags::PARAMETER), LAST),
    names("CatchFormalParameter", V.union(Flags::CATCH), LAST),
    names("ForSpec", V, &[(Sel::LastBefore(":"), Flow::Declare)]),
    names(
        "LambdaExpression",
        V.union(Flags::PARAMETER),
        &[(Sel::LastBefore("->"), Flow::Declare)],
    ),
    names(
        "InferredParameters",
        V.union(Flags::PARAMETER),
        &[(Sel::All, Flow::Declare)],
    ),
    names("PatternVariable", V, FIRST),
    names("Resource", V, &[(Sel::LastBefore("="), Flow::Declare)]),
    Row {
        kind: "AssignmentExpression",
        tokens: &[(Sel::BeforeOp, Flow::Assigns)],
        ..Row::NONE
    },
    Row {
        kind: "UpdateExpression",
        tokens: &[(Sel::All, Flow::Assigns)],
        ..Row::NONE
    },
];

static LANG: ScopeLang = ScopeLang::new(ROWS, &["Identifier"]).bare().docs("/**");

pub static UNUSED_IMPORT: ScopeRule = ScopeRule::unused_import(&LANG);
pub static UNUSED_LOCAL: ScopeRule = ScopeRule::unused_local(&LANG);
pub static PARAMETER_ASSIGNMENT: ScopeRule = ScopeRule::parameter_assignment(&LANG);
