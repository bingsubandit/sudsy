//! The `regex` family: rules about the source of a regular expression, written as a literal (a
//! token with a `regex` row in the grammar) or as the first argument of a call (`regex-call`
//! rows). The pattern is read by [`parse`] with the rules of the row's flavor ([`flavor`]); a
//! string argument is read through the string model of the grammar, and with a language's scope
//! table the names of variables written once are followed. Checks (param `check`):
//! - `control`: a control character (below U+0020) matched by its code, as `\x1f`;
//! - `spaces`: two or more spaces outside a class (a string literal argument only);
//! - `empty-class`: `[]` in a literal, in dialects where it is a class that matches nothing;
//! - `misleading-class`: a combining mark after another character in a class;
//! - `useless-escape`: a backslash that escapes nothing special, in a literal's pattern and in
//!   the strings of nodes tagged `#quoted` (the grammar's `string` rows say what their escapes
//!   are; a node that is the last child of a `#raw-strings` node, such as a JSX attribute, has
//!   none).
//!
//! The message has `{name}`.
//!
//! concept: regex-source-checks
//! what: Rules about the source of regular expressions in literals and call arguments, by flavor.
//! tools: eslint no-control-regex no-regex-spaces no-empty-character-class no-misleading-character-class, ruff regex checks
//! reads: grammar:regex*

mod flavor;
mod parse;

use self::flavor::Flavor;
use self::parse::{Item, Kind, Member, Regex};
use crate::scope_rules::{self, ScopeLang};
use crate::tree::significant_range;
use crate::{Family, Params, Rule, RuleContext, RuleMeta};
use core_semantics::scopes::Flags;
use core_syntax::first_code_token;
use core_syntax::regex::RegexCall;
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, SyntaxToken, TextRange, TextSize};
use std::sync::OnceLock;

/// The family of a language with no scope table: names are not followed.
pub const FAMILY: Family = Family {
    name: "regex",
    build: |meta, p| build(None, meta, p),
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Check {
    Control,
    Spaces,
    EmptyClass,
    MisleadingClass,
    UselessEscape,
}

struct RegexRule {
    meta: &'static RuleMeta,
    check: Check,
    message: String,
    tags: &'static [&'static str],
    lang: Option<&'static ScopeLang>,
}

/// The `build` of the family over `lang`, the scope table whose model follows names.
pub fn build(
    lang: Option<&'static ScopeLang>,
    meta: &'static RuleMeta,
    mut p: Params,
) -> Result<Box<dyn Rule>, String> {
    let check = p.choice(
        "check",
        true,
        &[
            ("control", Check::Control),
            ("spaces", Check::Spaces),
            ("empty-class", Check::EmptyClass),
            ("misleading-class", Check::MisleadingClass),
            ("useless-escape", Check::UselessEscape),
        ],
    )?;
    let tags: &'static [&'static str] = match check {
        Check::EmptyClass => &["regex"],
        Check::UselessEscape => &["regex", "quoted"],
        _ => &["regex", "call"],
    };
    let rule = RegexRule {
        meta,
        check,
        message: p.text("message")?,
        tags,
        lang,
    };
    p.done()?;
    Ok(Box::new(rule))
}

/// A pattern and where it came from.
struct Source {
    pattern: String,
    flavor: &'static Flavor,
    /// The offset of the pattern in the file when it is written there as it is (a literal).
    at: Option<usize>,
}

impl Rule for RegexRule {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn bodies(&self) -> bool {
        false
    }

