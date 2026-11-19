//! The role vocabulary: what a node kind is, for code that works across languages (layout kits,
//! scope binders, shared rules). A grammar gives a node kind a role by tagging it `#<role>`, or
//! marks it `#opaque` when no role fits; a grammar that declares `roles` must do one of the two
//! for every node kind (`Grammar::check_roles`, run by `cargo xtask codegen`). A kind may carry
//! several roles (a method is a declaration and a function).
//!
//! `fields` lists the named parts shared code reads through `Grammar::slot`; a name ending in `?`
//! is optional. A required field must be declared in the node's body, as the item's label or as
//! an alias (`when_true=then:#expression`); the item itself may be optional or repeated.
//!
//! concept: role-vocabulary
//! what: What a node kind is for cross-language code, by role tags.
//! tools: none (internal)
//! reads: grammar:language

use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

pub struct Role {
    pub name: &'static str,
    pub fields: &'static str,
    pub doc: &'static str,
}

pub const OPAQUE: &str = "opaque";

pub const ROLES: &[Role] = &[
    Role {
        name: "call",
        fields: "callee arguments",
        doc: "calls a function or method",
    },
    Role {
        name: "member-access",
        fields: "member object?",
        doc: "`object.member`; the object is absent in a binding (`.member`)",
    },
    Role {
        name: "index",
        fields: "arguments object?",
        doc: "`object[arguments]`; the object is absent in a binding",
    },
    Role {
        name: "binary",
        fields: "left operator right",
        doc: "two operands and an operator, any precedence",
    },
    Role {
        name: "unary",
        fields: "operand operator?",
        doc: "prefix, postfix or keyword operator on one operand",
    },
    Role {
        name: "assignment",
        fields: "left operator right",
        doc: "stores a value, plain or compound",
    },
    Role {
        name: "conditional",
        fields: "condition then else?",
        doc: "ternary expression, or an `if` statement or branch (`else` is the else branch, absent without one; in Python the first `elif`/`else` clause, whose siblings are the rest); a clause with no condition (`else`) has no `condition`",
    },
    Role {
        name: "loop",
        fields: "body condition? iterable? init? update?",
        doc: "`while`, `do`, `for`, `foreach`; no condition in a `foreach` (it has an `iterable`) or `for (;;)`; `init` and `update` are the parts of a `for` that run before the loop and after each pass",
    },
    Role {
        name: "switch",
        fields: "cases subject?",
        doc: "selects among cases by a value (`switch`, `match`, SQL `CASE`); `cases` are the `#case` nodes or the items that hold them",
    },
    Role {
        name: "case",
        fields: "test body",
        doc: "one arm of a switch; `test` is the value, pattern or condition (repeated when one arm has several labels, absent for the default arm)",
    },
    Role {
        name: "try",
        fields: "body catches finally?",
        doc: "`try` with its `#catch` clauses (any number) and `finally` block (optional)",
    },
    Role {
        name: "catch",
        fields: "body parameter? type? binding?",
        doc: "an exception handler; `parameter` is the caught type or variable, absent in a catch-all; `type` is the caught type (a list of alternatives is one node or the first of several items), absent when the handler catches everything or the grammar does not separate it from the variable; `binding` is the variable the error is bound to, where the grammar has it apart from `parameter`",
    },
    Role {
        name: "expression-statement",
        fields: "expression?",
        doc: "an expression used as a statement; `expression` is the expression",
    },
    Role {
        name: "empty-statement",
        fields: "",
        doc: "a statement that does nothing: a lone `;` or Python `pass`",
    },
    Role {
        name: "debugger",
        fields: "",
        doc: "a statement that stops in a debugger (`debugger`)",
    },
    Role {
        name: "return",
        fields: "value",
        doc: "leaves a function; `value` is optional",
    },
    Role {
        name: "throw",
        fields: "value? cause?",
        doc: "raises an exception; `value` is absent in a rethrow; `cause` is the error it is raised from",
    },
    Role {
        name: "break",
        fields: "label?",
        doc: "leaves the innermost loop or switch, or the labeled statement `label` names",
    },
    Role {
        name: "continue",
        fields: "label?",
        doc: "goes on with the next pass of the innermost loop, or of the labeled one",
    },
    Role {
        name: "assert",
        fields: "test message?",
        doc: "checks a condition that must hold, with an optional message (`assert test, message`)",
    },
    Role {
        name: "labeled",
        fields: "label body",
        doc: "a statement with a name that `break` and `continue` can address",
    },
    Role {
        name: "statements",
        fields: "items",
        doc: "a wrapper that holds statements without braces of its own: an `else` or `finally` clause, the body of `lock` or `using`",
    },
    Role {
        name: "key-value",
        fields: "key value?",
        doc: "an entry of an object or dictionary, or a named argument (`value` absent when it is the next sibling)",
    },
    Role {
        name: "function",
        fields: "parameters body? expression_body?",
        doc: "method, lambda or local function",
    },
    Role {
        name: "parameter-list",
        fields: "parameters",
        doc: "the parameters of a function with their brackets",
    },
    Role {
        name: "block",
        fields: "items",
        doc: "braced sequence of statements or members",
    },
    Role {
        name: "declaration",
        fields: "name?",
        doc: "introduces a name (member, accessor, namespace)",
    },
    Role {
        name: "type-declaration",
        fields: "members name?",
        doc: "class, struct, interface, enum",
    },
    Role {
        name: "import",
        fields: "path",
        doc: "brings the names of another file or namespace into scope",
    },
    Role {
        name: "literal",
        fields: "value",
        doc: "a constant written in the source",
    },
    Role {
        name: "parenthesized",
        fields: "",
        doc: "`(` one expression `)`",
    },
    Role {
        name: "scope",
        fields: "",
        doc: "names declared inside are not visible outside",
    },
    Role {
        name: "reference",
        fields: "name",
        doc: "uses a name",
    },
];

