//! What each token of an unwrapped line is (operator or declarator, template bracket or
//! comparison, ...), the blank before it, and whether and how dearly a line may break there.
//! The same questions clang-format's TokenAnnotator answers, by the same kind of heuristics.
//!
//! concept: none (token roles, spacing and break penalties are clang-format TokenAnnotator facts; the spacing mechanism is still local, see docs/ai/formatting/cpp.md)

use super::model::Tk;
use super::style::{BinBreak, PtrAlign, Style};
use crate::generated::kind as k;
use core_syntax::SyntaxKind;

pub const NONE: usize = usize::MAX;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Normal,
    Open,
    Close,
    Unary,
    Postfix,
    Binary,
    Assign,
    Ptr,
    TplOpen,
    TplClose,
    Question,
    TernColon,
    Comma,
    Semi,
    Label,
    CtorColon,
    InheritColon,
    RangeColon,
    BitColon,
    Member,
    Cast,
    Lambda,
    Arrow,
    Comment,
}

#[derive(Clone, Copy, Debug)]
pub struct Ann {
    pub role: Role,
    /// A blank before the token when it stays on the line.
    pub space: bool,
    /// The line must break before the token.
    pub must: bool,
    /// The line may break before the token, and what it costs.
    pub can: bool,
    pub pen: u32,
    /// How many brackets the token is inside.
    pub level: u32,
    /// The matching bracket.
    pub mate: usize,
    /// Precedence of a binary operator.
    pub prec: u8,
    /// A break here starts the line at the indentation of the statement (a function name after
    /// its return type).
    pub base: bool,
}

fn is_word(t: &Tk) -> bool {
    matches!(t.kind, k::IDENT | k::KEYWORD)
}

fn is_literal(t: &Tk) -> bool {
    matches!(t.kind, k::NUMBER | k::STRING | k::CHAR)
}

const TYPE_WORDS: &[&str] = &[
    "int",
    "char",
    "long",
    "short",
    "unsigned",
    "signed",
    "float",
    "double",
    "void",
    "bool",
    "auto",
    "wchar_t",
    "char8_t",
    "char16_t",
    "char32_t",
    "size_t",
    "const",
    "volatile",
    "struct",
    "class",
    "union",
    "enum",
    "typename",
    "decltype",
    "constexpr",
    "static",
    "extern",
    "inline",
    "register",
    "mutable",
    "virtual",
    "explicit",
    "friend",
    "thread_local",
    "typedef",
    "restrict",
    "__restrict",
];

fn type_word(t: &Tk) -> bool {
    t.kind == k::KEYWORD && TYPE_WORDS.contains(&t.text)
}

/// A word after which an operator is a prefix operator.
fn prefix_word(t: &Tk) -> bool {
    t.kind == k::KEYWORD
        && matches!(
            t.text,
            "return"
                | "case"
                | "throw"
                | "sizeof"
                | "co_return"
                | "co_yield"
                | "co_await"
                | "new"
                | "delete"
                | "else"
                | "do"
                | "and"
                | "or"
                | "not"
                | "alignof"
                | "typeid"
        )
}

/// The precedence of a binary operator token.
fn precedence(kind: SyntaxKind) -> u8 {
    match kind {
        k::PIPE_PIPE => 4,
        k::AMP_AMP => 5,
        k::PIPE => 6,
        k::CARET => 7,
        k::AMP => 8,
        k::EQ_EQ | k::BANG_EQ => 9,
        k::LESS | k::GREATER | k::LESS_EQ | k::GREATER_EQ => 10,
        k::SPACESHIP => 11,
        k::LESS_LESS | k::GREATER_GREATER => 12,
        k::PLUS | k::MINUS => 13,
        k::STAR | k::SLASH | k::PERCENT => 14,
        k::DOT_STAR | k::ARROW_STAR => 15,
        _ => 0,
    }
}

fn assignment(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::EQUAL
            | k::PLUS_EQ
            | k::MINUS_EQ
            | k::STAR_EQ
            | k::SLASH_EQ
            | k::PERCENT_EQ
            | k::CARET_EQ
            | k::AMP_EQ
            | k::PIPE_EQ
            | k::LESS_LESS_EQ
            | k::GREATER_GREATER_EQ
    )
}

fn opener(kind: SyntaxKind) -> bool {
    matches!(kind, k::LPAR | k::LSQB | k::LBRACE)
}

fn closer(kind: SyntaxKind) -> bool {
    matches!(kind, k::RPAR | k::RSQB | k::RBRACE)
}

