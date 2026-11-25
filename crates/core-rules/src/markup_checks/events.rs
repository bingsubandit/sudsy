//! The checks over event handlers: a click with no key, a handler on an element that takes no
//! focus, a pointer event with no focus event.
//!
//! concept: markup-checks
//! what: Elements whose accessible name, scope, visibility or keyboard access is missing or contradictory: links without text, forms without a submit button, table headers without scope, hidden focusable elements, click handlers no key reaches, labels without a control, empty links and buttons, images without alternative text.
//! tools: html-validate wcag/h30, wcag/h32, wcag/h63, hidden-focusable, angular-eslint

use super::{MarkupCheck, Words};
use crate::content_model::Model;
use crate::markup::Markup;
use core_syntax::TextRange;

type Out = Vec<(TextRange, String)>;

/// The event without its key filter (`keydown.enter` is `keydown`).
fn event(name: &str) -> &str {
    name.split('.').next().unwrap_or(name)
}

impl Words {
    fn listens(&self, m: &Markup, i: usize, names: &[&str]) -> bool {
        m.events(i).any(|(n, _)| names.contains(&event(n)))
    }

    fn handles_key(&self, m: &Markup, i: usize) -> bool {
        self.listens(m, i, self.key_events)
    }

    /// An element a user can operate by itself: interactive content and widget elements of the
    /// model, not the ones that act through their control (`passive`).
    pub(crate) fn operable(&self, model: &Model, m: &Markup, i: usize) -> bool {
        let name = &m.elements[i].name;
        (model.in_category(m, i, "interactive") && !model.listed("passive", name))
            || model.in_category(m, i, "widget")
            || m.plain(i, self.contenteditable)
                .is_some_and(|v| v != "false")
    }

    /// Out of the way of assistive technology: hidden from it or marked presentational.
    pub(crate) fn set_aside(&self, m: &Markup, i: usize) -> bool {
        self.aria_hidden_self(m, i)
            || m.plain(i, self.role)
                .is_some_and(|r| self.presentation.contains(&r))
            || (m.elements[i].name == self.input
                && m.plain(i, self.type_attr) == Some(self.input_hidden))
    }
}

impl MarkupCheck {
    pub(super) fn click_key_events(&self, model: &Model, m: &Markup, i: usize, out: &mut Out) {
        let w = self.words;
        if !model.is_known(&m.elements[i].name)
            || !w.listens(m, i, &[w.click])
            || w.handles_key(m, i)
            || w.set_aside(m, i)
            || w.operable(model, m, i)
        {
            return;
        }
        out.push((
            m.elements[i].range,
            "Click events must be accompanied by either keyup, keydown or keypress events"
                .to_string(),
        ));
    }

    pub(super) fn interactive_focus(&self, model: &Model, m: &Markup, i: usize, out: &mut Out) {
        let w = self.words;
        let handles = w.listens(m, i, &[w.click]) || w.handles_key(m, i);
        let role = m.plain(i, w.role);
        if !handles
            || !model.is_known(&m.elements[i].name)
            || w.set_aside(m, i)
            || w.operable(model, m, i)
            || role.is_some_and(|r| model.listed("static-role", r))
            || m.attr(i, w.tabindex).is_some()
            || m.plain(i, w.aria_disabled) == Some("true")
        {
            return;
        }
        let message = match role {
            Some(r) => format!("Elements with the '{r}' interactive role must be focusable"),
            None => "Elements with interactive handlers must be focusable".to_string(),
        };
        out.push((m.elements[i].range, message));
    }

    pub(super) fn pointer_key_events(&self, m: &Markup, i: usize, out: &mut Out) {
        let w = self.words;
        for (pointer, key) in w.pointer_events {
            if m.events(i).any(|(n, _)| event(n) == *pointer) && !w.listens(m, i, &[key]) {
                out.push((
                    m.elements[i].range,
                    format!("'{pointer}' must be accompanied by '{key}' for accessibility"),
                ));
            }
        }
    }
}
