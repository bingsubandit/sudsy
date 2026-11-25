//! Accessibility checks over the element tree ([`crate::markup`]) and the content categories of a
//! [`Model`]: a link with no text, a form with no submit button, a table whose header cells lack a
//! scope, a focusable element that is hidden from assistive technology, a click handler no key
//! reaches, a label with no control, an empty link, button or heading. The names of the elements
//! and attributes are the language's [`Words`]; the sets of elements and roles are rows of its
//! [`Model`].
//!
//! concept: markup-checks
//! what: Elements whose accessible name, scope, visibility or keyboard access is missing or contradictory: links without text, forms without a submit button, table headers without scope, hidden focusable elements, click handlers no key reaches, labels without a control, empty links and buttons, images without alternative text.
//! tools: html-validate wcag/h30, wcag/h32, wcag/h63, hidden-focusable, angular-eslint

use crate::checks::FileCache;
use crate::content_model::Model;
use crate::markup::Markup;
use crate::options::RuleOptions;
use crate::rule::{FileContext, Rule, RuleContext, RuleMeta};
use core_syntax::TextRange;

mod events;
mod names;

/// The words of a language for the checks.
pub struct Words {
    pub link: &'static str,
    pub target: &'static str,
    pub image: &'static str,
    pub alt: &'static str,
    pub label: &'static str,
    pub hidden: &'static str,
    pub aria_hidden: &'static str,
    pub inert: &'static str,
    pub role: &'static str,
    pub presentation: &'static [&'static str],
    pub tabindex: &'static str,
    pub disabled: &'static str,
    pub style: &'static str,
    pub table: &'static str,
    pub header: &'static str,
    pub row: &'static str,
    pub row_groups: &'static [&'static str],
    pub head_group: &'static str,
    pub body_group: &'static str,
    pub scope: &'static str,
    pub scopes: &'static [&'static str],
    pub headers: &'static str,
    pub id: &'static str,
    /// The attribute that ties a control to a form by id.
    pub form_attr: &'static str,
    /// Categories of the model: forms, controls that can take focus, buttons that submit.
    pub form_category: &'static str,
    pub focusable_category: &'static str,
    pub submit_category: &'static str,
    /// Template syntax that makes a value unknown (`{{`).
    pub dynamic: &'static str,
    /// The attributes of a label's target (`for`, and `htmlFor` of a framework), lowercase.
    pub label_for: &'static [&'static str],
    pub label_element: &'static str,
    pub labelledby: &'static str,
    pub autofocus: &'static str,
    pub dialog: &'static str,
    pub type_attr: &'static str,
    pub contenteditable: &'static str,
    pub aria_disabled: &'static str,
    /// The element and the `type` of an image button, and the elements that need an alternative
    /// text besides the image.
    pub input: &'static str,
    pub input_hidden: &'static str,
    pub input_image: &'static str,
    pub area: &'static str,
    pub object: &'static str,
    /// Events that count as a key (the name may continue after a `.`).
    pub key_events: &'static [&'static str],
    pub click: &'static str,
    /// Events that listen to the pointer and the one that reaches the same user by keyboard.
    pub pointer_events: &'static [(&'static str, &'static str)],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    /// An element with a click handler that no key event comes with.
    ClickKeyEvents,
    /// An element with a click or key handler that cannot take focus.
    InteractiveFocus,
    /// A pointer event with no matching focus event.
    PointerKeyEvents,
    /// A label with no control.
    LabelControl,
    /// A link, button or heading with nothing in it.
    EmptyContent,
    /// An `autofocus` attribute.
    Autofocus,
    /// An image (or area, object, image button) with no text alternative.
    ImageAlt,
    /// A link with no text, no image with alt text and no label.
    LinkText,
    /// A form with no submit button of its own or associated with it.
    FormSubmit,
    /// Header cells with no valid scope in a table that is not a simple one.
    HeaderScope,
    /// An element that takes focus and is hidden with `aria-hidden`.
    HiddenFocusable,
}

pub struct MarkupCheck {
    pub(crate) meta: &'static RuleMeta,
    check: Check,
    model: fn() -> &'static Model,
    words: &'static Words,
}

impl MarkupCheck {
    pub const fn new(
        meta: &'static RuleMeta,
        check: Check,
        model: fn() -> &'static Model,
        words: &'static Words,
    ) -> Self {
        MarkupCheck {
            meta,
            check,
            model,
            words,
        }
    }
}

fn value<'a>(m: &'a Markup, i: usize, name: &str) -> Option<&'a str> {
    m.plain(i, name)
}

impl Words {
    pub(crate) fn aria_hidden_self(&self, m: &Markup, i: usize) -> bool {
        value(m, i, self.aria_hidden) == Some("true")
    }

