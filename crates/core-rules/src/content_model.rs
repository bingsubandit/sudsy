//! What an element may contain, as data over one check: the content categories, the children an
//! element permits (`content`), the descendants it forbids (`exclude`), the order of its children
//! (`order`), the children it requires (`required`) and the elements a child opens implicitly (a
//! `<script>` straight under `<html>` opens a `<head>`). The rules are [`ContentModel`] values with
//! one [`Check`] each; the data is a [`Model`] parsed from text rows:
//!
//! ```text
//! known a,abbr,...                one element name list: the elements that have rules at all
//! category flow a,abbr,...        the members of a category (`@flow` in items; `@a-not-b` is a without b)
//! if interactive a href           a member of a category only with an attribute (`attr`, `!attr`, `attr=a|b`, `attr!=value`) or place (`in:tag`)
//! content div @flow dt dd         permitted children (`name`, `@category`, `a|b`, a `?` suffix: at most one)
//! exclude a @interactive a        forbidden descendants
//! order table caption tbody       children in this order
//! required head title             children that must be there
//! open html @meta:head            a child of this category gets an implied element of that name
//! closed head body @flow-not-meta children that end an implied element
//! transparent a,ins               children are judged against the parent's content
//! ```
//!
//! concept: content-model
//! what: Elements of a markup file that break the content model of their parent or ancestors (permitted content, order, occurrences, required content).
//! tools: html-validate element-permitted-content, element-permitted-order, element-permitted-occurrences, element-required-content

use crate::family::comma_list as list;
use crate::markup::{Dialect, Element, Markup, Spellings};
use crate::rule::{FileContext, Rule, RuleContext, RuleMeta};
use core_syntax::TextRange;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
enum Cond {
    Attr(String),
    AttrNot(String, String),
    /// The attribute's value is one of these (lowercase).
    AttrIn(String, Vec<String>),
    Absent(String),
    In(String),
}

/// The rows of a content model.
#[derive(Default, Debug)]
pub struct Model {
    known: HashSet<String>,
    categories: HashMap<String, HashSet<String>>,
    conditions: HashMap<String, Vec<(Vec<String>, Cond)>>,
    content: HashMap<String, Vec<String>>,
    exclude: HashMap<String, Vec<String>>,
    order: HashMap<String, Vec<String>>,
    required: HashMap<String, Vec<String>>,
    open: HashMap<String, Vec<(String, String)>>,
    closed: HashMap<String, Vec<String>>,
    transparent: HashSet<String>,
    spellings: Spellings,
}

