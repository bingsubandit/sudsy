//! MySQL stored-program statements that have a structure of their own (`IF`); `BEGIN`, `LOOP`,
//! `WHILE` and `REPEAT` stay flat and get their indents in the layout.

use super::lexer::T;
use super::P;
use crate::generated::kind as k;

impl P<'_> {
    /// `IF cond THEN stmts [ELSEIF cond THEN stmts]... [ELSE stmts] END IF`.
    pub fn if_block(&mut self) {
        self.bump_as(k::KEYWORD);
        self.then_branch();
        while self.at("elseif") {
            self.bump_as(k::KEYWORD);
            self.then_branch();
        }
        if self.at("else") {
            self.bump_as(k::KEYWORD);
            self.branch_body();
        }
        if self.at("end") && self.is(1, "if") {
            self.bump_as(k::KEYWORD);
            self.bump_as(k::KEYWORD);
        }
    }

    fn then_branch(&mut self) {
        self.expression();
        if self.at("then") {
            self.bump_as(k::KEYWORD);
        }
        self.branch_body();
    }

    /// Statements up to the next `ELSEIF`, `ELSE` or `END`, indented.
    fn branch_body(&mut self) {
        self.indent();
        while !self.eof() && !self.at_any(&["elseif", "else", "end"]) {
            if self.is_t(0, T::Semi) {
                self.bump();
                continue;
            }
            let before = self.pos;
            let m = self.mark();
            if self.at_query_start() {
                self.query();
            } else {
                self.statement_body();
            }
            self.close(k::STATEMENT, m);
            if self.pos == before {
                self.bump();
            }
        }
        self.dedent();
    }
}