    pub(crate) fn aria_hidden(&self, m: &Markup, i: usize) -> bool {
        m.ancestors_and_self(i).any(|a| self.aria_hidden_self(m, a))
    }

    /// Hidden from everyone: the `hidden` attribute, `inert` or a hiding style, on the element or an ancestor.
    pub(crate) fn hidden(&self, m: &Markup, i: usize) -> bool {
        m.ancestors_and_self(i).any(|a| {
            m.attr(a, self.hidden).is_some()
                || m.attr(a, self.inert).is_some()
                || value(m, a, self.style).is_some_and(|s| {
                    let s: String = s
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>()
                        .to_ascii_lowercase();
                    s.contains("display:none") || s.contains("visibility:hidden")
                })
        })
    }

    fn in_tree(&self, m: &Markup, i: usize) -> bool {
        !self.aria_hidden(m, i)
            && !self.hidden(m, i)
            && !(m.attr(i, self.tabindex).is_none()
                && value(m, i, self.role).is_some_and(|r| self.presentation.contains(&r)))
    }

    pub(crate) fn labelled(&self, m: &Markup, i: usize) -> bool {
        m.attr(i, self.label)
            .is_some_and(|a| a.value.as_deref().is_some_and(|v| !v.is_empty()))
    }

    fn has_text(&self, m: &Markup, root: usize) -> bool {
        let mut stack = vec![root];
        while let Some(i) = stack.pop() {
            if m.elements[i].text {
                return true;
            }
            stack.extend(
                m.elements[i]
                    .children
                    .iter()
                    .copied()
                    .filter(|&c| m.attr(c, self.hidden).is_none() && !self.aria_hidden_self(m, c)),
            );
        }
        false
    }
}

impl MarkupCheck {
    fn run(&self, m: &Markup, options: &RuleOptions, out: &mut Vec<(TextRange, String)>) {
        let model = (self.model)();
        let w = self.words;
        for i in 0..m.elements.len() {
            let e = &m.elements[i];
            if e.implied {
                continue;
            }
            match self.check {
                Check::ClickKeyEvents => self.click_key_events(model, m, i, out),
                Check::InteractiveFocus => self.interactive_focus(model, m, i, out),
                Check::PointerKeyEvents => self.pointer_key_events(m, i, out),
                Check::LabelControl => self.label_control(model, m, i, out),
                Check::EmptyContent => self.empty_content(model, m, i, out),
                Check::Autofocus => self.autofocus(model, m, i, out),
                Check::ImageAlt => self.image_alt(m, i, options, out),
                Check::LinkText => self.link_text(m, i, out),
                Check::FormSubmit => {
                    if model.in_category(m, i, w.form_category) && !self.has_submit(model, m, i) {
                        out.push((
                            e.range,
                            format!("<{}> element must have a submit button", e.name),
                        ));
                    }
                }
                Check::HeaderScope => {
                    if e.name == w.table && !self.simple_table(m, i) {
                        for d in m
                            .descendants(i)
                            .into_iter()
                            .filter(|&d| m.elements[d].name == w.header)
                        {
                            let scope = m.attr(d, w.scope);
                            if let Some(v) = scope.and_then(|a| a.value.as_deref())
                                && (w.scopes.contains(&v) || v.contains(w.dynamic))
                            {
                                continue;
                            }
                            let range = scope
                                .map(|a| a.value_range.unwrap_or(a.key))
                                .unwrap_or(m.elements[d].range);
                            out.push((
                                range,
                                format!(
                                    "<{}> element must have a valid scope attribute: {}",
                                    w.header,
                                    natural_join(w.scopes)
                                ),
                            ));
                        }
                    }
                }
                Check::HiddenFocusable => {
                    let focus = match value(m, i, w.tabindex) {
                        Some(t) => t.trim().parse::<i64>().is_ok_and(|n| n >= 0),
                        None if m.attr(i, w.tabindex).is_some() => false,
                        None => {
                            model.in_category(m, i, w.focusable_category)
                                && m.attr(i, w.disabled).is_none()
                        }
                    };
                    if focus && !w.hidden(m, i) && w.aria_hidden(m, i) {
                        let own = m.attr(i, w.aria_hidden);
                        let (range, message) = match own {
                            Some(a) => (a.key, "aria-hidden cannot be used on focusable elements"),
                            None => (
                                e.range,
                                "aria-hidden cannot be used on focusable elements (hidden by ancestor element)",
                            ),
                        };
                        out.push((range, message.to_string()));
                    }
                }
            }
        }
    }