    fn tags(&self) -> &[&'static str] {
        self.tags
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        if self.check == Check::UselessEscape && g.has_tag(node.kind(), "quoted") {
            return self.useless_in_strings(node, ctx);
        }
        let Some(source) = self.source(node, ctx) else {
            return;
        };
        if self.check == Check::EmptyClass && source.at.is_none() {
            return;
        }
        let regex = parse::parse(&source.pattern, source.flavor);
        let range = significant_range(node, g);
        match self.check {
            Check::Control => {
                let found = controls(&regex.items);
                if !found.is_empty() {
                    self.hit(ctx, range, &found.join(", "));
                }
            }
            Check::Spaces => {
                for len in space_runs(&regex.items) {
                    self.hit(ctx, range, &len.to_string());
                }
            }
            Check::EmptyClass => {
                let empty = |i: &Item| matches!(&i.kind, Kind::Class(c) if !c.negated && c.members.is_empty());
                if source.flavor.empty_class && regex.items.iter().any(empty) {
                    self.hit(ctx, range, "");
                }
            }
            Check::MisleadingClass => {
                if misleading(&regex) {
                    self.hit(ctx, range, "");
                }
            }
            Check::UselessEscape => {
                let at = source.at.unwrap_or(0);
                for e in regex.escapes.iter().filter(|e| source.flavor.useless(e)) {
                    let start = TextSize::from((at + e.start) as u32);
                    let len = TextSize::from(1 + e.ch.len_utf8() as u32);
                    self.hit(ctx, TextRange::at(start, len), &e.ch.to_string());
                }
            }
        }
    }
}

impl RegexRule {
    fn hit(&self, ctx: &mut RuleContext<'_>, range: TextRange, name: &str) {
        ctx.report(range, self.message.replace("{name}", name));
    }

    /// The pattern of a regex literal, or of a call that takes one.
    fn source(&self, node: &SyntaxNode, ctx: &RuleContext<'_>) -> Option<Source> {
        let g = ctx.file.grammar;
        if g.has_tag(node.kind(), "regex") {
            let token = first_code_token(node, g)?;
            let row = g.regex_row(token.kind())?;
            let text = token.text();
            let body = row.pattern(text)?;
            return Some(Source {
                pattern: body.to_string(),
                flavor: flavor::named(&row.flavor)?,
                at: Some(usize::from(token.text_range().start()) + row.quote.len_utf8()),
            });
        }
        if self.check == Check::UselessEscape || g.regex_calls().is_empty() {
            return None;
        }
        // A method call's text starts with its object (`Pattern.compile`).
        let callee = g.slot(node, "callee")?;
        let object = g.slot(node, "object");
        let head = object.clone().unwrap_or_else(|| callee.clone());
        let first = first_token(&head, g)?;
        if !g.regex_calls().iter().any(|r| {
            r.callees
                .iter()
                .any(|c| c.split('.').next() == Some(first.text()))
        }) {
            return None;
        }
        let mut text: String = element_tokens(&head, g)
            .map(|t| t.text().to_owned())
            .collect();
        if object.is_some() {
            text.push('.');
            text.extend(element_tokens(&callee, g).map(|t| t.text().to_owned()));
        }
        let row = g.regex_calls().iter().find(|r| r.callees.contains(&text))?;
        let args = g.slot(node, "arguments")?;
        let list = args.as_node().filter(|n| g.has_tag(n.kind(), "arguments"));
        let arg = match list {
            Some(list) => list
                .children_with_tokens()
                .filter(|e| {
                    e.as_token().is_none_or(|t| {
                        !g.is_trivia(t.kind()) && !g.has_tag(t.kind(), "punctuation")
                    })
                })
                .nth(row.arg)?,
            None if row.arg == 0 => args,
            None => return None,
        };
        let follow = self.check != Check::Spaces;
        let eval = Eval {
            ctx,
            row,
            lang: self.lang.filter(|_| follow),
            full: follow,
        };
        let value = eval.constant(&arg, 0)?;
        let pattern = if row.delimited {
            core_syntax::regex::undelimit(&value)?.to_string()
        } else {
            value
        };
        Some(Source {
            pattern,
            flavor: flavor::named(&row.flavor)?,
            at: None,
        })
    }

    /// `no-useless-escape` on the strings of a node: every own token with a `string` row.
    fn useless_in_strings(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        if let Some(parent) = node.parent()
            && g.has_tag(parent.kind(), "raw-strings")
            && parent.children().last().as_ref() == Some(node)
        {
            return;
        }
        for token in node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| g.has_tag(t.kind(), "string"))
        {
            let text = token.text();
            for row in g.string_rows(token.kind()) {
                let Some(lit) = row.split(text).filter(|_| row.matches(text)) else {
                    continue;
                };
                let quote = row
                    .quotes
                    .iter()
                    .copied()
                    .find(|q| lit.open.starts_with(*q))
                    .or(row.quotes.first().copied())
                    .unwrap_or('\0');
                let base =
                    usize::from(token.text_range().start()) + lit.prefix.len() + lit.open.len();
                row.unknown_escapes(lit.inner, quote, |pos, ch| {
                    let start = TextSize::from((base + pos) as u32);
                    let len = TextSize::from(1 + ch.len_utf8() as u32);
                    self.hit(ctx, TextRange::at(start, len), &ch.to_string());
                });
                break;
            }
        }
    }
}

