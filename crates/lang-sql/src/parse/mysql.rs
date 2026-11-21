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

    fn at_separator(&self) -> bool {
        self.is_t(0, T::Semi)
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

    fn statement(&mut self) {
        let m = self.mark();
        if self.at_query_start() {
            self.query();
        } else {
            self.statement_body();
        }
        self.close(k::STATEMENT, m);
    }

    fn position(&self) -> usize {
        self.pos
    }
}

impl P<'_> {
    /// A statement with arms (`IF`, `CASE`): the frame of `SPEC` that opens at the cursor.
    pub fn arm_block(&mut self) {
        if let Some(f) = SPEC
            .frames
            .iter()
            .find(|f| self.is(0, f.open) && !f.arms.is_empty())
        {
            block_parse::arm_block(self, f, SPEC.end);
        }
    }

    /// A flat block opener, labelled or not; false when none opens here.
    pub fn block_opener(&mut self) -> bool {
        block_parse::block_opener(self, SPEC.frames, SPEC.end)
    }
}