    fn link_text(&self, m: &Markup, i: usize, out: &mut Vec<(TextRange, String)>) {
        let w = self.words;
        let e = &m.elements[i];
        if e.name != w.link || m.attr(i, w.target).is_none() || !w.in_tree(m, i) {
            return;
        }
        if w.has_text(m, i) || w.labelled(m, i) {
            return;
        }
        let below = m.descendants(i);
        let alt = |d: &usize| {
            m.elements[*d].name == w.image && value(m, *d, w.alt).is_some_and(|v| !v.is_empty())
        };
        if below.iter().any(alt) || below.iter().any(|&d| w.labelled(m, d)) {
            return;
        }
        out.push((
            e.range,
            "Anchor link must have a text describing its purpose".to_string(),
        ));
    }

    fn has_submit(&self, model: &Model, m: &Markup, form: usize) -> bool {
        let w = self.words;
        let submit = |d: usize| model.in_category(m, d, w.submit_category);
        if m.descendants(form)
            .into_iter()
            .any(|d| submit(d) && m.attr(d, w.form_attr).is_none())
        {
            return true;
        }
        let Some(id) = value(m, form, w.id).filter(|v| !v.is_empty()) else {
            return false;
        };
        (0..m.elements.len())
            .any(|d| submit(d) && value(m, d, w.form_attr).is_some_and(|v| v.trim() == id))
    }

    /// A table with one row of header cells (or one column of them, without a `<thead>`): its
    /// headers need no scope.
    fn simple_table(&self, m: &Markup, table: usize) -> bool {
        let w = self.words;
        let kids = &m.elements[table].children;
        let name = |i: usize| m.elements[i].name.as_str();
        let rows_of = |group: &str| -> Vec<usize> {
            kids.iter()
                .copied()
                .filter(|&k| name(k) == group)
                .flat_map(|g| {
                    m.elements[g]
                        .children
                        .iter()
                        .copied()
                        .filter(|&r| name(r) == w.row)
                })
                .collect()
        };
        let direct: Vec<usize> = kids.iter().copied().filter(|&k| name(k) == w.row).collect();
        let body_rows = rows_of(w.body_group);
        if direct.iter().chain(&body_rows).any(|&r| {
            m.elements[r]
                .children
                .iter()
                .any(|&c| m.attr(c, w.headers).is_some())
        }) {
            return false;
        }
        let mut rows: Vec<usize> = direct;
        for group in w.row_groups {
            rows.extend(rows_of(group));
        }
        rows.sort_unstable();
        let Some(&first) = rows.first() else {
            return false;
        };
        let cells: Vec<Vec<usize>> = rows
            .iter()
            .map(|&r| {
                m.elements[r]
                    .children
                    .iter()
                    .map(|&c| usize::from(name(c) == w.header))
                    .collect()
            })
            .collect();
        let columns = m.elements[first].children.len();
        if columns == 0 || cells.iter().any(|r| r.len() != columns) {
            return false;
        }
        let per_row: Vec<usize> = cells.iter().map(|r| r.iter().sum()).collect();
        if per_row[0] == columns && per_row[1..].iter().all(|&n| n == 0) {
            return true;
        }
        let per_column: Vec<usize> = (0..columns)
            .map(|c| cells.iter().map(|r| r[c]).sum())
            .collect();
        let thead = kids.iter().any(|&k| name(k) == w.head_group);
        per_column[0] == cells.len() && per_column[1..].iter().all(|&n| n == 0) && !thead
    }
}

fn natural_join(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => one.to_string(),
        [head @ .., last] => format!("{} or {last}", head.join(", ")),
    }
}

thread_local! {
    /// The tree of the last file: the checks of a file share one build.
    static MARKUP: FileCache<Markup> = const { FileCache::new() };
}

impl Rule for MarkupCheck {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let model = (self.model)();
        let markup = MARKUP.with(|cache| {
            cache.get(file, || {
                let mut markup = Markup::build(
                    file.root,
                    file.grammar,
                    file.text,
                    true,
                    &model.dialect(file.path),
                );
                model.synthesize(&mut markup);
                markup
            })
        });
        let mut out = Vec::new();
        self.run(&markup, ctx.options, &mut out);
        for (range, message) in out {
            ctx.report(range, message);
        }
    }
}

/// Declares a static [`MarkupCheck`] rule.
#[macro_export]
macro_rules! markup_rule {
    (
        $vis:vis $name:ident {
            id: $id:expr,
            category: $category:ident,
            description: $description:expr,
            why: $why:expr,
            example: $example:expr,
            options: $options:expr,
            check: $check:ident,
            model: $model:expr,
            words: $words:expr,
        }
    ) => {
        $vis static $name: $crate::markup_checks::MarkupCheck = $crate::markup_checks::MarkupCheck::new(
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
                    options: $options,
                };
                &META
            },
            $crate::markup_checks::Check::$check,
            $model,
            $words,
        );
    };
}
