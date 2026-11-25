//! The accessibility checks over the element tree: the words of HTML for the shared checks.
//!
//! concept: markup-checks

use crate::content::model;
use core_rules::markup_checks::Words;
use core_rules::markup_rule;
use core_rules::options::{Check, Opt, OptionDecl};

const SKIP_HIDDEN: OptionDecl = OptionDecl {
    name: "skip_hidden",
    default: Opt::Bool(true),
    check: Check::Any,
    description: "Leave out an element hidden from assistive technology (`aria-hidden`, `role=\"presentation\"` or `none`).",
};
const NAME_COUNTS: OptionDecl = OptionDecl {
    name: "name_counts",
    default: Opt::Bool(false),
    check: Check::Any,
    description: "A non-empty `aria-label` or `aria-labelledby` (and `title` on `<object>`) counts as the text alternative.",
};
const OTHER_ELEMENTS: OptionDecl = OptionDecl {
    name: "other_elements",
    default: Opt::Bool(false),
    check: Check::Any,
    description: "Also check `<area>`, `<object>` and `<input type=\"image\">`.",
};

static WORDS: Words = Words {
    link: "a",
    target: "href",
    image: "img",
    alt: "alt",
    label: "aria-label",
    hidden: "hidden",
    aria_hidden: "aria-hidden",
    inert: "inert",
    role: "role",
    presentation: &["presentation", "none"],
    tabindex: "tabindex",
    disabled: "disabled",
    style: "style",
    table: "table",
    header: "th",
    row: "tr",
    row_groups: &["thead", "tbody"],
    head_group: "thead",
    body_group: "tbody",
    scope: "scope",
    scopes: &["row", "col", "rowgroup", "colgroup"],
    headers: "headers",
    id: "id",
    form_attr: "form",
    form_category: "form",
    focusable_category: "focusable",
    submit_category: "submit",
    dynamic: "{{",
    label_for: &["for"],
    label_element: "label",
    labelledby: "aria-labelledby",
    autofocus: "autofocus",
    dialog: "dialog",
    type_attr: "type",
    contenteditable: "contenteditable",
    aria_disabled: "aria-disabled",
    input: "input",
    input_hidden: "hidden",
    input_image: "image",
    area: "area",
    object: "object",
    key_events: &["keydown", "keyup", "keypress"],
    click: "click",
    pointer_events: &[("mouseover", "focus"), ("mouseout", "blur")],
};

