//! T-SQL statements: variables, flow control, blocks, procedures.
//!
//! concept: recursive-descent-parser

use super::P;
use super::lexer::T;
use crate::generated::kind as k;

impl P<'_> {
    /// A T-SQL statement of its own syntax; false when `statement_body` should go on.
    pub fn tsql_statement(&mut self) -> bool {
        if self.is_t(0, T::Word) && self.is_t(1, T::Colon) && !self.is_t(2, T::Colon) {
            // A label.
            self.bump_as(k::IDENTIFIER);
            self.bump();
            return true;
        }
        match self.text(0).to_ascii_lowercase().as_str() {
            "declare" if self.is_t(0, T::Word) => self.declare(),
            "set" => self.set_stmt(),
            "if" => self.if_stmt(),
            "while" => self.while_stmt(),
            "begin" => return self.begin_stmt(),
            "exec" | "execute" => self.exec_stmt(),
            "print" | "return" | "throw" => self.keyword_expr(),
            "waitfor" => {
                self.bump_as(k::KEYWORD);
                self.eat_any(&["delay", "time"]);
                self.expression();
            }
            "raiserror" => self.raiserror(),
            "go" | "break" | "continue" => self.bump_as(k::KEYWORD),
            "create" | "alter" if self.routine_ahead() => self.routine(),
            _ => return false,
        }
        true
    }

    /// `PRINT expr`, `RETURN expr`, `THROW a, b, c`.
    fn keyword_expr(&mut self) {
        self.bump_as(k::KEYWORD);
        if self.expression() {
            while self.is_t(0, T::Comma) {
                self.bump();
                self.expression();
            }
        }
    }

    /// `OUTPUT inserted.id, deleted.c [INTO target (cols)]`.
    pub(super) fn output_clause(&mut self) {
        self.bump_as(k::KEYWORD);
        self.indent();
        self.list(|p| {
            if p.at_any(&["inserted", "deleted"]) && p.is_t(1, T::Dot) {
                p.bump_as(k::KEYWORD);
                p.bump();
                if p.at_op("*") {
                    p.bump();
                } else {
                    p.eat_name();
                }
                p.alias();
                true
            } else {
                let ok = p.element();
                p.alias();
                ok
            }
        });
        self.dedent();
        if self.at("into") {
            self.bump_as(k::KEYWORD);
            self.obj_ref();
            if self.is_t(0, T::LParen) {
                self.bracketed(|p| {
                    p.list(|p| p.obj_ref());
                });
            }
        }
    }

    fn raiserror(&mut self) {
        self.bump_as(k::KEYWORD);
        if self.is_t(0, T::LParen) {
            self.bracketed(|p| {
                p.list(|p| p.element());
            });
        }
        if self.at("with") {
            self.bump_as(k::KEYWORD);
            self.list(|p| p.eat_any(&["log", "nowait", "seterror"]));
        }
    }

    /// `DECLARE @a int = 1, @b table (...)`, `DECLARE c CURSOR FOR select`.
    fn declare(&mut self) {
        self.bump_as(k::KEYWORD);
        if self.is_t(0, T::Word) && !self.is_t(0, T::Var) && self.is(1, "cursor") {
            self.eat_name();
            while self.at_any(&[
                "cursor",
                "scroll",
                "insensitive",
                "static",
                "keyset",
                "dynamic",
                "fast_forward",
                "read_only",
                "local",
                "global",
                "for",
            ]) {
                self.bump_as(k::KEYWORD);
            }
            if self.at_query_start() {
                self.query();
            }
            return;
        }
        self.list(|p| {
            if !p.is_t(0, T::Var) {
                return false;
            }
            p.bump();
            p.eat("as");
            if p.at("table") {
                p.bump_as(k::KEYWORD);
                p.generic_bracket_run();
            } else {
                p.type_name();
            }
            if p.at_op("=") {
                p.bump();
                p.expression();
            }
            true
        });
    }

    fn set_stmt(&mut self) {
        // `SET NOCOUNT ON`, `SET TRANSACTION ISOLATION LEVEL ...`: options.
        if !self.is_t(1, T::Var) {
            self.generic();
            return;
        }
        self.bump_as(k::KEYWORD);
        self.indent();
        self.bump();
        if self.is_t(0, T::Op) {
            // `+=`, `=`: an assignment.
            self.bump();
        }
        self.expression();
        self.dedent();
    }

    fn if_stmt(&mut self) {
        self.bump_as(k::KEYWORD);
        self.expression();
        self.nested_statement();
        if self.at("else") {
            self.bump_as(k::KEYWORD);
            self.nested_statement();
        }
    }

    fn while_stmt(&mut self) {
        self.bump_as(k::KEYWORD);
        self.expression();
        self.nested_statement();
    }

    /// A statement inside another (the body of `IF`).
    fn nested_statement(&mut self) {
        if self.eof() || self.is_t(0, T::Semi) {
            return;
        }
        let m = self.mark();
        if self.at_query_start() {
            self.query();
        } else {
            self.statement_body();
        }
        self.tsql_end();
        self.close(k::STATEMENT, m);
    }

    /// The end of a T-SQL statement: a `;` or the start of another statement.
    fn tsql_end(&mut self) {
        if self.eof() || self.is_t(0, T::Semi) || self.at_statement_start() {
            return;
        }
        self.generic();
    }

    pub fn at_statement_start(&self) -> bool {
        self.is_t(0, T::Word) && super::stmt::TSQL_STARTS.iter().any(|w| self.at(w))
    }

    /// `BEGIN TRY`, `BEGIN CATCH`, `BEGIN TRAN`, `BEGIN ... END`; false for a transaction.
    fn begin_stmt(&mut self) -> bool {
        if self.at_any_next(&[
            "tran",
            "transaction",
            "distributed",
            "dialog",
            "conversation",
        ]) {
            self.generic();
            return true;
        }
        self.bump_as(k::KEYWORD);
        let sub = self.at_any(&["try", "catch"]);
        let which = self.text(0).to_ascii_lowercase();
        if sub {
            self.bump_as(k::KEYWORD);
        }
        self.indent();
        self.block_body();
        self.dedent();
        if self.eat("end") && sub {
            self.eat(&which);
            if which == "try" && self.at("begin") && self.is(1, "catch") {
                self.begin_stmt();
            }
        }
        true
    }

    fn at_any_next(&self, ws: &[&str]) -> bool {
        ws.iter().any(|w| self.is(1, w))
    }

    /// Statements up to `END`.
    fn block_body(&mut self) {
        while !self.eof() && !self.at("end") {
            if self.is_t(0, T::Semi) {
                self.bump();
                continue;
            }
            let before = self.pos;
            self.nested_statement();
            if self.pos == before {
                self.bump();
            }
        }
    }

    fn exec_stmt(&mut self) {
        self.bump_as(k::KEYWORD);
        if self.is_t(0, T::Var) && self.is_op(1, "=") {
            self.bump();
            self.bump();
        }
        if self.is_t(0, T::LParen) {
            self.bracketed(|p| {
                p.list(|p| p.element());
            });
            return;
        }
        if !self.obj_ref() {
            return;
        }
        self.indent();
        self.list(|p| {
            if p.is_t(0, T::Var) && p.is_op(1, "=") {
                p.bump();
                let m = p.mark();
                p.bump();
                p.close(k::COMPARISON_OPERATOR, m);
                p.expression_value()
            } else {
                p.expression_value()
            }
        });
        self.dedent();
    }

    /// An argument: an expression, `DEFAULT`, `OUT`, `OUTPUT`.
    fn expression_value(&mut self) -> bool {
        let ok = self.expression() || self.eat("default");
        self.eat_any(&["output", "out"]);
        ok
    }

    fn routine_ahead(&self) -> bool {
        (1..4).any(|i| {
            self.is(i, "procedure")
                || self.is(i, "proc")
                || self.is(i, "function")
                || self.is(i, "trigger")
        }) && !(1..4).any(|i| self.is(i, "table") || self.is(i, "index"))
    }

    /// `CREATE PROCEDURE name @p int, ... AS body`, `CREATE FUNCTION name (...) RETURNS t AS BEGIN ... END`.
    fn routine(&mut self) {
        let m = self.mark();
        let mut function = false;
        while !self.eof() && !self.is_t(0, T::Semi) && !self.at("as") {
            if self.is_t(0, T::Var) {
                self.indent();
                self.list(|p| p.parameter());
                self.dedent();
            } else if self.is_t(0, T::LParen) {
                let signature = self.mark();
                self.bracketed(|p| {
                    p.list(|p| p.parameter());
                });
                if function {
                    self.close(k::FUNCTION_CONTENTS, signature);
                }
            } else if self.at_any(&["procedure", "proc", "function", "trigger"]) {
                function = self.at("function");
                self.bump_as(k::KEYWORD);
                if self.starts_name(0) {
                    self.routine_name();
                }
            } else if self.at("returns") {
                self.bump_as(k::KEYWORD);
                if self.is_t(0, T::Var) {
                    self.bump();
                    self.eat("table");
                } else if self.at("table") && !self.is_t(1, T::LParen) {
                    self.bump_as(k::TYPE_IDENTIFIER);
                } else if !self.type_name() {
                    self.eat("table");
                }
            } else if self.is_t(0, T::Word) {
                let kind = if self.keyword_like(0) {
                    k::KEYWORD
                } else {
                    k::IDENTIFIER
                };
                self.bump_as(kind);
            } else {
                self.bump();
            }
        }
        self.eat("as");
        while !self.eof() && !self.at("go") && !self.is_t(0, T::Semi) {
            let before = self.pos;
            self.nested_statement();
            if self.pos == before {
                break;
            }
            self.eat_t(T::Semi);
        }
        let _ = m;
    }

    fn routine_name(&mut self) {
        let m = self.mark();
        loop {
            let more = self.is_t(1, T::Dot) && self.starts_name(2);
            let kind = if self.is_t(0, T::Quoted) {
                k::QUOTED_IDENTIFIER
            } else {
                k::IDENTIFIER
            };
            self.bump_as(kind);
            if !more {
                break;
            }
            self.bump();
        }
        self.close(k::OBJECT_REFERENCE, m);
    }

    /// `@p int = 1 OUTPUT`.
    fn parameter(&mut self) -> bool {
        if !self.is_t(0, T::Var) {
            return false;
        }
        self.bump();
        self.eat("as");
        self.type_name();
        if self.at_op("=") {
            self.bump();
            self.expression();
        }
        self.eat_any(&["output", "out", "readonly"]);
        true
    }
}
