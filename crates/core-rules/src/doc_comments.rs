//! Doc comments: the `/** ... */` block comment before a declaration ([`leading_doc`]) and the
//! family `missing-doc-comment`, which reports a visible declaration without one. Params:
//! - `<group>`: declaration rows ([`crate::decl_rows`]) of what needs a doc comment (a group of
//!   rows has a name of its own; `outer.<group>` lists the modifier keywords of which each
//!   enclosing type declaration must have one, empty: no condition);
//! - `visible`: the modifier keywords that make a declaration visible (a declaration with none of
//!   them is not reported), `implicit-public-in`: the keywords of the type declarations whose
//!   members are visible without a modifier (Java `interface @interface`);
//! - `getters` and `setters`: name prefixes of property accessors (`get is`, `set`), which the
//!   option `property_accessors` leaves out: a getter has no parameter and one `#return`
//!   statement, a setter one parameter and one expression statement;
//! - `message` with `{name}`.
//!
//! A declaration inside a function or an object creation is not reported. The option
//! `allowed_annotations` (a list of names, `*` matches any run of characters) leaves out a
//! declaration with one of them. The report is at the first token of the declaration.
//!
//! concept: missing-doc-comment
//! what: A visible declaration has a doc comment before it.
//! tools: Checkstyle MissingJavadocMethod, MissingJavadocType
//! rules: missing-doc-comment
//! reads: family:missing-doc-comment, tag:modifiers, tag:annotation, tag:parameter-list

use crate::decl_rows::{Row, annotation_names, glob, has_modifier};
use crate::slot_path::SlotPath;
use crate::tree::is_code;
use crate::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

pub const FAMILY: Family = Family {
    name: "missing-doc-comment",
    build,
};

/// The doc comment (a block comment that starts with the opening delimiter and one more of its
/// last character: `/**`) among the comments before the first code token of `node`.
pub fn leading_doc(node: &SyntaxNode, g: &Grammar) -> Option<SyntaxToken> {
    let (open, close) = g.comments.block.as_ref()?;
    let last = open.chars().last()?;
    let mut found = None;
    for t in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if is_code(&t, g) {
            break;
        }
        let text = t.text();
        if g.has_tag(t.kind(), "comment")
            && text.starts_with(open.as_str())
            && text[open.len()..].starts_with(last)
            && text.len() > open.len() + close.len()
        {
            found = Some(t);
        }
    }
    found
}

struct Group {
    rows: Vec<Row>,
    outer: Vec<String>,
}

struct Missing {
    meta: &'static RuleMeta,
    message: String,
    groups: Vec<Group>,
    visible: Vec<String>,
    implicit: Vec<String>,
    getters: Vec<String>,
    setters: Vec<String>,
    tags: Vec<&'static str>,
}

fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let message = p.text("message")?;
    let visible = words(&p.optional("visible").unwrap_or_default());
    let implicit = words(&p.optional("implicit-public-in").unwrap_or_default());
    let getters = words(&p.optional("getters").unwrap_or_default());
    let setters = words(&p.optional("setters").unwrap_or_default());
    let mut outers = Vec::new();
    let mut groups = Vec::new();
    for (key, value) in p.rest() {
        if let Some(group) = key.strip_prefix("outer.") {
            outers.push((group.to_string(), words(&value)));
            continue;
        }
        let rows = (value.split('|'))
            .map(|alt| Row::parse(&key, alt))
            .collect::<Result<Vec<_>, _>>()?;
        groups.push((
            key,
            Group {
                rows,
                outer: Vec::new(),
            },
        ));
    }
    for (name, outer) in outers {
        let g = groups.iter_mut().find(|(k, _)| *k == name);
        g.ok_or_else(|| format!("param `outer.{name}` names no group"))?
            .1
            .outer = outer;
    }
    let groups: Vec<Group> = groups.into_iter().map(|(_, g)| g).collect();
    let mut tags: Vec<&'static str> = Vec::new();
    for r in groups.iter().flat_map(|g| &g.rows) {
        if !tags.contains(&r.tag.as_str()) {
            tags.push(Box::leak(r.tag.clone().into_boxed_str()));
        }
    }
    Ok(Box::new(Missing {
        meta,
        message,
        groups,
        visible,
        implicit,
        getters,
        setters,
        tags,
    }))
}

