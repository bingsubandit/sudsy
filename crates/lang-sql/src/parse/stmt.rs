//! The file and its statements.

use super::lexer::T;
use super::{Dialect, P};
use crate::generated::kind as k;

/// Words that start a statement in T-SQL, where statements need no terminator.
pub(super) const TSQL_STARTS: &[&str] = &[
    "select", "insert", "update", "delete", "merge", "with", "declare", "set", "if", "while", "begin", "end", "exec",
    "execute", "print", "return", "create", "alter", "drop", "truncate", "use", "go", "break", "continue", "goto",
    "throw", "raiserror", "open", "close", "fetch", "deallocate", "commit", "rollback", "save", "grant", "revoke",
    "deny", "backup", "restore", "dbcc", "waitfor", "else", "readtext", "writetext", "updatetext", "bulk", "enable",
    "disable", "kill", "checkpoint", "reconfigure", "shutdown", "setuser", "sp_executesql",
];

pub(super) fn file(p: &mut P) {
    while !p.eof() {
        if p.is_t(0, T::Semi) {
            p.bump();
            continue;
        }
        let before = p.pos;
        if p.is_t(0, T::Data) {
            p.bump_as(k::WORD);
            continue;
        }
        if p.d == Dialect::Mysql && p.at("delimiter") {
            // `DELIMITER ~`: the new delimiter stands outside the statement.
            let m = p.mark();
            p.bump_as(k::KEYWORD);
            p.close(k::STATEMENT, m);
            if p.is_t(0, T::Op) {
                p.bump();
            }
            continue;
        }
        statement(p);
        if p.pos == before {
            // Nothing took the token: it is an error of its own.
            let m = p.mark();
            p.error(p.pos, "syntax error");
            p.bump();
            p.close(core_syntax::SyntaxKind::ERROR, m);
        }
    }
}

fn statement(p: &mut P) {
    let m = p.mark();
    if p.at_query_start() {
        p.query();
    } else {
        p.statement_body();
    }
    end_statement(p);
    p.close(k::STATEMENT, m);
}

/// What follows a statement is the end of it, or the start of the next one in T-SQL.
fn end_statement(p: &mut P) {
    if p.eof() || p.is_t(0, T::Semi) {
        return;
    }
    if p.d == Dialect::Tsql && p.at_any(TSQL_STARTS) {
        return;
    }
    // What is left is taken as the rest of the statement, by the place of its words.
    p.generic();
}

impl P<'_> {
    /// A statement that is not a query: dispatches on its first words.
    pub fn statement_body(&mut self) {
        if self.d == Dialect::Tsql && self.tsql_statement() {
            return;
        }
        match self.text(0).to_ascii_lowercase().as_str() {
            "insert" | "replace" if self.is_t(0, T::Word) => self.insert(),
            "update" => self.update(),
            "delete" => self.delete(),
            "if" if self.d == Dialect::Mysql => self.if_block(),
            "set" if self.d == Dialect::Mysql && self.is_assignment() => self.set_assign(),
            _ if self.is_t(0, T::Word) && !self.keyword_like(0) && !self.is_t(1, T::Colon) => self.skip_statement(),
            _ => self.generic(),
        }
    }

    /// `SET a = 1`, `SET @a := 1`, `SET new.c = 1` (not `SET NAMES x`, `SET GLOBAL x = 1`).
    fn is_assignment(&self) -> bool {
        matches!(self.t(1), Some(T::Word | T::Var)) && (self.is_op(2, "=") || self.is_op(2, ":=") || self.is_t(2, T::Dot))
    }

    fn set_assign(&mut self) {
        self.bump_as(k::KEYWORD);
        self.list(|p| {
            if p.is_t(0, T::Var) {
                p.bump();
            } else if p.is_t(1, T::Dot) {
                p.obj_ref();
            } else {
                p.bump_as(k::WORD);
            }
            if !p.is_t(0, T::Op) {
                return false;
            }
            p.element_op();
            p.expression()
        });
    }

    fn insert(&mut self) {
        let m = self.mark();
        self.bump_as(k::KEYWORD);
        self.eat_any(&["ignore", "overwrite"]);
        self.eat("into");
        self.eat("table");
        self.obj_ref();
        if self.is_t(0, T::LParen) && !self.paren_query_ahead() {
            self.bracketed(|p| {
                p.list(|p| p.obj_ref());
            });
        }
        if self.at("values") {
            self.values_clause();
        } else if self.at_query_start() {
            self.query();
        } else if self.at("default") {
            self.eat_seq(&["default", "values"]);
        }
        self.dml_tail();
        let _ = m;
    }

    fn paren_query_ahead(&self) -> bool {
        self.is_t(0, T::LParen) && (self.is(1, "select") || self.is(1, "with"))
    }

    /// `ON CONFLICT ...`, `RETURNING ...`: what can follow a DML statement.
    fn dml_tail(&mut self) {
        while !self.eof() && !self.is_t(0, T::Semi) {
            if self.at("returning") {
                self.bump_as(k::KEYWORD);
                self.list(|p| {
                    if p.at_op("*") {
                        p.bump();
                        return true;
                    }
                    let ok = p.element();
                    p.alias();
                    ok
                });
            } else if self.at("on") {
                self.bump_as(k::KEYWORD);
                while !self.eof() && !self.is_t(0, T::Semi) && !self.at_any(&["set", "returning"]) {
                    if self.is_t(0, T::LParen) {
                        self.bracketed(|p| {
                            p.list(|p| p.element());
                        });
                    } else if self.is_t(0, T::Word) {
                        let kind = if self.keyword_like(0) { k::KEYWORD } else { k::IDENTIFIER };
                        self.bump_as(kind);
                    } else {
                        break;
                    }
                }
            } else if self.at("set") {
                self.set_clause();
            } else if self.at("where") {
                self.where_clause();
            } else {
                break;
            }
        }
    }

    fn set_clause(&mut self) {
        self.bump_as(k::KEYWORD);
        self.indent();
        self.list(|p| {
            if p.is_t(0, T::LParen) {
                p.bracketed(|p| {
                    p.list(|p| p.obj_ref());
                });
            } else if !p.obj_ref() {
                return false;
            }
            if p.is_t(0, T::Op) {
                p.element_op();
            }
            p.element()
        });
        self.dedent();
    }

    /// The `=` of an assignment.
    fn element_op(&mut self) {
        let m = self.mark();
        self.bump();
        self.close(k::COMPARISON_OPERATOR, m);
    }

    fn update(&mut self) {
        self.bump_as(k::KEYWORD);
        self.indent();
        self.eat_any(&["only", "low_priority", "ignore"]);
        self.list(|p| {
            let ok = p.obj_ref();
            if !p.at("set") {
                p.alias();
            }
            ok
        });
        self.dedent();
        if self.at("set") {
            self.set_clause();
        }
        if self.at("from") {
            let m = self.mark();
            self.bump_as(k::KEYWORD);
            self.list(|p| p.from_expression());
            self.close(k::FROM_CLAUSE, m);
        }
        self.dml_tail();
    }

    fn delete(&mut self) {
        self.bump_as(k::KEYWORD);
        if self.at("from") {
            let m = self.mark();
            self.bump_as(k::KEYWORD);
            self.list(|p| p.from_expression());
            self.close(k::FROM_CLAUSE, m);
        }
        if self.at("using") {
            self.bump_as(k::KEYWORD);
            self.list(|p| p.from_expression());
        }
        self.dml_tail();
    }
}
