//! Template actions in HTML (`dialect: template`) as the parser sees them: the delimiters and
//! the counting of blocks are shared (`core_syntax::template_actions`), here are the owner of
//! an action (the open element) and the errors.
//!
//! concept: none (glue: template actions in the parser)

use super::P;
use crate::go_template::ACTIONS;

impl P<'_> {
    /// Counts the action at `start..end` (the `}}` at `end`) in the element it stands in.
    pub(super) fn template_action(&mut self, start: usize, end: usize) {
        if !self.balanced {
            return;
        }
        let Some(step) = ACTIONS.step(&self.src[start + 2..end]) else {
            return;
        };
        let owner = self.stack.last().map_or(usize::MAX, |&(_, n)| n);
        if !self.blocks.step(owner, start, step) {
            self.error(
                start,
                end + 2,
                "template block is not opened in this element",
            );
        }
    }

    /// Errors for the blocks still open at the end of the file.
    pub(super) fn template_blocks_closed(&mut self) {
        for at in self.blocks.unclosed() {
            self.error(at, at + 2, "template block is not closed in its element");
        }
    }

    /// The end of the action at `at`, when templates are on and it closes.
    pub(super) fn action_end(&self, at: usize) -> Option<usize> {
        if !self.templates {
            return None;
        }
        ACTIONS.end(self.src, at)
    }

    /// The end of a quoted attribute value (after the closing quote); `from` is after the
    /// opening quote.
    pub(super) fn value_end(&self, from: usize, quote: u8) -> Option<usize> {
        if !self.templates {
            return self.src[from..].find(quote as char).map(|i| from + i + 1);
        }
        ACTIONS.value_end(self.src, from, quote)
    }
}