/// The first code token of an element, without collecting the others.
fn first_token(e: &SyntaxElement, g: &Grammar) -> Option<SyntaxToken> {
    match e {
        SyntaxElement::Token(t) => Some(t.clone()).filter(|t| !g.is_trivia(t.kind())),
        SyntaxElement::Node(n) => first_code_token(n, g),
    }
}

/// The code tokens of an element: the token itself, or those below a node.
fn element_tokens(e: &SyntaxElement, g: &Grammar) -> impl Iterator<Item = SyntaxToken> {
    let tokens: Vec<SyntaxToken> = match e {
        SyntaxElement::Token(t) => vec![t.clone()],
        SyntaxElement::Node(n) => n
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .collect(),
    };
    tokens.into_iter().filter(|t| !g.is_trivia(t.kind()))
}

/// Reads an argument as the string it always is.
struct Eval<'a, 'b> {
    ctx: &'a RuleContext<'b>,
    row: &'a RegexCall,
    lang: Option<&'static ScopeLang>,
    /// Concatenations, parentheses and names too, not only a literal.
    full: bool,
}

impl Eval<'_, '_> {
    fn constant(&self, e: &SyntaxElement, depth: u8) -> Option<String> {
        let file = self.ctx.file;
        let g = file.grammar;
        if depth > 16 {
            return None;
        }
        let n = match e {
            SyntaxElement::Token(t) if g.has_tag(t.kind(), "string") => {
                return g.string_value(t.kind(), t.text()).map(|v| v.into_owned());
            }
            SyntaxElement::Token(_) => return None,
            SyntaxElement::Node(n) => n,
        };
        let mut tokens = n
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !g.is_trivia(t.kind()));
        let (first, second) = (tokens.next(), tokens.next());
        if let (Some(t), None) = (&first, &second)
            && g.has_tag(t.kind(), "string")
            && n.children().next().is_none()
        {
            return g.string_value(t.kind(), t.text()).map(|v| v.into_owned());
        }
        // Strings written one after the other are one (`"a" "b"`).
        if first.is_none() && n.children().next().is_some() {
            return n
                .children()
                .map(|c| self.constant(&c.into(), depth + 1))
                .collect();
        }
        if !self.full {
            return None;
        }
        let tag = |t: &str| g.has_tag(n.kind(), t);
        if tag("parenthesized") {
            return self.constant(&n.children().next()?.into(), depth + 1);
        }
        if tag("binary") {
            let op = g.slot(n, "operator")?.into_token()?;
            if self.row.concat.as_deref() != Some(op.text()) {
                return None;
            }
            let left = self.constant(&g.slot(n, "left")?, depth + 1)?;
            let right = self.constant(&g.slot(n, "right")?, depth + 1)?;
            return Some(left + &right);
        }
        if tag("reference") {
            let model = scope_rules::model(self.lang?, file);
            let start = significant_range(n, g).start();
            let i = model
                .refs
                .binary_search_by_key(&start, |r| r.range.start())
                .ok()?;
            let symbol = model.refs[i].symbol?;
            let [decl] = model.symbol(symbol).decls.as_slice() else {
                return None;
            };
            let once = decl.flags.contains(Flags::CONST)
                || (decl.flags.contains(Flags::VAR)
                    && !model.refs_of(symbol).any(|w| w.is_write()));
            let init = g.slot(&decl.node(file.root), "init")?;
            return once.then(|| self.constant(&init, depth + 1))?;
        }
        None
    }
}

