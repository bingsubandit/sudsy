//! The checks over names: a label with no control, a link, button or heading with no content, an
//! image with no text alternative, an `autofocus` attribute.
//!
//! concept: markup-checks
//! what: Elements whose accessible name, scope, visibility or keyboard access is missing or contradictory: links without text, forms without a submit button, table headers without scope, hidden focusable elements, click handlers no key reaches, labels without a control, empty links and buttons, images without alternative text.
//! tools: html-validate wcag/h30, wcag/h32, wcag/h63, hidden-focusable, angular-eslint

use super::MarkupCheck;
use crate::content_model::Model;
use crate::markup::Markup;
use crate::options::RuleOptions;
use core_syntax::TextRange;

type Out = Vec<(TextRange, String)>;

impl MarkupCheck {
    pub(super) fn label_control(&self, model: &Model, m: &Markup, i: usize, out: &mut Out) {
        let w = self.words;
        if m.elements[i].name != w.label_element {
            return;
        }
        let tied = w.label_for.iter().any(|a| m.attr(i, a).is_some())
            || m.descendants(i).into_iter().any(|d| {
                model.in_category(m, d, "labelable")
                    && m.plain(d, w.type_attr) != Some(w.input_hidden)
            });
        if !tied {
            out.push((
                m.elements[i].range,
                "A label must be associated with a form control".to_string(),
            ));
        }
    }

    pub(super) fn empty_content(&self, model: &Model, m: &Markup, i: usize, out: &mut Out) {
        let e = &m.elements[i];
        if !model.listed("needs-content", &e.name) {
            return;
        }
        let gives = |x: usize| {
            m.elements[x]
                .attrs
                .iter()
                .any(|a| model.listed("gives-content", &a.base.to_ascii_lowercase()))
        };
        if e.text || !e.children.is_empty() || gives(i) {
            return;
        }
        out.push((
            e.range,
            format!("<{}> must have content for assistive technology", e.name),
        ));
    }

    pub(super) fn autofocus(&self, model: &Model, m: &Markup, i: usize, out: &mut Out) {
        let w = self.words;
        if m.ancestors_and_self(i)
            .any(|a| m.elements[a].name == w.dialog)
        {
            return;
        }
        // A binding on a component is one of its inputs (`[autoFocus]`), not the HTML attribute.
        if let Some(a) = m
            .attr(i, w.autofocus)
            .filter(|a| a.bind.is_none() || model.is_known(&m.elements[i].name))
        {
            out.push((a.key, "Avoid using autofocus attribute".to_string()));
        }
    }

    pub(super) fn image_alt(&self, m: &Markup, i: usize, options: &RuleOptions, out: &mut Out) {
        let w = self.words;
        let e = &m.elements[i];
        let kind = if e.name == w.image {
            w.image
        } else if !options.bool("other_elements") {
            return;
        } else if e.name == w.area || e.name == w.object {
            e.name.as_str()
        } else if e.name == w.input && m.plain(i, w.type_attr) == Some(w.input_image) {
            w.input
        } else {
            return;
        };
        if options.bool("skip_hidden") && w.set_aside(m, i) {
            return;
        }
        let named = |name: &str| {
            m.attr(i, name).is_some_and(|a| {
                a.bind.is_some() || a.value.as_deref().is_some_and(|v| !v.is_empty())
            })
        };
        let alt = m.attr(i, w.alt).is_some()
            || (options.bool("name_counts")
                && (named(w.label) || named(w.labelledby) || (kind == w.object && named("title"))));
        if !alt {
            out.push((e.range, format!("<{}> needs an `alt` attribute", e.name)));
        }
    }
}
