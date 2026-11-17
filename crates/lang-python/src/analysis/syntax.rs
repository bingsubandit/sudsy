//! Checks that read the syntax only, run by the walk of `checker` where the nodes are met.

use super::model::Model;
use ruff_python_ast::token::{Token, TokenKind};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange, TextSize};

/// E741: `l`, `O` and `I` look like digits.
pub fn ambiguous_name(m: &mut Model<'_>, name: &str, range: TextRange) {
    if matches!(name, "l" | "O" | "I") {
        m.hit("ambiguous-variable-name", range, name);
    }
}

/// E742 and E743: a class or a function named like that.
pub fn ambiguous_def(m: &mut Model<'_>, name: &str, range: TextRange, check: &'static str) {
    if matches!(name, "l" | "O" | "I") {
        m.hit(check, range, name);
    }
}

/// E701: the body of a compound statement starts on the line of its colon, unless it is `...`.
pub fn compound_header(m: &mut Model<'_>, src: &str, body: &[Stmt]) {
    let Some(first) = body.first() else {
        return;
    };
    if matches!(first, Stmt::Expr(e) if matches!(*e.value, Expr::EllipsisLiteral(_)))
        && body.len() == 1
    {
        return;
    }
    let before = src[..first.start().to_usize()].trim_end_matches([' ', '\t']);
    if before.ends_with(':') {
        let at = TextSize::try_from(before.len() - 1).unwrap_or_default();
        m.hit(
            "multiple-statements-on-one-line-colon",
            TextRange::at(at, TextSize::new(1)),
            "",
        );
    }
}

/// E702 and E703: a semicolon between statements and one that ends a line.
pub fn semicolons(m: &mut Model<'_>, tokens: &[Token]) {
    for (i, t) in tokens.iter().enumerate() {
        if t.kind() != TokenKind::Semi {
            continue;
        }
        let next = tokens[i + 1..]
            .iter()
            .map(Token::kind)
            .find(|k| *k != TokenKind::Comment);
        let ends = matches!(
            next,
            None | Some(TokenKind::Newline | TokenKind::NonLogicalNewline | TokenKind::EndOfFile)
        );
        let check = if ends {
            "useless-semicolon"
        } else {
            "multiple-statements-on-one-line-semicolon"
        };
        m.hit(check, t.range(), "");
    }
}

/// W605: a backslash in a string that is not raw and starts no escape sequence.
pub fn invalid_escapes(m: &mut Model<'_>, src: &str, tokens: &[Token]) {
    for t in tokens {
        if !matches!(
            t.kind(),
            TokenKind::String | TokenKind::FStringMiddle | TokenKind::TStringMiddle
        ) || t.string_flags().is_none_or(|f| f.is_raw_string())
        {
            continue;
        }
        let range = t.range();
        let mut chars = src[range].char_indices();
        while let Some((i, c)) = chars.next() {
            if c != '\\' {
                continue;
            }
            // The backslash ends the piece of an f-string: the replacement field's brace follows.
            let next = chars.next().map_or('{', |(_, n)| n);
            if !matches!(
                next,
                '\n' | '\r' | '\\' | '\'' | '"' | 'a' | 'b' | 'f' | 'n' | 'r' | 't' | 'v' | '0'
                    ..='7' | 'x' | 'N' | 'u' | 'U'
            ) {
                let at = range.start() + TextSize::try_from(i).unwrap_or_default();
                let len = TextSize::try_from(1 + next.len_utf8()).unwrap_or_default();
                m.hit(
                    "invalid-escape-sequence",
                    TextRange::at(at, len),
                    format!("\\{next}"),
                );
            }
        }
    }
}

/// The dotted path of a chain of names and attributes.
fn dotted(e: &Expr) -> Option<String> {
    match e {
        Expr::Name(n) => Some(n.id.to_string()),
        Expr::Attribute(a) => dotted(&a.value).map(|p| format!("{p}.{}", a.attr)),
        _ => None,
    }
}

/// A statement that sets the module up before the imports (`sys.path.insert(...)`,
/// `os.environ[...] = ...`, `matplotlib.use(...)`, `pytest.importorskip(...)`).
fn is_setup(stmt: &Stmt) -> bool {
    let call_path = |e: &Expr| match e {
        Expr::Call(c) => dotted(&c.func),
        _ => None,
    };
    match stmt {
        Stmt::Expr(e) => call_path(&e.value).is_some_and(|p| {
            p.starts_with("sys.path.")
                || p.starts_with("os.environ.")
                || p == "matplotlib.use"
                || p == "pytest.importorskip"
        }),
        Stmt::Assign(a) => {
            call_path(&a.value).is_some_and(|p| p == "pytest.importorskip")
                || a.targets.iter().any(|t| {
                    matches!(t, Expr::Subscript(s) if dotted(&s.value).is_some_and(|p| p == "os.environ"))
                })
        }
        _ => false,
    }
}

fn is_dunder_assignment(stmt: &Stmt) -> bool {
    let dunder =
        |e: &Expr| matches!(e, Expr::Name(n) if n.id.starts_with("__") && n.id.ends_with("__"));
    match stmt {
        Stmt::Assign(a) => a.targets.len() == 1 && dunder(&a.targets[0]),
        Stmt::AnnAssign(a) => dunder(&a.target),
        _ => false,
    }
}

/// E402: an import of the module after code (a docstring, `__all__`-style assignments, blocks and
/// setup of `sys.path` or `os.environ` are not code).
pub fn module_imports(m: &mut Model<'_>, body: &[Stmt]) {
    let mut code = false;
    for (i, stmt) in body.iter().enumerate() {
        match stmt {
            Stmt::Import(_) | Stmt::ImportFrom(_) => {
                if code {
                    m.hit("module-import-not-at-top-of-file", stmt.range(), "");
                }
            }
            Stmt::Expr(e) if i == 0 && matches!(*e.value, Expr::StringLiteral(_)) => {}
            Stmt::If(_) | Stmt::Try(_) | Stmt::With(_) | Stmt::Match(_) => {}
            _ => code |= !(is_dunder_assignment(stmt) || is_setup(stmt)),
        }
    }
}