/// The control characters a pattern matches by their code, as `\x1f`.
fn controls(items: &[Item]) -> Vec<String> {
    let mut found = Vec::new();
    let mut add = |cp: u32| {
        if cp < 0x20 {
            found.push(format!("\\x{cp:02x}"));
        }
    };
    for item in items {
        match &item.kind {
            Kind::Char(cp) => add(*cp),
            Kind::Class(c) => {
                for m in &c.members {
                    match m {
                        Member::Char(cp) => add(*cp),
                        Member::Range(from, to) => {
                            add(*from);
                            to.iter().for_each(|t| add(*t));
                        }
                        Member::Set => {}
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// Runs of two or more spaces outside classes, as the length to report: a run followed by a
/// quantifier gives its last space to it.
fn space_runs(items: &[Item]) -> Vec<usize> {
    let (mut runs, mut i) = (Vec::new(), 0);
    let space = |i: usize| matches!(items.get(i).map(|x| &x.kind), Some(Kind::Char(0x20)));
    while i < items.len() {
        if !space(i) {
            i += 1;
            continue;
        }
        let len = (i..).take_while(|j| space(*j)).count();
        let quantified = matches!(
            items.get(i + len).map(|x| &x.kind),
            Some(Kind::Meta('+' | '*' | '{' | '?'))
        );
        if len >= 2 && !quantified {
            runs.push(len);
        } else if len >= 3 {
            runs.push(len - 1);
        }
        i += len;
    }
    runs
}

/// The code points of `\p{M}`.
fn marks() -> &'static [(u32, u32)] {
    static MARKS: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
    MARKS.get_or_init(|| {
        use regex_syntax::hir::{Class, HirKind};
        match regex_syntax::parse(r"\p{M}").map(|h| h.into_kind()) {
            Ok(HirKind::Class(Class::Unicode(u))) => u
                .iter()
                .map(|r| (r.start() as u32, r.end() as u32))
                .collect(),
            _ => Vec::new(),
        }
    })
}

fn combining(cp: u32) -> bool {
    let table = marks();
    table
        .binary_search_by(|&(lo, hi)| {
            if cp < lo {
                std::cmp::Ordering::Greater
            } else if cp > hi {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn sequence_combined(seq: &[u32]) -> bool {
    seq.windows(2).any(|w| combining(w[1]) && !combining(w[0]))
}

/// Does a class have a combining mark after a character that is not one (the sequences ESLint
/// reads: a range ends the one before it and its last character starts the next)?
fn misleading(regex: &Regex) -> bool {
    regex.items.iter().any(|item| {
        let Kind::Class(class) = &item.kind else {
            return false;
        };
        let mut seq: Vec<u32> = Vec::new();
        for m in &class.members {
            match m {
                Member::Char(cp) => seq.push(*cp),
                Member::Set => seq.clear(),
                Member::Range(from, to) => {
                    seq.push(*from);
                    if sequence_combined(&seq) {
                        return true;
                    }
                    seq = to.iter().copied().collect();
                }
            }
        }
        sequence_combined(&seq)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(p: &str) -> Vec<Item> {
        parse::parse(p, flavor::named("ecmascript").unwrap()).items
    }

    #[test]
    fn control_characters() {
        assert_eq!(controls(&items(r"\x1f[\u{0}-a]")), ["\\x1f", "\\x00"]);
        assert!(controls(&items(r"\t\n\x20")).is_empty());
    }

    #[test]
    fn space_runs_leave_the_quantified_space() {
        assert_eq!(space_runs(&items("a  b")), [2]);
        assert!(space_runs(&items("a  +b")).is_empty());
        assert_eq!(space_runs(&items("a   +b")), [2]);
        assert!(space_runs(&items("[  ]")).is_empty());
    }

    #[test]
    fn combining_marks_in_classes() {
        let m = |p: &str| misleading(&parse::parse(p, flavor::named("ecmascript").unwrap()));
        assert!(m("[A\\u0301]"));
        assert!(!m("[\\u0301]"));
        assert!(!m("[A\\d\\u0301]"));
    }

    #[test]
    fn every_flavor_a_grammar_names_exists() {
        let g = core_syntax::grammars::JS.get();
        for call in g.regex_calls() {
            assert!(flavor::named(&call.flavor).is_some(), "{}", call.flavor);
        }
        let row = g.regex_row(g.kind_by_name("RegExp").unwrap()).unwrap();
        assert!(flavor::named(&row.flavor).is_some());
    }
}
