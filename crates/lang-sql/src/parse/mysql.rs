//! MySQL stored-program statements that have a structure of their own (`IF`, `CASE`, labelled
//! blocks): the words are the frames of `layout::SPEC`, the shared block parse does the rest.
//!
//! concept: none (glue: the parser as a block-parse cursor)

use super::P;
use super::lexer::T;
use crate::generated::kind as k;
use crate::layout::SPEC;
use core_syntax::block_parse::{self, Cursor};

impl Cursor for P<'_> {
    fn text_at(&self, n: usize) -> &str {
        self.text(n)
    }

    fn at_end(&self) -> bool {
        self.eof()
    }

    fn separator_at(&self, n: usize) -> bool {
        self.is_t(n, T::Semi)
    }

    fn at_close(&self) -> bool {
        self.is_t(0, T::RParen)
    }

    fn at_label(&self) -> bool {
        self.is_t(1, T::Colon)
    }

    fn bump_label(&mut self) {
        self.bump_as(k::IDENTIFIER);
        self.bump();
    }

    fn bump_keyword(&mut self) {
        self.bump_as(k::KEYWORD);
    }

    fn bump_any(&mut self) {
        self.bump();
    }

    fn expression(&mut self) {
        P::expression(self);
    }

    fn statement(&mut self, bracketed: bool) {
        self.in_brackets += usize::from(bracketed);
        let m = self.mark();
        if self.at_query_start() {
            self.query();
        } else {
            self.statement_body();
        }
        self.close(k::STATEMENT, m);
        self.in_brackets -= usize::from(bracketed);
    }

    fn position(&self) -> usize {
        self.pos
    }
}

impl P<'_> {
    /// A block statement of `SPEC` (flat block, `IF`, `CASE`); false when none opens here.
    pub fn block_statement(&mut self) -> bool {
        block_parse::statement(self, &SPEC.blocks)
    }
}
