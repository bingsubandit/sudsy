//! What Kotlin says to the shared scope rules (`core_rules::scope_rules`): an import declares the
//! last name of its path or its alias, and every `ReferenceExpression` is a use.

use core_rules::{ScopeLang, ScopeRule};
use core_semantics::scopes::{Declares, Flags, Flow, Row, Sel};

const ROWS: &[Row] = &[
    Row {
        kind: "PackageDirective",
        inert: true,
        ..Row::NONE
    },
    Row {
        kind: "ImportDirective",
        inert: true,
        declares: Some(Declares {
            flags: Flags::VALUE.union(Flags::TYPE).union(Flags::IMPORT),
            name_last: true,
            skip_token: "*",
            ..Declares::NONE
        }),
        ..Row::NONE
    },
    Row {
        kind: "ReferenceExpression",
        reference: true,
        ..Row::NONE
    },
    // An infix call (`a then b`) names its function in an operation reference.
    Row {
        kind: "OperationReference",
        tokens: &[(Sel::All, Flow::Through)],
        ..Row::NONE
    },
];

/// Functions a call site uses without naming them (operators, delegates, destructuring).
const CONVENTIONS: &[&str] = &[
    "getValue",
    "setValue",
    "provideDelegate",
    "invoke",
    "iterator",
    "hasNext",
    "next",
    "contains",
    "compareTo",
    "equals",
    "get",
    "set",
    "plus",
    "minus",
    "times",
    "div",
    "rem",
    "unaryPlus",
    "unaryMinus",
    "not",
    "inc",
    "dec",
    "rangeTo",
    "rangeUntil",
    "plusAssign",
    "minusAssign",
    "timesAssign",
    "divAssign",
    "remAssign",
    "component1",
    "component2",
    "component3",
    "component4",
    "component5",
];

static LANG: ScopeLang = ScopeLang::new(ROWS, &["Identifier"])
    .docs("/**")
    .conventions(CONVENTIONS);

pub static UNUSED_IMPORT: ScopeRule = ScopeRule::unused_import(&LANG);