/// Operator kinds, the `operator_kind` of operator text: what an operator does, so a shared rule
/// asks of the token in an `operator` field whether it is an equality, in any language. A grammar
/// adds texts or overrides the common ones with `operators <kind> "text"...` lines.
pub const OPERATOR_KINDS: &[(&str, &str)] = &[
    ("equality", "`==` `!=` `===` `!==`"),
    ("comparison", "`<` `>` `<=` `>=` `<=>`"),
    ("logical", "`&&` `||` `!` `and` `or` `xor` `not`"),
    ("arithmetic", "`+` `-` `*` `/` `%` `**` `++` `--`"),
    ("bitwise", "`&` `|` `^` `~` `<<` `>>` `>>>`"),
    ("coalesce", "`??`"),
    ("membership", "`in`"),
    ("assign", "`=` `:=`"),
    ("compound", "`+=` and every other `op=`"),
];

/// The kind of operator text common to the languages (the table above); `None` for the rest.
pub fn common_operator_kind(text: &str) -> Option<&'static str> {
    Some(match text {
        "==" | "!=" | "===" | "!==" => "equality",
        "<" | ">" | "<=" | ">=" | "<=>" => "comparison",
        "&&" | "||" | "!" | "and" | "or" | "xor" | "not" => "logical",
        "+" | "-" | "*" | "/" | "%" | "**" | "++" | "--" => "arithmetic",
        "&" | "|" | "^" | "~" | "<<" | ">>" | ">>>" => "bitwise",
        "??" => "coalesce",
        "in" => "membership",
        "=" | ":=" => "assign",
        "+=" | "-=" | "*=" | "/=" | "%=" | "**=" | "//=" | "@=" | "&=" | "|=" | "^=" | "<<="
        | ">>=" | ">>>=" | "??=" | "&&=" | "||=" | ".=" => "compound",
        _ => return None,
    })
}

/// Tags a kind carries beside its role (or `#opaque`) that shared rules read: not roles, so a kind
/// with one still needs a role or `#opaque`.
pub const MARKS: &[(&str, &str)] = &[
    (
        "pure",
        "an expression that has no effect of its own (a name, a literal, a member access): as a whole statement it does nothing",
    ),
    (
        "with",
        "a statement that runs its body inside a resource or scope it opens (Python `with`, JS `with`); its statements are the node's statement children",
    ),
    (
        "yield",
        "an expression that hands a value to the caller of a generator (`yield`, `yield from`)",
    ),
    (
        "await",
        "an expression that waits for an asynchronous result (`await x`)",
    ),
];

/// Is `name` a role or a mark?
pub fn is_tag(name: &str) -> bool {
    role(name).is_some() || MARKS.iter().any(|m| m.0 == name)
}

pub fn role(name: &str) -> Option<&'static Role> {
    ROLES.iter().find(|r| r.name == name)
}

impl Role {
    /// The fields as (name, required).
    pub fn fields(&self) -> impl Iterator<Item = (&'static str, bool)> {
        self.fields
            .split_whitespace()
            .map(|f| f.strip_suffix('?').map_or((f, true), |n| (n, false)))
    }
}

/// Does the kind carry the tag? Answered by the grammar of the kind's own language.
#[inline]
pub fn has(kind: SyntaxKind, tag: &str) -> bool {
    crate::grammars::of(kind.lang()).is_some_and(|g| g.has_tag(kind, tag))
}

/// Does the kind carry any of the tags?
#[inline]
pub fn has_any(kind: SyntaxKind, tags: &[&str]) -> bool {
    crate::grammars::of(kind.lang()).is_some_and(|g| tags.iter().any(|t| g.has_tag(kind, t)))
}

/// The node without the `#transparent` wrappers around it (parentheses): the expression inside
/// any number of them.
pub fn un(node: &SyntaxNode) -> SyntaxNode {
    let mut n = node.clone();
    let Some(g) = crate::grammars::of(n.kind().lang()) else {
        return n;
    };
    while g.has_tag(n.kind(), "transparent")
        && let Some(inner) = n.children().next()
    {
        n = inner;
    }
    n
}

/// The node with the `#transparent` wrappers around it: as its parent holds it.
pub fn outer(node: &SyntaxNode) -> SyntaxNode {
    let mut n = node.clone();
    let Some(g) = crate::grammars::of(n.kind().lang()) else {
        return n;
    };
    while let Some(p) = n.parent() {
        if !g.has_tag(p.kind(), "transparent") {
            break;
        }
        n = p;
    }
    n
}

/// The node that holds `node` and the wrappers around it.
pub fn parent(node: &SyntaxNode) -> Option<SyntaxNode> {
    outer(node).parent()
}

/// The own tokens of a node that are code: neither trivia nor empty.
pub fn code_tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    let g = crate::grammars::of(node.kind().lang());
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(move |t| !g.is_some_and(|g| g.is_trivia(t.kind())))
}

/// The code tokens of a node written one after the other: `a.b` for a dotted name, the node
/// without its whitespace and comments.
pub fn code_text(node: &SyntaxNode) -> String {
    let g = crate::grammars::of(node.kind().lang());
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !g.is_some_and(|g| g.is_trivia(t.kind())))
        .map(|t| t.text().to_owned())
        .collect()
}

/// Does the node have an own code token with this text?
pub fn has_own_token(node: &SyntaxNode, text: &str) -> bool {
    code_tokens(node).any(|t| t.text() == text)
}