/// The bracket pairs of the tokens, and which `<` `>` are template brackets.
fn pair(toks: &[Tk], decl: bool) -> (Vec<usize>, Vec<bool>) {
    let n = toks.len();
    let mut mate = vec![NONE; n];
    let mut stack = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if opener(t.kind) {
            stack.push(i);
        } else if closer(t.kind)
            && let Some(o) = stack.pop()
        {
            mate[o] = i;
            mate[i] = o;
        }
    }
    let mut tpl = vec![false; n];
    for i in 1..n {
        if toks[i].kind != k::LESS {
            continue;
        }
        let p = &toks[i - 1];
        let after_template = p.word("template");
        if !(p.kind == k::IDENT
            || after_template
            || (p.kind == k::KEYWORD && !type_word(p) && p.text != "operator")
            || (p.kind == k::KEYWORD && type_word(p) && decl))
            || p.text == "operator"
        {
            continue;
        }
        let mut depth = 1i32;
        let mut parens = 0i32;
        let mut end = NONE;
        let mut j = i + 1;
        while j < n {
            let t = &toks[j];
            match t.kind {
                k::LPAR | k::LSQB | k::LBRACE => parens += 1,
                k::RPAR | k::RSQB | k::RBRACE => {
                    if parens == 0 {
                        break;
                    }
                    parens -= 1;
                }
                k::SEMI if parens == 0 => break,
                // `a < b && c > d` is a comparison; `f<g<T>::v && h<T>::v>` has templates inside.
                k::PIPE_PIPE | k::AMP_AMP
                    if parens == 0
                        && !after_template
                        && !type_word(&toks[i - 1])
                        && depth == 1
                        // `std::enable_if_t<!A && B>`: a qualified name is a template.
                        && !(i >= 2 && toks[i - 2].kind == k::COLON_COLON)
                        && !toks[i + 1..j].iter().any(|t| t.kind == k::LESS) =>
                {
                    break;
                }
                k::EQ_EQ | k::BANG_EQ | k::LESS_EQ | k::GREATER_EQ | k::PLUS_EQ | k::MINUS_EQ
                    if parens == 0 =>
                {
                    break;
                }
                k::LESS if parens == 0 => depth += 1,
                k::GREATER if parens == 0 => {
                    depth -= 1;
                    if depth == 0 {
                        end = j;
                        break;
                    }
                }
                k::GREATER_GREATER if parens == 0 => {
                    depth -= 2;
                    if depth == 0 {
                        end = j;
                        break;
                    }
                    if depth < 0 {
                        // `>>` closes this template and the one around it.
                        if depth == -1 && toks[..i].iter().any(|t| t.kind == k::LESS) {
                            end = j;
                        }
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        if end == NONE {
            continue;
        }
        let plausible = match toks.get(end + 1) {
            None => true,
            Some(t) => {
                is_word(t)
                    || matches!(
                        t.kind,
                        k::COLON_COLON
                            | k::LPAR
                            | k::LBRACE
                            | k::GREATER
                            | k::GREATER_GREATER
                            | k::COMMA
                            | k::RPAR
                            | k::AMP
                            | k::AMP_AMP
                            | k::STAR
                            | k::SEMI
                            | k::ELLIPSIS
                            | k::LSQB
                            | k::EQUAL
                            | k::COLON
                            | k::DOT
                            | k::RSQB
                            | k::COMMENT
                    )
            }
        };
        if plausible {
            tpl[i] = true;
            tpl[end] = true;
            if toks[end].kind == k::GREATER_GREATER && depth == 0 {
                // `>>` closes this and the enclosing template: the enclosing `<` is found when
                // it is its turn; its scan ends at the same token.
            }
        }
    }
    (mate, tpl)
}

/// Whether the parenthesis group opened at `o` and closed at `c` is a C-style cast.
fn is_cast(toks: &[Tk], o: usize, c: usize, tpl: &[bool]) -> bool {
    if c == o + 1 || c + 1 >= toks.len() {
        return false;
    }
    if o > 0 {
        let p = &toks[o - 1];
        // `new (place) T`: placement arguments, not a cast.
        if p.word("new") {
            return false;
        }
        let ok = match p.kind {
            k::IDENT => false,
            k::KEYWORD => prefix_word(p),
            k::RPAR | k::RSQB | k::GREATER => false,
            k::NUMBER | k::STRING | k::CHAR => false,
            _ => true,
        };
        if !ok {
            return false;
        }
    }
    let mut typeish = false;
    for (i, t) in toks.iter().enumerate().take(c).skip(o + 1) {
        match t.kind {
            k::IDENT => {}
            k::KEYWORD => typeish |= type_word(t),
            k::COLON_COLON | k::COMMA => return false,
            k::STAR | k::AMP => typeish = true,
            k::LESS | k::GREATER | k::GREATER_GREATER if tpl[i] => typeish = true,
            _ => return false,
        }
    }
    let n = &toks[c + 1];
    match n.kind {
        k::IDENT | k::NUMBER | k::STRING | k::CHAR | k::LPAR | k::BANG | k::TILDE => true,
        k::KEYWORD => !matches!(
            n.text,
            "const" | "noexcept" | "override" | "final" | "volatile" | "mutable"
        ),
        k::MINUS | k::PLUS | k::STAR | k::AMP | k::PLUS_PLUS | k::MINUS_MINUS => typeish,
        _ => false,
    }
}

pub struct Cx<'a> {
    pub style: &'a Style,
    /// Declarations (file, namespace or class level) rather than statements.
    pub decl: bool,
}

pub fn annotate(toks: &[Tk], cx: &Cx) -> Vec<Ann> {
    let n = toks.len();
    let (mate, tpl) = pair(toks, cx.decl);
    let mut a: Vec<Ann> = (0..n)
        .map(|i| Ann {
            role: Role::Normal,
            space: false,
            must: false,
            can: false,
            pen: 0,
            level: 0,
            mate: mate[i],
            prec: 0,
            base: false,
        })
        .collect();
    // Levels.
    let mut depth = 0u32;
    for i in 0..n {
        let t = &toks[i];
        let close =
            closer(t.kind) && mate[i] != NONE && mate[i] < i || (tpl[i] && is_close_angle(t));
        if close {
            let drop = if t.kind == k::GREATER_GREATER && tpl[i] {
                2
            } else {
                1
            };
            depth = depth.saturating_sub(drop);
        }
        a[i].level = depth;
        if (opener(t.kind) && mate[i] != NONE && mate[i] > i) || (tpl[i] && t.kind == k::LESS) {
            depth += 1;
        }
    }
    roles(toks, &mut a, &tpl, cx);
    for i in 0..n {
        a[i].space = i > 0 && space(toks, &a, i, cx);
    }
    penalties(toks, &mut a, cx);
    a
}

fn is_close_angle(t: &Tk) -> bool {
    matches!(t.kind, k::GREATER | k::GREATER_GREATER)
}

fn roles(toks: &[Tk], a: &mut [Ann], tpl: &[bool], cx: &Cx) {
    let n = toks.len();
    let mut questions: Vec<u32> = vec![0; 64];
    // Text-level facts about the line.
    let first_word = toks.iter().find(|t| !t.comment());
    let record_line = toks
        .iter()
        .any(|t| t.kind == k::KEYWORD && matches!(t.text, "class" | "struct" | "union"));
    for i in 0..n {
        let t = &toks[i];
        let prev = if i > 0 { Some(&toks[i - 1]) } else { None };
        let next = toks.get(i + 1);
        let after_operator = prev.is_some_and(|p| p.word("operator"));
        let lvl = (a[i].level as usize).min(63);
        let role = match t.kind {
            _ if t.comment() => Role::Comment,
            k::COMMA => Role::Comma,
            k::SEMI => Role::Semi,
            k::LPAR | k::LBRACE => Role::Open,
            k::LSQB => {
                let lambda = prev.is_none_or(|p| {
                    !(is_word(p) && !prefix_word(p)
                        || is_literal(p)
                        || matches!(p.kind, k::RPAR | k::RSQB))
                        && !(p.kind == k::GREATER && tpl[i - 1])
                });
                let array_delete = prev.is_some_and(|p| p.word("delete"))
                    && next.is_some_and(|x| x.kind == k::RSQB);
                if lambda && !array_delete {
                    Role::Lambda
                } else {
                    Role::Open
                }
            }
            k::RPAR => {
                if a[i].mate != NONE && is_cast(toks, a[i].mate, i, tpl) {
                    Role::Cast
                } else {
                    Role::Close
                }
            }
            k::RSQB | k::RBRACE => Role::Close,
            k::LESS if tpl[i] => Role::TplOpen,
            k::GREATER | k::GREATER_GREATER if tpl[i] => Role::TplClose,
            _ if after_operator && !is_word(t) && !matches!(t.kind, k::LPAR | k::STRING) => {
                Role::Normal
            }
            k::DOT | k::ARROW => {
                let trailing = t.kind == k::ARROW
                    && prev.is_some_and(|p| {
                        matches!(p.kind, k::RPAR)
                            || matches!(p.text, "const" | "noexcept" | "override" | "mutable")
                    })
                    && toks[..i]
                        .iter()
                        .any(|x| x.word("auto") || x.kind == k::LSQB)
                    && a[i].level == 0;
                if trailing { Role::Arrow } else { Role::Member }
            }
            k::DOT_STAR | k::ARROW_STAR => Role::Member,
            k::QUESTION => {
                questions[lvl] += 1;
                Role::Question
            }
            k::COLON => {
                if questions[lvl] > 0 {
                    questions[lvl] -= 1;
                    Role::TernColon
                } else if a[i].level > 0 {
                    // Inside `for (x : y)`, or a bit field in a parenthesis (not found there).
                    let o = toks[..i]
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(j, x)| x.kind == k::LPAR && a[*j].mate > i)
                        .map(|(j, _)| j);
                    if o.is_some_and(|o| o > 0 && toks[o - 1].word("for")) {
                        Role::RangeColon
                    } else {
                        Role::TernColon
                    }
                } else if i + 1 == n
                    || next.is_some_and(|x| x.comment()) && i + 2 == n
                    || toks[0].kind == k::KEYWORD && matches!(toks[0].text, "case" | "default")
                {
                    Role::Label
                } else if prev.is_some_and(|p| {
                    p.kind == k::RPAR
                        || matches!(
                            p.text,
                            "const" | "noexcept" | "override" | "final" | "volatile"
                        )
                }) && toks[..i].iter().any(|x| x.kind == k::LPAR)
                {
                    Role::CtorColon
                } else if record_line {
                    Role::InheritColon
                } else if cx.decl && next.is_some_and(|x| x.kind == k::NUMBER || is_word(x)) {
                    Role::BitColon
                } else {
                    Role::TernColon
                }
            }
            k::PLUS_PLUS | k::MINUS_MINUS => {
                if prev.is_some_and(|p| {
                    (is_word(p) && !prefix_word(p))
                        || matches!(p.kind, k::RPAR | k::RSQB)
                        || (p.kind == k::GREATER && tpl[i - 1])
                }) {
                    Role::Postfix
                } else {
                    Role::Unary
                }
            }
            k::BANG | k::TILDE => Role::Unary,
            k::STAR | k::AMP | k::AMP_AMP | k::PLUS | k::MINUS => {
                star_amp(toks, a, tpl, cx, i, first_word)
            }
            _ if assignment(t.kind) => Role::Assign,
            k::LESS
            | k::GREATER
            | k::LESS_EQ
            | k::GREATER_EQ
            | k::EQ_EQ
            | k::BANG_EQ
            | k::SPACESHIP
            | k::SLASH
            | k::PERCENT
            | k::CARET
            | k::PIPE
            | k::PIPE_PIPE
            | k::LESS_LESS
            | k::GREATER_GREATER => Role::Binary,
            _ => Role::Normal,
        };
        a[i].role = role;
        a[i].prec = match role {
            Role::Binary => precedence(t.kind),
            Role::Ptr => 0,
            Role::Unary => 0,
            _ if matches!(t.kind, k::STAR | k::AMP | k::AMP_AMP | k::PLUS | k::MINUS)
                && role == Role::Normal =>
            {
                0
            }
            _ => 0,
        };
        if matches!(t.kind, k::STAR | k::AMP | k::AMP_AMP | k::PLUS | k::MINUS)
            && role == Role::Binary
        {
            a[i].prec = precedence(t.kind);
        }
    }
    // The binary roles of `* & + -` were decided as Binary above through `star_amp`.
    for i in 0..n {
        if a[i].role == Role::Binary && a[i].prec == 0 {
            a[i].prec = precedence(toks[i].kind);
        }
    }
}

/// `*`, `&`, `&&`, `+` and `-`: a prefix operator, a binary operator, or a pointer or reference
/// in a declarator.
fn star_amp(toks: &[Tk], a: &[Ann], tpl: &[bool], cx: &Cx, i: usize, _first: Option<&Tk>) -> Role {
    let t = &toks[i];
    let sign = matches!(t.kind, k::PLUS | k::MINUS);
    let Some(p) = i.checked_sub(1).map(|j| &toks[j]) else {
        return Role::Unary;
    };
    let next = toks.get(i + 1);
    // `Class::*member`: a pointer to member.
    if p.kind == k::COLON_COLON && t.kind == k::STAR {
        return Role::Ptr;
    }
    // After an operator, an opening bracket, a comma or a keyword like `return`: prefix.
    let after_value = match p.kind {
        k::RPAR => a[i - 1].role != Role::Cast && !control_paren(toks, a, i - 1),
        k::RSQB | k::NUMBER | k::STRING | k::CHAR => true,
        k::IDENT => true,
        k::KEYWORD => !prefix_word(p) && !matches!(p.text, "sizeof"),
        k::GREATER | k::GREATER_GREATER => tpl[i - 1],
        k::PLUS_PLUS | k::MINUS_MINUS => a[i - 1].role == Role::Postfix,
        k::ELLIPSIS => true,
        k::STAR | k::AMP | k::AMP_AMP => a[i - 1].role == Role::Ptr,
        _ => false,
    };
    if !after_value {
        return Role::Unary;
    }
    if sign {
        return Role::Binary;
    }
    // A value before: binary operator, or a declarator.
    let type_before = match p.kind {
        k::KEYWORD => type_word(p),
        k::GREATER | k::GREATER_GREATER => tpl[i - 1] && decl_start(toks, a, tpl, i, cx),
        k::STAR | k::AMP | k::AMP_AMP => a[i - 1].role == Role::Ptr,
        k::IDENT => decl_start(toks, a, tpl, i, cx),
        k::ELLIPSIS => false,
        _ => false,
    };
    if !type_before {
        return Role::Binary;
    }
    match next.map(|x| x.kind) {
        None => Role::Ptr,
        Some(nk) => match nk {
            k::RPAR
            | k::COMMA
            | k::GREATER
            | k::GREATER_GREATER
            | k::SEMI
            | k::STAR
            | k::AMP
            | k::AMP_AMP
            | k::LSQB
            | k::ELLIPSIS
            | k::EQUAL
            | k::LBRACE
            | k::LPAR => Role::Ptr,
            k::IDENT | k::KEYWORD => Role::Ptr,
            _ => Role::Binary,
        },
    }
}

/// Whether the `)` at `i` closes the condition of `if`, `for`, `while` or `switch`.
fn control_paren(toks: &[Tk], a: &[Ann], i: usize) -> bool {
    let o = a[i].mate;
    o != NONE
        && o > 0
        && toks[o - 1].kind == k::KEYWORD
        && matches!(toks[o - 1].text, "if" | "for" | "while" | "switch")
}

/// Whether the identifier before token `i` ends a type of a declaration: the tokens from the
/// start of the declaration up to it are all parts of a type, and the line is a declaration.
fn decl_start(toks: &[Tk], a: &[Ann], tpl: &[bool], i: usize, cx: &Cx) -> bool {
    // Walk back over the parts of a type.
    let mut j = i;
    let mut words = 0;
    let mut kw = false;
    while j > 0 {
        let t = &toks[j - 1];
        if t.text == "defined" {
            return false;
        }
        match t.kind {
            k::IDENT => words += 1,
            k::KEYWORD => {
                if matches!(
                    t.text,
                    "return"
                        | "case"
                        | "throw"
                        | "sizeof"
                        | "else"
                        | "new"
                        | "delete"
                        | "co_return"
                        | "defined"
                ) {
                    return false;
                }
                kw |= type_word(t);
                words += 1;
            }
            k::COLON_COLON => {}
            k::GREATER | k::GREATER_GREATER if tpl[j - 1] => {
                // Skip the template arguments.
                let mut depth = if t.kind == k::GREATER_GREATER { 2 } else { 1 };
                j -= 1;
                while j > 0 && depth > 0 {
                    let u = &toks[j - 1];
                    if tpl[j - 1] {
                        match u.kind {
                            k::GREATER => depth += 1,
                            k::GREATER_GREATER => depth += 2,
                            k::LESS => depth -= 1,
                            _ => {}
                        }
                    }
                    j -= 1;
                }
                continue;
            }
            k::STAR | k::AMP | k::AMP_AMP if a[j - 1].role == Role::Ptr => {}
            k::COMMENT => {}
            // `[[maybe_unused]]` before the type.
            k::RSQB if j >= 2 && toks[j - 2].kind == k::RSQB => {
                let mut open = 0;
                while j > 0 && open < 2 {
                    if toks[j - 1].kind == k::LSQB {
                        open += 1;
                    }
                    j -= 1;
                }
                continue;
            }
            _ => break,
        }
        j -= 1;
    }
    if words == 0 {
        return false;
    }
    // What starts the declaration.
    let before = if j == 0 { None } else { Some(&toks[j - 1]) };
    match before {
        None => cx.decl || words >= 2 || starts_declaration_line(toks, i),
        Some(b) => match b.kind {
            k::LPAR => {
                // Parameters of a function declaration, or of a lambda or a cast.
                let o = j - 1;
                kw || param_paren(toks, o, cx)
            }
            k::COMMA => {
                // The next parameter or declarator of the enclosing group.
                let o = enclosing_paren(toks, a, j - 1);
                o.is_some_and(|o| kw || param_paren(toks, o, cx) || toks[o].kind == k::LESS)
                    || (a[j - 1].level == 0 && (cx.decl || words >= 2))
            }
            k::LESS => tpl[j - 1],
            k::SEMI | k::LBRACE | k::RBRACE => words >= 2 || cx.decl,
            k::RPAR => {
                // `if (x) T *p`, rare: not a declaration.
                false
            }
            k::COLON => false,
            // `using Alias = T *;`
            k::EQUAL => toks.first().is_some_and(|t| t.word("using")) && j >= 2,
            k::ARROW => a[j - 1].role == Role::Arrow,
            _ => false,
        },
    }
}

fn starts_declaration_line(toks: &[Tk], i: usize) -> bool {
    // `Type *name` at the start of a statement: the token after the declarator is a name.
    let mut j = i + 1;
    while toks
        .get(j)
        .is_some_and(|n| matches!(n.kind, k::STAR | k::AMP | k::AMP_AMP))
    {
        j += 1;
    }
    toks.get(j).is_some_and(|n| is_word(n))
        && toks
            .get(j + 1)
            .is_some_and(|n| matches!(n.kind, k::EQUAL | k::SEMI | k::COMMA | k::LSQB | k::COLON))
}

fn enclosing_paren(toks: &[Tk], a: &[Ann], at: usize) -> Option<usize> {
    (0..at).rev().find(|&o| {
        (toks[o].kind == k::LPAR || toks[o].kind == k::LESS) && a[o].mate != NONE && a[o].mate > at
            || toks[o].kind == k::LESS && a[o].role == Role::TplOpen && a[o].mate == NONE && o < at
    })
}

/// Whether the parenthesis at `o` holds the parameters of a function declaration: the
/// statement is a declaration and the parenthesis follows its name.
fn param_paren(toks: &[Tk], o: usize, cx: &Cx) -> bool {
    if o == 0 {
        return false;
    }
    let name = &toks[o - 1];
    // `[](int *x)`, `for (T *x : xs)`, `catch (T *e)`.
    if name.kind == k::RSQB || name.kind == k::KEYWORD && matches!(name.text, "for" | "catch") {
        return true;
    }
    if o >= 2
        && (toks[o - 2].word("operator")
            || (name.kind == k::RPAR
                && o >= 3
                && toks[o - 2].kind == k::LPAR
                && toks[o - 3].word("operator")))
    {
        return true;
    }
    if name.kind != k::IDENT {
        return false;
    }
    // Back over the qualified name.
    let mut s = o - 1;
    while s >= 2 && toks[s - 1].kind == k::COLON_COLON && toks[s - 2].kind == k::IDENT {
        s -= 2;
    }
    if s >= 1 && toks[s - 1].kind == k::COLON_COLON {
        s -= 1;
    }
    let prefix = &toks[..s];
    if prefix.is_empty() {
        return cx.decl;
    }
    let mut depth = 0i32;
    for t in prefix {
        if depth > 0 && !matches!(t.kind, k::LESS | k::GREATER | k::GREATER_GREATER) {
            continue;
        }
        match t.kind {
            k::LESS => depth += 1,
            k::GREATER => depth -= 1,
            k::GREATER_GREATER => depth -= 2,
            k::IDENT | k::COLON_COLON | k::STAR | k::AMP | k::AMP_AMP | k::TILDE | k::ELLIPSIS => {}
            k::KEYWORD => {
                if !type_word(t)
                    && !matches!(
                        t.text,
                        "template" | "operator" | "noexcept" | "unsigned" | "signed" | "typename"
                    )
                    && !t.text.starts_with("__")
                {
                    return false;
                }
            }
            k::COMMA
            | k::NUMBER
            | k::STRING
            | k::CHAR
            | k::LPAR
            | k::RPAR
            | k::LSQB
            | k::RSQB
            | k::LBRACE
            | k::RBRACE
                if depth > 0 => {}
            k::COMMENT => {}
            _ => return false,
        }
    }
    true
}

fn pointer_gap(style: &Style) -> PtrAlign {
    style.pointer
}

fn space(toks: &[Tk], a: &[Ann], i: usize, cx: &Cx) -> bool {
    let p = &toks[i - 1];
    let c = &toks[i];
    let (pa, ca) = (&a[i - 1], &a[i]);
    use Role::*;
    let style = cx.style;
    if c.blk {
        // The `{` and `}` of a block on one line with its statement.
        if c.kind == k::RBRACE {
            return !(p.blk && p.kind == k::LBRACE);
        }
        return true;
    }
    if p.blk && p.kind == k::LBRACE {
        return c.kind != k::RBRACE;
    }
    if ca.role == Comment {
        return true;
    }
    // `Ts &&...Args`: the pack goes with the declarator before it.
    if p.kind == k::ELLIPSIS && i >= 2 && a[i - 2].role == Ptr && is_word(c) {
        return style.pointer == PtrAlign::Left;
    }
    // `T * = nullptr`: the tokens must not run together into `*=`.
    if c.kind == k::EQUAL && matches!(p.kind, k::STAR | k::AMP | k::AMP_AMP) {
        return true;
    }
    if pa.role == Comment {
        // `f(/*name=*/value)`: an argument comment stays against its value.
        if p.text.ends_with("=*/") && !c.sp {
            return false;
        }
        return p.text.starts_with("//") || c.sp || !matches!(c.kind, k::COMMA | k::SEMI | k::RPAR);
    }
    match ca.role {
        Comma | Semi => return false,
        Postfix | Member => return false,
        Label => return false,
        Close | Cast if c.kind != k::RBRACE || !p.blk => return false,
        _ => {}
    }
    // `#x` and `a##b` in a macro body.
    if p.kind == k::HASH || p.kind == k::HASH_HASH || c.kind == k::HASH_HASH {
        return false;
    }
    if c.kind == k::COLON_COLON {
        return match pa.role {
            Open | TplOpen | Member | Normal if p.kind == k::COLON_COLON => false,
            _ if p.kind == k::KEYWORD => !matches!(p.text, "operator"),
            Binary | Assign | Comma | Question | TernColon | Arrow | CtorColon | InheritColon
            | RangeColon | BitColon => true,
            _ => false,
        };
    }
    match pa.role {
        Open => {
            return p.kind == k::LBRACE && p.blk;
        }
        Unary => {
            // `- -x`, `& &x`.
            return p.kind == c.kind && matches!(c.kind, k::PLUS | k::MINUS | k::AMP | k::AMP_AMP);
        }
        Member => return false,
        Cast => return false,
        Comma => return true,
        Semi => return !matches!(c.kind, k::SEMI | k::RPAR),
        Lambda => return false,
        _ => {}
    }
    if p.kind == k::COLON_COLON {
        return false;
    }
    // The symbol after `operator`.
    if i >= 2 && toks[i - 2].word("operator") && !is_word(p) && p.kind != k::STRING {
        return false;
    }
    if p.word("operator") {
        return is_word(c);
    }
    match (pa.role, ca.role) {
        (
            Binary | Assign | Question | TernColon | Arrow | CtorColon | InheritColon | RangeColon
            | BitColon,
            _,
        ) => return true,
        (
            _,
            Binary | Assign | Question | TernColon | Arrow | CtorColon | InheritColon | RangeColon
            | BitColon,
        ) => return true,
        (Label, _) => return true,
        _ => {}
    }
    if ca.role == Unary {
        return true;
    }
    // Template brackets.
    if ca.role == TplOpen {
        return p.word("template");
    }
    if pa.role == TplOpen {
        return false;
    }
    if ca.role == TplClose {
        // `> >` stays two tokens: `>>` would be another tree.
        return p.kind == k::GREATER && c.kind == k::GREATER;
    }
    if pa.role == TplClose {
        return match ca.role {
            Ptr => style.pointer != PtrAlign::Left,
            Open if c.kind == k::LBRACE && a[..i].iter().any(|x| x.role == InheritColon) => true,
            Open if c.kind == k::LPAR || c.kind == k::LBRACE || c.kind == k::LSQB => false,
            _ => is_word(c) || is_literal(c),
        };
    }
    // Pointers and references.
    if ca.role == Ptr {
        return match pa.role {
            Ptr => false,
            _ => style.pointer != PtrAlign::Left,
        };
    }
    if pa.role == Ptr {
        let tight = matches!(
            c.kind,
            k::RPAR
                | k::COMMA
                | k::SEMI
                | k::GREATER
                | k::GREATER_GREATER
                | k::RSQB
                | k::LSQB
                | k::ELLIPSIS
                | k::EQUAL
                | k::LPAR
        ) || matches!(ca.role, Ptr | Close | TplClose);
        if tight {
            return c.kind == k::EQUAL;
        }
        return match style.pointer {
            PtrAlign::Right => false,
            PtrAlign::Left => true,
            PtrAlign::Middle => true,
        };
    }
    // Brackets.
    match c.kind {
        k::LPAR => {
            // `int (*f)(int)`: a declarator in parentheses after a type.
            if (is_word(p) || p.kind == k::GREATER)
                && toks
                    .get(i + 1)
                    .is_some_and(|t| matches!(t.kind, k::STAR | k::AMP | k::CARET))
                && ca.mate != NONE
                && toks.get(ca.mate + 1).is_some_and(|t| {
                    t.kind == k::LPAR
                        || (t.kind == k::LSQB
                            && i >= 2
                            && (toks[i - 2].kind == k::IDENT || type_word(&toks[i - 2])))
                })
                && !matches!(
                    p.text,
                    "if" | "for" | "while" | "switch" | "return" | "sizeof" | "decltype" | "case"
                )
            {
                return true;
            }
            return match p.kind {
                k::KEYWORD => {
                    let ctrl = matches!(
                        p.text,
                        "if" | "for"
                            | "while"
                            | "switch"
                            | "catch"
                            | "_Generic"
                            | "constexpr"
                            | "consteval"
                            | "elif"
                            | "ifdef"
                    );
                    let value = matches!(p.text, "return" | "case" | "throw" | "co_return" | "co_await" | "co_yield" | "new" | "delete" | "else" | "do" | "and" | "or" | "not" | "operator" if p.text != "operator");
                    match style.parens.as_str() {
                        "never" => value,
                        "always" => !matches!(
                            p.text,
                            "sizeof"
                                | "alignof"
                                | "decltype"
                                | "alignas"
                                | "noexcept"
                                | "operator"
                                | "typeid"
                                | "defined"
                                | "requires"
                                | "static_assert"
                                | "__attribute__"
                                | "asm"
                                | "__asm__"
                        ),
                        _ => ctrl || value,
                    }
                }
                _ => false,
            };
        }
        k::LSQB => {
            return match ca.role {
                Lambda => true,
                _ => {
                    if p.kind == k::LSQB
                        || p.kind == k::RSQB && toks.get(i + 1).is_some_and(|n| n.kind == k::LSQB)
                    {
                        false
                    } else {
                        // `auto [a, b]`, `const auto &[a, b]`: structured bindings.
                        p.word("auto")
                            || pa.role == Ptr && i >= 2 && toks[i - 2].word("auto")
                            || p.kind == k::KEYWORD
                                && !type_word(p)
                                && !matches!(p.text, "delete" | "new" | "operator")
                    }
                }
            };
        }
        k::LBRACE => {
            return match p.kind {
                k::EQUAL => true,
                k::LPAR | k::LSQB | k::LBRACE => false,
                k::RPAR | k::COMMA | k::COLON | k::QUESTION => true,
                k::KEYWORD => !matches!(p.text, "operator"),
                k::IDENT | k::GREATER | k::GREATER_GREATER | k::STRING | k::NUMBER | k::RSQB => {
                    false
                }
                _ => true,
            };
        }
        _ => {}
    }
    if p.kind == k::ELLIPSIS {
        return is_word(c) || is_literal(c);
    }
    if c.kind == k::ELLIPSIS {
        return false;
    }
    if pa.role == Close && p.kind == k::RBRACE {
        return true;
    }
    if pa.role == Close || pa.role == Cast {
        return !(pa.role == Cast);
    }
    if p.kind == k::TILDE && pa.role == Unary {
        return false;
    }
    let _ = pointer_gap(style);
    true
}

fn penalties(toks: &[Tk], a: &mut [Ann], cx: &Cx) {
    let n = toks.len();
    let style = cx.style;
    // `[[attribute]]` at the start of a line that the source ended there stays alone on its line.
    let attr_end = if n > 4 && toks[0].kind == k::LSQB && toks[1].kind == k::LSQB {
        (2..n - 1)
            .find(|&j| toks[j].kind == k::RSQB && toks[j + 1].kind == k::RSQB)
            .map(|j| j + 2)
    } else {
        None
    };
    for i in 1..n {
        let (p, c) = (&toks[i - 1], &toks[i]);
        let (pa, ca) = (a[i - 1], a[i]);
        use Role::*;
        let mut can = true;
        let mut pen: u32 = 3;
        if c.nl > 0 && c.comment() && i > 0 {
            a[i].must = true;
        }
        if attr_end == Some(i) && c.nl > 0 {
            a[i].must = true;
            a[i].base = true;
        }
        if p.comment() && p.text.starts_with("//") {
            a[i].must = true;
        }
        match ca.role {
            Comma | Semi | Close | Cast | Postfix | TplClose | TplOpen | Label => can = false,
            Open if c.kind == k::LPAR || c.kind == k::LSQB => can = false,
            Open if c.kind == k::LBRACE => {
                can = p.kind == k::EQUAL || matches!(p.kind, k::COMMA | k::LPAR)
            }
            _ => {}
        }
        if c.kind == k::COLON_COLON
            || c.kind == k::ELLIPSIS
            || c.kind == k::COLON && ca.role == Label
        {
            can = false;
        }
        if p.kind == k::COLON_COLON || pa.role == Unary || pa.role == Lambda {
            can = false;
        }
        if p.word("operator") || i >= 2 && toks[i - 2].word("operator") {
            can = false;
        }
        if p.blk && p.kind == k::LBRACE || c.blk && c.kind == k::RBRACE {
            can = false;
        }
        // Binary operators: break after, or before.
        let before_ops = match style.bin_break {
            BinBreak::None => false,
            BinBreak::NonAssignment | BinBreak::All => true,
        };
        if ca.role == Binary && c.kind != k::LESS_LESS {
            can = before_ops;
            pen = ca.prec as u32;
            if (9..=11).contains(&ca.prec) {
                pen = 100;
            }
        }
        if ca.role == Binary && c.kind == k::LESS_LESS {
            can = true;
            pen = 1;
        }
        if pa.role == Binary {
            if p.kind == k::LESS_LESS {
                can = false;
            } else {
                can = !before_ops;
                pen = pa.prec as u32;
                if (9..=11).contains(&pa.prec) {
                    pen = 100;
                }
            }
        }
        if ca.role == Assign && style.bin_break == BinBreak::All {
            can = false;
        }
        if pa.role == Assign {
            pen = 2;
        }
        if ca.role == Question || ca.role == TernColon {
            can = true;
            pen = 3;
        }
        if ca.role == CtorColon {
            can = true;
            pen = 0;
        }
        if pa.role == CtorColon {
            can = false;
        }
        if pa.role == Question || pa.role == TernColon {
            can = false;
        }
        if pa.role == Comma {
            can = true;
            pen = 1;
        }
        if ca.role == Member {
            can = true;
            pen = 100;
        }
        if pa.role == Member {
            can = false;
        }
        if ca.role == Ptr {
            pen = 190;
            can = a[i].level > 0 || cx.decl;
        }
        if pa.role == Open && ca.role != Close && !(p.kind == k::LBRACE && p.blk) {
            can = true;
            pen = 19;
            if p.kind == k::LPAR && cx.decl && param_paren(toks, i - 1, cx) {
                pen = 100;
            }
            if style.align == super::style::BracketAlign::DontAlign {
                pen = 0;
            }
        }
        if pa.role == Open && ca.role == Close {
            can = false;
        }
        // `new (place) T`, `f() MACRO`: a line break there starts a new statement.
        if pa.role == Close && c.kind == k::IDENT {
            can = false;
        }
        if is_word(c)
            && matches!(
                c.text,
                "const" | "noexcept" | "override" | "final" | "volatile"
            )
            && pa.role == Close
            && a[i].level == 0
        {
            pen = 500;
        }
        if ca.role == Close && c.kind == k::RBRACE {
            can = true;
            pen = 1;
        }
        if pa.role == TplOpen {
            can = true;
            pen = 100;
        }
        if ca.role == Arrow || pa.role == Arrow {
            can = ca.role == Arrow;
            pen = 110;
        }
        if pa.role == Cast {
            pen = 100;
        }
        if p.kind == k::STRING && c.kind == k::STRING {
            a[i].must = true;
        }
        // A `<<` after text that ends a line starts a line.
        if c.kind == k::LESS_LESS
            && matches!(p.kind, k::STRING | k::CHAR)
            && (p.text.ends_with("\\n\"") || p.text.ends_with("\\n'"))
        {
            a[i].must = true;
        }
        if is_word(c)
            && (is_word(p) || pa.role == Ptr || pa.role == TplClose)
            && ca.role == Normal
            && pa.role != Binary
        {
            // Between a type and a name: the return type on its own line.
            can = a[i].level == 0 && cx.decl && toks[i..].iter().any(|t| t.kind == k::LPAR);
            // A short return type is not worth a line of its own.
            let before: usize = (0..i)
                .map(|j| toks[j].text.chars().count() + usize::from(j > 0 && a[j].space))
                .sum();
            pen = if before >= 6 {
                cx.style.ret_penalty
            } else {
                cx.style.ret_penalty.max(500)
            };
            if can && cx.style.ret_break != "none" {
                a[i].must = true;
            }
            a[i].base = true;
        }
        if p.kind == k::KEYWORD && matches!(p.text, "class" | "struct" | "union") {
            can = false;
        }
        if pa.role == Comment {
            can = true;
            pen = 1000;
        }
        if ca.role == Comment {
            can = c.nl > 0 || !c.text.starts_with("//");
            pen = 300;
        }
        if is_literal(c) && is_literal(p) {
            pen = 3;
        }
        if pa.role == Semi {
            can = true;
            pen = 0;
        }
        a[i].can = can || a[i].must;
        a[i].pen = pen + 20 * a[i].level;
    }
    // A declaration does not break inside its return type.
    if cx.decl
        && let Some(o) = (0..n).find(|&i| toks[i].kind == k::LPAR && a[i].level == 0)
    {
        for x in a.iter_mut().take(o.saturating_sub(1)) {
            if x.level > 0 && x.can {
                x.pen += 1000;
            }
        }
    }
}
