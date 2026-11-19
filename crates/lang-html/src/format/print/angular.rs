//! Angular's template syntax in the printer: control flow blocks, `@let`, ICU messages, bindings,
//! microsyntax and pipes. Expressions are formatted by the engine as TypeScript (`Opts::embed`);
//! pipes, which are no TypeScript, are split off first (`pipes`).
//!
//! concept: new angular-template-layout
//! what: Angular template syntax: control flow blocks, @let, ICU, bindings.
//! tools: Prettier angular

use super::embed::lines;
use super::microsyntax::{Span, TemplateBinding, template_bindings};
use super::{P, s};
use crate::format::model::{N, T};
use core_format::EmbedMode;
use core_format::doc::{self, Doc};

mod pipes;

/// The blocks that continue a block of this name (Prettier's `ANGULAR_CONTROL_FLOW_BLOCK_WITH_PARAMETERS`):
/// the block does not close with `}` before one of them.
fn continues(name: &str, next: &str) -> bool {
    let chain = ["placeholder", "error", "loading"];
    match name {
        "if" | "else if" => matches!(next, "else" | "else if"),
        "for" => next == "empty",
        "defer" => chain.contains(&next),
        "boundary" => next == "error",
        _ => chain.contains(&name) && chain.contains(&next),
    }
}

/// `item of list` split at the word `of`.
fn split_of(p: &str) -> Option<(&str, &str)> {
    let blank = |c: Option<char>| c.is_some_and(char::is_whitespace);
    p.match_indices("of").find_map(|(i, _)| {
        let (before, after) = (&p[..i], &p[i + 2..]);
        (blank(before.chars().next_back()) && blank(after.chars().next()))
            .then_some((before, after))
    })
}

