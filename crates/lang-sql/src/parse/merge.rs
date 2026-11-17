//! `MERGE INTO target USING source ON cond WHEN [NOT] MATCHED [BY ..] [AND cond] THEN action ..`.

use super::lexer::T;
use super::{Dialect, P};
use crate::generated::kind as k;

impl P<'_> {
    pub fn merge(&mut self) {
        self.bump_as(k::KEYWORD);
        if self.d == Dialect::Tsql && self.at("top") {
            self.bump_as(k::KEYWORD);
            self.bracketed(|p| {
                p.expression();
            });
            self.eat("percent");
        }
        self.eat("into");
        self.indent();
        self.obj_ref();
        if self.d == Dialect::Tsql && self.at("with") && self.is_t(1, T::LParen) {
            self.bump_as(k::KEYWORD);
            self.bracketed(|p| {
                p.list(|p| p.table_hint());
            });
        }
        self.alias();
        self.dedent();
        if self.at("using") {
            self.bump_as(k::KEYWORD);
            self.indent();
            let m = self.mark();
            self.from_element();
            self.dedent();
            if self.at("on") {
                self.indent();
                self.bump_as(k::KEYWORD);
                self.implicit();
                self.bracketed_expression();
                self.dedent();
                self.dedent();
            }
            let _ = m;
        }
        while self.at("when") {
            self.merge_when();
        }
        self.dml_tail();
    }

    fn merge_when(&mut self) {
        self.bump_as(k::KEYWORD);
        self.eat("not");
        self.eat("matched");
        if self.at("by") {
            self.bump_as(k::KEYWORD);
            self.eat_any(&["source", "target"]);
        }
        if self.at("and") {
            self.bump_as(k::KEYWORD);
            self.expression();
        }
        if !self.eat("then") {
            return;
        }
        self.indent();
        if self.at("update") {
            self.bump_as(k::KEYWORD);
            self.indent();
            if self.at("set") {
                self.set_clause();
            }
            self.dedent();
        } else if self.at("insert") {
            self.bump_as(k::KEYWORD);
            self.indent();
            if self.is_t(0, T::LParen) {
                self.bracketed(|p| {
                    p.list(|p| p.obj_ref());
                });
            }
            self.dedent();
            if self.at("values") {
                self.values_clause();
            } else if self.at("default") {
                self.eat_seq(&["default", "values"]);
            }
        } else if self.at("delete") {
            self.bump_as(k::KEYWORD);
        } else if self.at("do") {
            self.bump_as(k::KEYWORD);
            self.eat("nothing");
        }
        self.dedent();
    }
}