impl Model {
    pub fn parse(text: &str) -> Model {
        let mut m = Model {
            spellings: Spellings::parse(text),
            ..Model::default()
        };
        for line in text.lines() {
            let mut words = line.split_whitespace();
            let (Some(kind), Some(subject)) = (words.next(), words.next()) else {
                continue;
            };
            let rest: Vec<String> = words.map(str::to_string).collect();
            match kind {
                "known" => m.known.extend(list(subject)),
                "transparent" => m.transparent.extend(list(subject)),
                "category" => {
                    if let Some(names) = rest.first() {
                        m.categories
                            .entry(subject.to_string())
                            .or_default()
                            .extend(list(names));
                    }
                }
                "if" => {
                    if let [names, cond] = rest.as_slice() {
                        let cond = if let Some(tag) = cond.strip_prefix("in:") {
                            Cond::In(tag.to_string())
                        } else if let Some((a, v)) = cond.split_once("!=") {
                            Cond::AttrNot(a.to_string(), v.to_string())
                        } else if let Some((a, v)) = cond.split_once('=') {
                            Cond::AttrIn(a.to_string(), v.split('|').map(str::to_string).collect())
                        } else if let Some(a) = cond.strip_prefix('!') {
                            Cond::Absent(a.to_string())
                        } else {
                            Cond::Attr(cond.to_string())
                        };
                        m.conditions
                            .entry(subject.to_string())
                            .or_default()
                            .push((list(names), cond));
                    }
                }
                _ => {
                    for name in list(subject) {
                        match kind {
                            "content" => drop(m.content.insert(name, rest.clone())),
                            "exclude" => drop(m.exclude.insert(name, rest.clone())),
                            "order" => drop(m.order.insert(name, rest.clone())),
                            "required" => drop(m.required.insert(name, rest.clone())),
                            "closed" => drop(m.closed.insert(name, rest.clone())),
                            "open" => {
                                let rows = rest
                                    .iter()
                                    .filter_map(|r| r.split_once(':'))
                                    .map(|(c, t)| {
                                        (c.trim_start_matches('@').to_string(), t.to_string())
                                    })
                                    .collect();
                                m.open.insert(name, rows);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        m
    }

    /// How the file's dialect writes bound attributes.
    pub fn dialect(&self, path: &std::path::Path) -> Dialect<'_> {
        self.spellings.of(path)
    }

    /// Is `name` an element of the language (not a component of a framework)?
    pub fn is_known(&self, name: &str) -> bool {
        self.known.contains(name)
    }

    /// Is `name` in the plain list of the category (a row without conditions)?
    pub fn listed(&self, category: &str, name: &str) -> bool {
        self.categories
            .get(category)
            .is_some_and(|c| c.contains(name))
    }

    pub fn in_category(&self, markup: &Markup, index: usize, category: &str) -> bool {
        if let Some((a, b)) = category.split_once("-not-") {
            return self.in_category(markup, index, a) && !self.in_category(markup, index, b);
        }
        let e = &markup.elements[index];
        if self
            .categories
            .get(category)
            .is_some_and(|c| c.contains(&e.name))
        {
            return true;
        }
        self.conditions.get(category).is_some_and(|rows| {
            rows.iter()
                .filter(|(names, _)| names.contains(&e.name))
                .any(|(_, cond)| match cond {
                    Cond::Attr(a) => markup.attr(index, a).is_some(),
                    Cond::AttrNot(a, v) => !markup
                        .plain(index, a)
                        .is_some_and(|x| x.eq_ignore_ascii_case(v)),
                    Cond::AttrIn(a, values) => markup
                        .plain(index, a)
                        .is_some_and(|x| values.iter().any(|v| x.eq_ignore_ascii_case(v))),
                    Cond::Absent(a) => markup.attr(index, a).is_none(),
                    Cond::In(tag) => markup
                        .ancestors_and_self(index)
                        .skip(1)
                        .any(|i| markup.elements[i].name == *tag),
                })
        })
    }

    /// Does the element match the item (a name, an `@category`, alternatives `a|b`, a `?`/`*`
    /// suffix ignored)? An element without rules is in no category: `default` answers.
    fn matches(&self, markup: &Markup, index: usize, item: &str, default: bool) -> bool {
        let item = item.trim_end_matches(['?', '*']);
        item.split('|').any(|alt| match alt.strip_prefix('@') {
            Some(category) => {
                if self.known.contains(&markup.elements[index].name) {
                    self.in_category(markup, index, category)
                } else {
                    default
                }
            }
            None => markup.elements[index].name == alt,
        })
    }

    /// Adds the elements an element's rules open implicitly: a run of children of one category
    /// gets one implied parent (an implied element stays open until a child of its `closed` list).
    pub fn synthesize(&self, markup: &mut Markup) {
        for p in 0..markup.elements.len() {
            let Some(rules) = self.open.get(&markup.elements[p].name) else {
                continue;
            };
            let children = std::mem::take(&mut markup.elements[p].children);
            let mut kept: Vec<usize> = Vec::new();
            let mut open: Option<(usize, &str)> = None;
            for c in children {
                if let Some((implied, tag)) = open
                    && !self.closed.get(tag).is_some_and(|closers| {
                        closers.iter().any(|x| self.matches(markup, c, x, false))
                            || markup.elements[c].name == tag
                    })
                {
                    markup.elements[c].parent = Some(implied);
                    markup.elements[implied].children.push(c);
                    continue;
                }
                open = None;
                let Some((_, tag)) = rules.iter().find(|(cat, _)| {
                    self.in_category(markup, c, cat) && markup.elements[c].name != *cat
                }) else {
                    kept.push(c);
                    continue;
                };
                if markup.elements[c].name == *tag {
                    kept.push(c);
                    continue;
                }
                let first = &markup.elements[c];
                let implied = markup.elements.len();
                markup.elements.push(Element {
                    name: tag.clone(),
                    name_range: first.name_range,
                    range: first.range,
                    parent: Some(p),
                    children: vec![c],
                    attrs: Vec::new(),
                    text: false,
                    end_tag: false,
                    empty: false,
                    implied: true,
                });
                markup.elements[c].parent = Some(implied);
                kept.push(implied);
                open = Some((implied, tag.as_str()));
            }
            markup.elements[p].children = kept;
        }
    }

    /// The reports of `check` over the document: range and message.
    fn run(&self, markup: &Markup, check: Check) -> Vec<(TextRange, String)> {
        let mut out = Vec::new();
        for i in 0..markup.elements.len() {
            let e = &markup.elements[i];
            if !self.known.contains(&e.name) && check != Check::Content {
                continue;
            }
            match check {
                Check::Content => self.content_of(markup, i, &mut out),
                Check::Order => self.order_of(markup, i, &mut out),
                Check::Occurrences => self.occurrences_of(markup, i, &mut out),
                Check::Required => self.required_of(markup, i, &mut out),
            }
        }
        out
    }

    fn content_of(&self, markup: &Markup, i: usize, out: &mut Vec<(TextRange, String)>) {
        let Some(parent) = markup.elements[i].parent else {
            return;
        };
        if self.permitted_under(markup, i, parent, out) {
            return;
        }
        let node = &markup.elements[i];
        for anc in markup.ancestors_and_self(parent) {
            let a = &markup.elements[anc];
            if a.name == "template" {
                break;
            }
            let Some(rules) = self.exclude.get(&a.name) else {
                continue;
            };
            if rules.iter().any(|r| self.matches(markup, i, r, false)) {
                out.push((
                    node.range,
                    format!(
                        "<{}> element is not permitted as a descendant of <{}>",
                        node.name, a.name
                    ),
                ));
                return;
            }
        }
    }

    /// Is `i` permitted as content of `parent` (its transparent children as well)? Reports when not.
    fn permitted_under(
        &self,
        markup: &Markup,
        i: usize,
        under: usize,
        out: &mut Vec<(TextRange, String)>,
    ) -> bool {
        let p = &markup.elements[under];
        let Some(rules) = self
            .known
            .contains(&p.name)
            .then(|| self.content.get(&p.name))
            .flatten()
        else {
            return false;
        };
        let node = &markup.elements[i];
        if !rules.iter().any(|r| self.matches(markup, i, r, true)) {
            out.push((
                node.range,
                format!(
                    "<{}> element is not permitted as content under <{}>",
                    node.name, p.name
                ),
            ));
            return true;
        }
        if self.transparent.contains(&node.name) {
            let mut any = false;
            for &c in &node.children {
                any |= self.permitted_under(markup, c, under, out);
            }
            return any;
        }
        false
    }

    fn order_of(&self, markup: &Markup, i: usize, out: &mut Vec<(TextRange, String)>) {
        let e = &markup.elements[i];
        let Some(rules) = self.order.get(&e.name) else {
            return;
        };
        let mut at = 0;
        let mut prev: Option<usize> = None;
        for &c in &e.children {
            let old = at;
            while at < rules.len() && !self.matches(markup, c, &rules[at], true) {
                at += 1;
            }
            if at >= rules.len() {
                if rules.iter().any(|r| self.matches(markup, c, r, true)) {
                    let node = &markup.elements[c];
                    let before = prev.map(|p| markup.elements[p].name.as_str()).unwrap_or("");
                    out.push((
                        node.range,
                        format!(
                            "Element <{}> must be used before <{}> in this context",
                            node.name, before
                        ),
                    ));
                    return;
                }
                at = old;
            }
            prev = Some(c);
        }
    }

    fn occurrences_of(&self, markup: &Markup, i: usize, out: &mut Vec<(TextRange, String)>) {
        let e = &markup.elements[i];
        let Some(rules) = self.content.get(&e.name) else {
            return;
        };
        for rule in rules.iter().filter(|r| r.ends_with('?')) {
            let category = rule.trim_end_matches('?');
            let found: Vec<usize> = e
                .children
                .iter()
                .copied()
                .filter(|&c| self.matches(markup, c, rule, true))
                .collect();
            for &c in found.iter().skip(1) {
                out.push((
                    markup.elements[c].range,
                    format!(
                        "Element <{category}> can only appear once under <{}>",
                        e.name
                    ),
                ));
            }
        }
    }

    fn required_of(&self, markup: &Markup, i: usize, out: &mut Vec<(TextRange, String)>) {
        let e = &markup.elements[i];
        let Some(rules) = self.required.get(&e.name) else {
            return;
        };
        for rule in rules {
            if e.children
                .iter()
                .any(|&c| self.matches(markup, c, rule, false))
            {
                continue;
            }
            let what = match rule.strip_prefix('@') {
                Some(category) => format!("{category} element"),
                None => format!("<{rule}>"),
            };
            out.push((
                e.range,
                format!("<{}> element must have {what} as content", e.name),
            ));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    /// A child or descendant the parent or an ancestor does not permit.
    Content,
    /// Children out of the order the parent lists.
    Order,
    /// More children of a kind than the parent allows.
    Occurrences,
    /// A child the element requires is missing.
    Required,
}

/// A rule of one [`Check`] over a [`Model`]. Names are lowercased (HTML).
pub struct ContentModel {
    meta: &'static RuleMeta,
    check: Check,
    model: fn() -> &'static Model,
}

impl ContentModel {
    pub const fn new(meta: &'static RuleMeta, check: Check, model: fn() -> &'static Model) -> Self {
        ContentModel { meta, check, model }
    }
}

impl Rule for ContentModel {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let model = (self.model)();
        let mut markup = Markup::build(
            file.root,
            file.grammar,
            file.text,
            true,
            &model.dialect(file.path),
        );
        model.synthesize(&mut markup);
        for (range, message) in model.run(&markup, self.check) {
            ctx.report(range, message);
        }
    }
}

/// Declares a static [`ContentModel`] rule: the metadata like `rule!`, the check and the model.
#[macro_export]
macro_rules! content_rule {
    (
        $vis:vis $name:ident {
            id: $id:expr,
            category: $category:ident,
            description: $description:expr,
            why: $why:expr,
            example: $example:expr,
            check: $check:ident,
            model: $model:expr,
        }
    ) => {
        $vis static $name: $crate::content_model::ContentModel =
            $crate::content_model::ContentModel::new(
                {
                    static META: $crate::RuleMeta = $crate::RuleMeta {
                        id: $id,
                        category: $crate::Category::$category,
                        why: $why,
                        source: "",
                        description: $description,
                        example: $example,
                        default_level: $crate::rule::Level::Off,
                        fix: None,
                        requires: &[],
                        uses: &[],
                        options: &[],
                    };
                    &META
                },
                $crate::content_model::Check::$check,
                $model,
            );
    };
}