/// `a   b` as `a b`.
fn collapse(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Where an expression stands in the template: the whole value of a binding (the operands of a
/// binary are not indented), part of a value, or the statements of an event handler.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Ng {
    Binding,
    Nested,
    Statements,
}

impl P<'_> {
    /// An expression formatted for `width`; `Ng::Statements` reads `a; b` (an event handler).
    pub(super) fn ng_expression(
        &self,
        code: &str,
        width: usize,
        single_quotes: bool,
        kind: Ng,
    ) -> Option<String> {
        let embed = self.o.embed?;
        let mode = match kind {
            Ng::Binding => EmbedMode::NgExpression,
            Ng::Nested | Ng::Statements => EmbedMode::NgNested,
        };
        let fmt = |c: &str| embed("ts", mode, c, width, single_quotes);
        if kind == Ng::Statements {
            let parts = pipes::split_statements(code);
            let out: Option<Vec<String>> = parts
                .iter()
                .map(|p| {
                    let out = pipes::format(p, &fmt)?;
                    Some(if pipes::has_side_effect(p) {
                        out
                    } else {
                        format!("({out})")
                    })
                })
                .collect();
            let out = out?;
            let flat = out.join("; ");
            return Some(if flat.len() <= width && !flat.contains('\n') {
                flat
            } else {
                out.join(";\n")
            });
        }
        pipes::format(code, &fmt)
    }

    /// `name="` formatted value `"` for the binding `name`; `None` keeps the attribute as written.
    pub(super) fn angular_attribute(&self, id: usize, name: &str, value: &str) -> Option<Doc> {
        let v = super::unescape_quotes(value);
        let (open, close) = (format!("{name}=\""), "\"");
        if name.starts_with('(') || name.starts_with("on-") {
            let f = |w: usize| self.ng_expression(&v, w, true, Ng::Statements);
            return self.ng_attr(id, &open, close, &f);
        }
        if name.starts_with(['[']) || name.starts_with("bind-") || name.starts_with("bindon-") {
            let f = |w: usize| self.ng_expression(&v, w, true, Ng::Binding);
            return self.ng_attr(id, &open, close, &f);
        }
        if name.starts_with('*') {
            return self.microsyntax_attribute(id, &open, &v);
        }
        if name == "i18n" || name.starts_with("i18n-") {
            let words = collapse(&v);
            return Some(s(format!("{name}=\"{}\"", words.replace('"', "&quot;"))));
        }
        if v.contains("{{") {
            return self.attribute_interpolation(id, &open, &v);
        }
        None
    }

    fn ng_attr(
        &self,
        id: usize,
        open: &str,
        close: &str,
        run: &dyn Fn(usize) -> Option<String>,
    ) -> Option<Doc> {
        let run = |w: usize| run(w).map(|o| (o, false));
        self.attr_expression(id, open, close, &|o| o.replace('"', "&quot;"), &run)
    }

    /// `Hi {{name}}!`: the text kept, each interpolation a group that breaks inside its braces.
    fn attribute_interpolation(&self, id: usize, open: &str, v: &str) -> Option<Doc> {
        let width = self
            .o
            .width
            .saturating_sub((self.depth(id) + 2) * self.o.tab);
        let quoted = |t: &str| {
            doc::join(
                &doc::hardline(),
                t.split('\n').map(|l| s(l.replace('"', "&quot;"))),
            )
        };
        let mut parts = Vec::new();
        let mut rest = v;
        while let Some(i) = rest.find("{{") {
            let end = rest[i + 2..].find("}}")? + i + 2;
            parts.push(quoted(&rest[..i]));
            let code = rest[i + 2..end].trim();
            let body = if code.is_empty() {
                doc::line()
            } else {
                let out = self.ng_expression(code, width, true, Ng::Nested)?;
                doc::concat(vec![doc::line(), quoted(&out)])
            };
            parts.push(doc::group(doc::concat(vec![
                s("{{"),
                doc::indent(body),
                doc::line(),
                s("}}"),
            ])));
            rest = &rest[end + 2..];
        }
        parts.push(quoted(rest));
        Some(doc::concat(vec![
            s(open),
            doc::group(doc::concat(parts)),
            s("\""),
        ]))
    }

    /// The value of a structural directive (`let item of items; trackBy: f`), as Prettier prints
    /// its microsyntax; read with the parser's own reading of a start tag.
    fn microsyntax(&self, value: &str, width: usize) -> Option<String> {
        let src = value;
        let bindings = template_bindings(src)?;
        let text = |sp: Span| &src[sp.start..sp.end];
        // (the operands of a binary are indented unless the value is one expression)
        let kind = if bindings.len() > 2 {
            Ng::Nested
        } else {
            Ng::Binding
        };
        let expr = |sp| self.ng_expression(text(sp), width, true, kind);
        let mut out = String::new();
        let (mut first_is_expression, mut previous_key): (bool, Option<&str>) = (false, None);
        // The directive's own empty first binding is not printed; an `as` that names the expression
        // before it is part of it.
        let items = bindings.iter().filter(|b| {
            !matches!(
                b,
                TemplateBinding::Expression {
                    key: None,
                    expression: None
                }
            )
        });
        let mut i = 0;
        for b in items {
            let alias = matches!(b, TemplateBinding::As { key: None, .. });
            if !alias {
                i += 1;
            }
            let i = i - 1;
            let mut piece = String::new();
            match b {
                TemplateBinding::Expression { key, expression } => {
                    let key = key.map(text);
                    if let Some(k) = key {
                        piece.push_str(k);
                    }
                    if let Some(e) = expression {
                        let colon = key.is_some() && {
                            let k = key.unwrap_or("");
                            !((i == 1 && k == "of")
                                || (first_is_expression
                                    && ((i == 1 && matches!(k, "then" | "else" | "as"))
                                        || ((i == 2 || i == 3)
                                            && (k == "track"
                                                || (k == "else"
                                                    && previous_key == Some("then")))))))
                        };
                        if key.is_some() {
                            piece.push_str(if colon { ": " } else { " " });
                        }
                        piece.push_str(&expr(*e)?);
                    }
                    if i == 0 {
                        first_is_expression = key.is_none();
                    }
                    previous_key = key;
                }
                TemplateBinding::Let { name, value } => {
                    piece.push_str(&format!("let {}", text(*name)));
                    if let Some(v) = value {
                        piece.push_str(&format!(" = {}", text(*v)));
                    }
                    previous_key = None;
                }
                TemplateBinding::As { key, alias } => {
                    if let Some(k) = key {
                        piece.push_str(&format!("{} ", text(*k)));
                    }
                    piece.push_str(&format!("as {}", text(*alias)));
                }
            }
            if alias {
                out.push(' ');
            } else if i > 0 {
                let of = matches!(b, TemplateBinding::Expression { key: Some(k), .. } if i == 1 && text(*k) == "of");
                out.push(if of { ' ' } else { '\u{1}' });
            }
            out.push_str(&piece);
        }
        Some(out)
    }

    /// `name="` the microsyntax `value` `"`: on one line when it fits, else a binding per line.
    fn microsyntax_attribute(&self, id: usize, open: &str, value: &str) -> Option<Doc> {
        let indent = (self.depth(id) + 1) * self.o.tab;
        let flat_width = self.o.width.saturating_sub(indent + open.len() + 1);
        let flat = self.microsyntax(value, flat_width)?.replace('\u{1}', "; ");
        if flat.len() <= flat_width && !flat.contains('\n') {
            return Some(s(format!("{open}{flat}\"")));
        }
        let broken = self.microsyntax(value, self.o.width.saturating_sub(indent + self.o.tab))?;
        let pieces = broken
            .split('\u{1}')
            .map(|p| lines(&p.replace('"', "&quot;")));
        Some(doc::concat(vec![
            s(open),
            doc::indent(doc::concat(vec![
                doc::hardline(),
                doc::join(&doc::concat(vec![s(";"), doc::hardline()]), pieces),
            ])),
            doc::hardline(),
            s("\""),
        ]))
    }

    /// The parameters of a block: expressions of `if`, `else if`, `for`, `switch` and `case`
    /// formatted, the rest of each parameter with its blanks collapsed.
    fn block_parameters(&self, n: &N<'_>, width: usize) -> Vec<String> {
        let name = collapse(n.name.trim_start_matches('@'));
        let expr = |code: &str| {
            self.ng_expression(code.trim(), width, false, Ng::Binding)
                .unwrap_or_else(|| collapse(code))
        };
        let mut out = Vec::new();
        for (i, p) in n.params.iter().enumerate() {
            let p = p.trim();
            out.push(match (name.as_str(), i) {
                ("if" | "else if" | "switch" | "case", 0) => expr(p),
                ("for", _) => {
                    if let Some(rest) = p
                        .strip_prefix("track")
                        .filter(|r| r.starts_with(char::is_whitespace))
                    {
                        format!("track {}", expr(rest))
                    } else if let Some((item, list)) = split_of(p) {
                        format!("{} of {}", collapse(item), expr(list))
                    } else {
                        collapse(p)
                    }
                }
                _ => collapse(p),
            });
        }
        out
    }

    fn block_name(n: &N<'_>) -> String {
        collapse(n.name.trim_start_matches('@'))
    }

    /// Does the block print its `}`? Not when a block that continues it follows.
    fn closes(&self, id: usize) -> bool {
        let n = self.n(id);
        // (a block kept as written has its own `}`, and one kept as written takes no `} `)
        self.has_ignore(id)
            || !n.next.is_some_and(|x| {
                self.n(x).t == T::NgBlock
                    && !self.has_ignore(x)
                    && continues(&Self::block_name(n), &Self::block_name(self.n(x)))
            })
    }

    /// `@name (parameters) {`, the content indented, `}`; the `}` of a block another follows
    /// moves to that block's line.
    pub(super) fn ng_block_doc(&self, id: usize) -> Doc {
        let n = self.n(id);
        let mut head = String::new();
        if let Some(p) = n.prev
            && self.n(p).t == T::NgBlock
            && !self.closes(p)
        {
            head.push_str("} ");
        }
        head.push('@');
        head.push_str(&Self::block_name(n));
        let mut parts = vec![s(head)];
        if !n.params.is_empty() {
            let width = self
                .o
                .width
                .saturating_sub((self.depth(id) + 1) * self.o.tab);
            let pieces = self
                .block_parameters(n, width)
                .into_iter()
                .map(|p| lines(&p));
            // (`@default never(x);` is one word to Prettier)
            parts.push(s(if Self::block_name(n) == "default never" {
                "("
            } else {
                " ("
            }));
            parts.push(doc::group(doc::concat(vec![
                doc::indent(doc::concat(vec![
                    doc::softline(),
                    doc::join(&doc::concat(vec![s(";"), doc::line()]), pieces),
                ])),
                doc::softline(),
            ])));
            parts.push(s(")"));
        }
        if n.self_closing {
            // A `@case` that falls through to the next one, or `@default never;`.
            if Self::block_name(n) == "default never" {
                parts.push(s(";"));
            }
            return doc::concat(parts);
        }
        parts.push(s(" {"));
        let close = self.closes(id);
        if n.kids.is_empty() {
            if close {
                parts.push(s("}"));
            }
        } else {
            parts.push(doc::indent(doc::concat(vec![
                doc::hardline(),
                self.children(id),
            ])));
            if close {
                parts.extend([doc::hardline(), s("}")]);
            }
        }
        doc::group(doc::concat(parts))
    }

    /// `@let name = value;`
    pub(super) fn ng_let_doc(&self, id: usize) -> Doc {
        let n = self.n(id);
        let head = format!("@let {} =", n.name.trim());
        let indent = self.depth(id) * self.o.tab;
        let code = n.value.trim();
        let flat = self.ng_expression(
            code,
            self.o.width.saturating_sub(indent + head.len() + 2),
            false,
            Ng::Binding,
        );
        match flat {
            Some(v) if !v.contains('\n') => s(format!("{head} {v};")),
            Some(_) => {
                let broken = self
                    .ng_expression(
                        code,
                        self.o.width.saturating_sub(indent + self.o.tab + 1),
                        false,
                        Ng::Binding,
                    )
                    .unwrap_or_default();
                let lines = lines(&broken);
                doc::concat(vec![
                    s(head),
                    doc::indent(doc::concat(vec![doc::hardline(), lines])),
                    s(";"),
                ])
            }
            None => s(format!("{head} {code};")),
        }
    }

    /// `{value, type, =0 {one} other {many}}`
    pub(super) fn icu_doc(&self, id: usize) -> Doc {
        let n = self.n(id);
        let mut head = vec![s(format!("{}, {}", n.value.trim(), n.name.trim()))];
        if !n.kids.is_empty() {
            let cases: Vec<Doc> = n.kids.iter().map(|&c| self.icu_case(c)).collect();
            head.push(s(","));
            head.push(doc::indent(doc::concat(vec![
                doc::line(),
                doc::join(&doc::line(), cases),
            ])));
        }
        head.push(doc::softline());
        doc::concat(vec![
            s(self.open_start(id)),
            doc::group(doc::concat(head)),
            s(self.close_end(id)),
        ])
    }

    fn icu_case(&self, id: usize) -> Doc {
        let n = self.n(id);
        let mut body = Vec::new();
        for (i, &c) in n.kids.iter().enumerate() {
            let k = self.n(c);
            if k.t == T::Text && k.lead_sp {
                // The case's own softline starts a broken body: a second line break would add a blank line.
                body.push(if i == 0 {
                    doc::if_break(doc::nil(), doc::line())
                } else {
                    doc::line()
                });
            }
            // An interpolation in a case stays as written.
            body.push(if k.t == T::Interp {
                s(&self.t.src[k.start..k.end])
            } else {
                self.print_child(c)
            });
            if k.t == T::Text && k.trail_sp && i + 1 < n.kids.len() {
                body.push(doc::line());
            }
        }
        doc::concat(vec![
            s(format!("{} {{", n.value.trim())),
            doc::group(doc::concat(vec![
                doc::indent(doc::concat(vec![doc::softline(), doc::concat(body)])),
                doc::softline(),
            ])),
            s("}"),
        ])
    }
}