markup_rule! {
    pub LINK_TEXT {
        id: "link-text",
        category: Correctness,
        description: "Reports a link (`<a href>`) with no text, no image with an `alt` and no \
              `aria-label` (WCAG H30). A link hidden from assistive technology is not reported.",
        why: "A screen reader announces the link as \"link\" with no purpose, and a list of the \
              links of a page shows an empty entry. Sighted users get a link they cannot see or \
              have no name for.\n\
              To fix, put text in the link, or an image with an `alt`, or add `aria-label`.",
        example: "<a href=\"/x\"><i class=\"icon\"></i></a>",
        options: &[],
        check: LinkText,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub FORM_SUBMIT {
        id: "form-submit",
        category: Correctness,
        description: "Reports a `<form>` that has no submit button inside it and none that names it with \
              `form=` (WCAG H32).",
        why: "Without a submit button a form can only be sent by pressing Enter in a field, \
              which not every user knows, and which does nothing in a form with several fields \
              in some browsers.\n\
              To fix, add a `<button type=\"submit\">` (or `<input type=\"submit\">`) to the form.",
        example: "<form><input name=\"q\"></form>",
        options: &[],
        check: FormSubmit,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub HEADER_SCOPE {
        id: "header-scope",
        category: Correctness,
        description: "Reports a `<th>` with no `scope` (`row`, `col`, `rowgroup` or `colgroup`) in a \
              table that is not a simple one. A table with a single row or a single column of \
              headers is simple (WCAG H63).",
        why: "In a table with headers on both axes a screen reader cannot tell whether a header \
              belongs to the cells beside it or below it, so a cell is read with the wrong \
              header.\n\
              To fix, add the `scope` that says which cells the header describes.",
        example: "<table><tr><th></th><th>b</th></tr><tr><th>c</th><td></td></tr></table>",
        options: &[],
        check: HeaderScope,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub HIDDEN_FOCUSABLE {
        id: "hidden-focusable",
        category: Correctness,
        description: "Reports an element that can take focus (a link, a control, an element with \
              `tabindex` of 0 or more) inside an element with `aria-hidden=\"true\"`.",
        why: "Keyboard users can still reach the element, but assistive technology does not \
              announce it, so focus goes to something that is not there for the screen reader \
              user.\n\
              To fix, remove `aria-hidden`, or make the content not focusable (`tabindex=\"-1\"`, \
              `disabled`, `inert`).",
        example: "<div aria-hidden=\"true\"><button>x</button></div>",
        options: &[],
        check: HiddenFocusable,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub IMG_ALT {
        id: "img-alt",
        category: Correctness,
        description: "Reports an `<img>` with no `alt` attribute (WCAG H37). One hidden from assistive technology (`aria-hidden=\"true\"`, `role=\"presentation\"` or `none`) is left alone, unless `skip_hidden` is off. A bound `[alt]` or `:alt` counts. With `name_counts`, so does a non-empty `aria-label` or `aria-labelledby`. With `other_elements`, an `<area>`, an `<object>` (also named by `title`) and an `<input type=\"image\">` need one too.",
        why: "With no `alt` a screen reader reads the file name, and a browser that does not load the \
              image shows nothing. An `alt` is also what search engines index for the image.\n\
              To fix, add an `alt` that says what the image shows, or `alt=\"\"` if it is only \
              decoration.",
        example: "<img src=\"a.png\">   <img src=\"a.png\" alt=\"A cat\">",
        options: &[SKIP_HIDDEN, OTHER_ELEMENTS, NAME_COUNTS],
        check: ImageAlt,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub CLICK_KEY_EVENTS {
        id: "click-events-have-key-events",
        category: Correctness,
        description: "Reports an element with a click handler (`(click)`, `@click`) and no `keydown`, `keyup` or `keypress` handler. Elements that a keyboard operates by themselves (links with `href`, buttons, form controls, `<summary>`, `<option>` and cells) and elements hidden from assistive technology or marked presentational are left alone.",
        why: "A click handler on a `<div>` or `<span>` works with a mouse only: the element takes no focus and a key press does not fire `click`. Keyboard and switch users cannot reach the action. Use a `<button>`, or add a key handler and a `tabindex`.",
        example: "<div (click)=\"open()\">Open</div>",
        options: &[],
        check: ClickKeyEvents,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub INTERACTIVE_FOCUS {
        id: "interactive-supports-focus",
        category: Correctness,
        description: "Reports an element with a click or key handler that cannot take focus: no `tabindex`, not a control or link with `href`, and no role that is not a widget (`heading`, `listitem`, `dialog`, ...). The role of the element is named in the message. Hidden, presentational and `aria-disabled=\"true\"` elements are left alone.",
        why: "An element that does something when it is clicked or when a key is pressed must be reachable by the Tab key, or a keyboard user never gets to trigger it. Add `tabindex=\"0\"` or use a native control such as `<button>`.",
        example: "<div role=\"button\" (keydown)=\"open()\">Open</div>",
        options: &[],
        check: InteractiveFocus,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub POINTER_KEY_EVENTS {
        id: "mouse-events-have-key-events",
        category: Correctness,
        description: "Reports a `mouseover` handler with no `focus` handler and a `mouseout` handler with no `blur` handler on the same element.",
        why: "Content that appears or changes on hover is out of reach for a keyboard user unless the same happens on focus. Pairing `mouseover` with `focus` and `mouseout` with `blur` gives both users the same behavior.",
        example: "<div (mouseover)=\"show()\"></div>",
        options: &[],
        check: PointerKeyEvents,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub LABEL_CONTROL {
        id: "label-has-control",
        category: Correctness,
        description: "Reports a `<label>` that names no control: it has no `for` attribute (a bound `[for]` counts) and holds no labelable element (`<input>` that is not hidden, `<select>`, `<textarea>`, `<button>`, `<meter>`, `<output>`, `<progress>`).",
        why: "A label that is tied to no control labels nothing: clicking it does not focus a field and a screen reader does not read it with one. Add `for` with the id of the control, or put the control inside the label.",
        example: "<label>Name</label>",
        options: &[],
        check: LabelControl,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub EMPTY_CONTENT {
        id: "empty-content",
        category: Correctness,
        description: "Reports a link, button or heading with nothing in it and no `aria-label`, `title` or binding to `innerHTML`, `innerText`, `outerHTML` or `textContent`. Text, an interpolation, a block or an element inside counts as content, whatever the element is.",
        why: "A link, button or heading with no accessible name is announced as just \"link\", \"button\" or \"heading\", so a screen reader user cannot tell what it does or where it leads, and a heading adds an empty entry to the outline of the page.",
        example: "<button></button>",
        options: &[],
        check: EmptyContent,
        model: model,
        words: &WORDS,
    }
}

markup_rule! {
    pub NO_AUTOFOCUS {
        id: "no-autofocus",
        category: Correctness,
        description: "Reports an element with an `autofocus` attribute (or a bound `[autofocus]` on an HTML element; on a component it is one of its inputs). One inside a `<dialog>` is left alone.",
        why: "Moving focus when a page loads skips the content before it: a screen reader user hears the page from the middle, and a sighted keyboard user loses the place. Let the user choose where to start.",
        example: "<input autofocus>",
        options: &[],
        check: Autofocus,
        model: model,
        words: &WORDS,
    }
}