/// The node with the modifiers of a declaration: `node`, or its parent when the parent holds them.
fn holder(g: &Grammar, node: &SyntaxNode) -> SyntaxNode {
    match node.parent() {
        Some(p) if p.children().any(|c| g.has_tag(c.kind(), "modifiers")) => p,
        _ => node.clone(),
    }
}

impl Missing {
    /// Is the declaration in `holder` (inside `node`'s enclosing declarations) visible without a modifier?
    fn implicit(&self, g: &Grammar, node: &SyntaxNode) -> bool {
        let Some(outer) = node
            .ancestors()
            .skip(1)
            .find(|a| g.has_tag(a.kind(), "type-declaration"))
        else {
            return false;
        };
        let keyword = outer
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| is_code(t, g));
        keyword.is_some_and(|k| self.implicit.iter().any(|w| w == k.text()))
    }

    fn visible_declaration(&self, g: &Grammar, node: &SyntaxNode, holder: &SyntaxNode) -> bool {
        (self.visible.iter()).any(|w| has_modifier(g, holder, w))
            || (self.implicit(g, node) && !has_modifier(g, holder, "private"))
    }

    fn accessor(&self, g: &Grammar, node: &SyntaxNode, name: &str) -> bool {
        let prefixed = |words: &[String]| {
            (words.iter()).any(|w| {
                (name.strip_prefix(w.as_str())).is_some_and(|r| r.starts_with(char::is_uppercase))
            })
        };
        let (getter, setter) = (prefixed(&self.getters), prefixed(&self.setters));
        if !getter && !setter {
            return false;
        }
        let params = (node.children())
            .find(|c| g.has_tag(c.kind(), "parameter-list"))
            .map_or(0, |l| {
                l.children()
                    .filter(|c| g.has_tag(c.kind(), "parameter"))
                    .count()
            });
        let Some(body) = node.children().find(|c| g.has_tag(c.kind(), "block")) else {
            return false;
        };
        let mut statements = body.children().filter(|c| g.has_tag(c.kind(), "statement"));
        let (Some(only), None) = (statements.next(), statements.next()) else {
            return false;
        };
        if getter {
            params == 0 && g.has_tag(only.kind(), "return")
        } else {
            params == 1 && g.has_tag(only.kind(), "expression-statement")
        }
    }
}

impl Rule for Missing {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn bodies(&self) -> bool {
        false
    }

    fn tags(&self) -> &[&'static str] {
        &self.tags
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let g = ctx.file.grammar;
        let Some(group) = self
            .groups
            .iter()
            .find(|gr| gr.rows.iter().any(|r| r.matches(g, node)))
        else {
            return;
        };
        let holder = holder(g, node);
        if !self.visible_declaration(g, node, &holder)
            || (node.ancestors().skip(1))
                .any(|a| g.has_tag(a.kind(), "function") || g.has_tag(a.kind(), "call"))
        {
            return;
        }
        if !group.outer.is_empty() {
            for a in node
                .ancestors()
                .skip(1)
                .filter(|a| g.has_tag(a.kind(), "type-declaration"))
            {
                let h = self::holder(g, &a);
                let ok = group.outer.iter().any(|w| has_modifier(g, &h, w))
                    || (self.implicit(g, &a) && !has_modifier(g, &h, "private"));
                if !ok {
                    return;
                }
            }
        }
        if leading_doc(&holder, g).is_some() {
            return;
        }
        let allowed = ctx.options.list("allowed_annotations");
        if !allowed.is_empty()
            && annotation_names(g, &holder).iter().any(|n| {
                let simple = n.rsplit('.').next().unwrap_or(n);
                allowed.iter().any(|a| glob(a, n) || glob(a, simple))
            })
        {
            return;
        }
        let default = SlotPath::parse("name").unwrap_or_else(|_| unreachable!());
        let Some(name) = default.find(g, node, ctx.file.text) else {
            return;
        };
        if ctx.options.bool("property_accessors") && self.accessor(g, node, &name.text) {
            return;
        }
        let Some(first) = holder
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| is_code(t, g))
        else {
            return;
        };
        ctx.report(
            first.text_range(),
            self.message.replace("{name}", &name.text),
        );
    }
}
