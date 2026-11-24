//! How a markup start tag is laid out from options: how many attributes fit on its line before
//! each gets a line of its own, whether the `/` of a self-closing tag has a space before it, and
//! whether a void element is written without the `/`, and whether an omitted end tag is written.
//!
//! concept: tag-layout
//! what: Attribute-per-line limit, slash gap, void style and omitted end tags of markup tags.
//! tools: eslint-plugin-vue max-attributes-per-line, html-eslint attrs-newline
//! reads: option:max_attributes_per_line

use core_config::{ResolvedOptions, opt};

#[derive(Clone, Copy)]
pub struct TagLayout {
    limit: usize,
    slash_space: bool,
    void_omit: bool,
    end_keep: bool,
}

impl TagLayout {
    pub fn of(options: &ResolvedOptions) -> Self {
        let limit = if options.flag(opt::SINGLE_ATTRIBUTE_PER_LINE, false) {
            1
        } else {
            options.uint(opt::MAX_ATTRIBUTES_PER_LINE, 0)
        };
        TagLayout {
            limit,
            slash_space: options.flag(opt::SELF_CLOSING_SPACE, true),
            void_omit: options.text(opt::VOID_ELEMENT_STYLE) == "omit",
            end_keep: options.text(opt::IMPLIED_END_TAGS) == "keep",
        }
    }

    /// A start tag with `attributes` attributes puts each on its own line.
    pub fn per_line(&self, attributes: usize) -> bool {
        self.limit > 0 && attributes > self.limit
    }

    /// A void element is written without the `/`.
    pub fn slashless(&self, void: bool) -> bool {
        self.void_omit && void
    }

    /// An element with no end tag in the source, which runs to `end`, is written without one
    /// when it is still open at the end of `src`.
    pub fn keeps_omitted_end(&self, src: &str, end: usize) -> bool {
        self.end_keep && end >= src.trim_end().len()
    }

    /// A space stands before the `/>` of a self-closing element (`void`: it is a void one).
    pub fn slash_gap(&self, self_closing: bool, void: bool) -> bool {
        self_closing && !self.slashless(void) && self.slash_space
    }
}
