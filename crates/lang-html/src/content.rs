//! The content model of HTML (what each element may hold): the four shared checks over
//! `data/model.txt`.
//!
//! concept: content-model

use core_rules::content_model::Model;
use core_rules::content_rule;
use core_rules::markup::Spellings;
use std::sync::LazyLock;

pub(crate) fn model() -> &'static Model {
    static MODEL: LazyLock<Model> = LazyLock::new(|| {
        Model::parse(concat!(
            include_str!("../data/model.txt"),
            include_str!("../data/spelling.txt"),
            include_str!("../data/accessibility.txt")
        ))
    });
    &MODEL
}

/// How the dialects write bound attributes (`data/spelling.txt`), parsed apart from the model so
/// that a rule that only reads attributes does not load the model.
pub(crate) fn spellings() -> &'static Spellings {
    static SPELLINGS: LazyLock<Spellings> =
        LazyLock::new(|| Spellings::parse(include_str!("../data/spelling.txt")));
    &SPELLINGS
}

content_rule! {
    pub PERMITTED_CONTENT {
        id: "element-permitted-content",
        category: Correctness,
        description: "Reports an element that its parent does not permit as content (`<li>` outside a \
              list, `<div>` in a `<table>`, a deprecated element in flow content), and one that \
              an ancestor forbids as a descendant (`<a>` in `<a>`, a block element in a \
              `<button>`).",
        why: "The browser repairs invalid nesting on its own, and the repair moves or splits \
              elements (a `<div>` in a `<table>` is moved out). The page then differs from the \
              markup and from what scripts and styles select.\n\
              To fix, move the element to a parent that permits it, or use an element that fits.",
        example: "<table><div></div></table>",
        check: Content,
        model: model,
    }
}

content_rule! {
    pub PERMITTED_ORDER {
        id: "element-permitted-order",
        category: Correctness,
        description: "Reports children that are out of the order the parent requires (`<thead>` and \
              `<tfoot>` before `<tbody>`, `<head>` before `<body>`, `<summary>` first in \
              `<details>`).",
        why: "The browser either reorders them or treats the later one as stray content, so the \
              page shows something other than the order written.\n\
              To fix, put the children in the required order.",
        example: "<table><tbody></tbody><thead></thead></table>",
        check: Order,
        model: model,
    }
}

content_rule! {
    pub PERMITTED_OCCURRENCES {
        id: "element-permitted-occurrences",
        category: Correctness,
        description: "Reports an element that the parent permits at most once (`<head>`, `<body>`, \
              `<title>`, `<caption>`) when it appears again. A `<script>` or `<style>` after \
              `</body>` opens an implied `<head>`, and is reported.",
        why: "The browser keeps the first one and merges or drops the others, so the second \
              `<title>` has no effect and the content in a second `<body>` ends up somewhere \
              else.\n\
              To fix, remove the repeated element, or merge its content into the first.",
        example: "<html><head></head><head></head></html>",
        check: Occurrences,
        model: model,
    }
}

content_rule! {
    pub REQUIRED_CONTENT {
        id: "element-required-content",
        category: Correctness,
        description: "Reports an element that lacks a child it requires: `<title>` in `<head>`, \
              `<summary>` in `<details>`, `<head>` and `<body>` in `<html>`.",
        why: "A page with no `<title>` has no name in tabs, history and search results, and a \
              `<details>` with no `<summary>` shows a label the browser invents.\n\
              To fix, add the missing child.",
        example: "<head></head>",
        check: Required,
        model: model,
    }
}
