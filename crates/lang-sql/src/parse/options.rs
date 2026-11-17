//! Option lists: `(name = value, name (nested), flag)`, the shape of the `WITH (...)` of most
//! T-SQL DDL. A word is a keyword when the dialect knows it, else an identifier.

use super::lexer::T;
use super::P;
use crate::generated::kind as k;

impl P<'_> {
    /// The bracket at the cursor as an option list.
    pub fn options_bracket(&mut self, keywords: bool) {
        self.bracketed(|p| {
            p.list(|p| p.option_item(keywords));
        });
    }

    fn option_item(&mut self, keywords: bool) -> bool {
        let start = self.pos;
        while let Some(t) = self.t(0) {
            match t {
                T::Comma | T::RParen | T::RSquare | T::Semi => break,
                T::LParen => self.options_bracket(keywords),
                T::Word if !keywords && self.at_any(&["true", "false", "null"]) => self.bump_as(k::WORD_LITERAL),
                T::Word => {
                    let kind = if keywords && self.keyword_like(0) { k::KEYWORD } else { k::IDENTIFIER };
                    self.bump_as(kind);
                }
                T::Op if self.at_op("=") => {
                    let m = self.mark();
                    self.bump();
                    self.close(k::COMPARISON_OPERATOR, m);
                }
                _ => self.bump(),
            }
        }
        self.pos > start
    }
}
