//! Queries: `WITH`, `SELECT` and its clauses, set operations, joins.

use super::lexer::T;
use super::{Dialect, P};
use crate::generated::kind as k;

const SET_OPS: &[&str] = &["union", "intersect", "except", "minus"];

impl P<'_> {
    /// A query: `WITH ...`, a select, set operations, a bracketed query, `VALUES`.
    pub fn query(&mut self) {
        if self.at("with") {
            self.with_query();
        } else {
            self.set_expr();
        }
    }

    fn with_query(&mut self) {
        let m = self.mark();
        self.bump_as(k::KEYWORD);
        self.eat("recursive");
        self.list(|p| p.cte());
        if self.at_any(&["insert", "update", "delete", "merge", "replace"]) {
            self.statement_body();
        } else {
            self.set_expr();
        }
        self.close(k::WITH_COMPOUND_STATEMENT, m);
    }

    fn cte(&mut self) -> bool {
        let m = self.mark();
        if !self.eat_name() {
            return false;
        }
        if self.is_t(0, T::LParen) {
            self.bracketed(|p| {
                p.list(|p| p.eat_name());
            });
        }
        self.eat("as");
        self.eat("materialized");
        if self.at("not") && self.is(1, "materialized") {
            self.eat_seq(&["not", "materialized"]);
        }
        if self.is_t(0, T::LParen) {
            self.bracketed(|p| {
                if p.at_any(&["insert", "update", "delete"]) {
                    p.statement_body();
                } else {
                    p.query();
                }
            });
        }
        // `SEARCH DEPTH FIRST BY id SET col`, `CYCLE id SET c USING path`.
        while self.at_any(&["search", "cycle"]) {
            self.bump_as(k::KEYWORD);
            let mut list = false;
            while self.is_t(0, T::Word) && !self.at_any(&["search", "cycle", "select", "insert", "update", "delete"]) {
                if self.at_any(&["depth", "breadth", "first", "by", "set", "using", "default", "to"]) {
                    list = self.at("by") || self.at("cycle");
                    self.bump_as(k::KEYWORD);
                } else {
                    self.bump_as(k::IDENTIFIER);
                }
                if list && self.is_t(0, T::Comma) && self.is_t(1, T::Word) && (self.is(2, "set") || self.is_t(2, T::Comma)) {
                    self.bump();
                }
            }
        }
        self.close(k::COMMON_TABLE_EXPRESSION, m);
        true
    }

    fn set_expr(&mut self) {
        let m = self.mark();
        let mut sets = false;
        self.query_term(false);
        while self.at_any(SET_OPS) && self.depth < super::MAX_DEPTH {
            sets = true;
            let o = self.mark();
            self.bump_as(k::KEYWORD);
            self.eat_any(&["all", "distinct"]);
            if self.at_any(&["by"]) {
                // `UNION BY NAME`
                self.bump_as(k::KEYWORD);
            }
            self.close(k::SET_OPERATOR, o);
            self.query_term(true);
        }
        if sets {
            self.tail_clauses();
            self.close(k::SET_EXPRESSION, m);
        }
    }

    /// One query of a set operation: a select, `VALUES`, or a bracketed query.
    fn query_term(&mut self, in_set: bool) {
        if self.is_t(0, T::LParen) {
            self.bracketed(|p| p.query());
        } else if self.at("values") {
            self.values_clause();
            if !in_set {
                self.tail_clauses();
            }
        } else {
            self.select_stmt(in_set);
        }
    }

    pub fn values_clause(&mut self) {
        self.bump_as(k::KEYWORD);
        self.list(|p| {
            if p.is_t(0, T::LParen) {
                p.bracketed(|p| {
                    p.list(|p| p.element() || p.eat_kw_value());
                });
                true
            } else {
                false
            }
        });
    }

    /// `DEFAULT` in a values list.
    fn eat_kw_value(&mut self) -> bool {
        self.eat("default")
    }

    fn select_stmt(&mut self, in_set: bool) {
        let m = self.mark();
        if !self.at("select") {
            // Not a query: the caller's tail is an error.
            if !self.eof() && !in_set {
                self.error(self.pos, "expected a query");
            }
            return;
        }
        self.select_clause();
        if self.at("into") && !self.is_t(1, T::LParen) {
            self.bump_as(k::KEYWORD);
            while self.d == Dialect::Postgres && self.at_any(&["temp", "temporary", "unlogged", "table"]) {
                self.bump_as(k::KEYWORD);
            }
            self.obj_ref();
        }
        if self.at("from") {
            self.from_clause();
        }
        self.body_clauses();
        if !in_set && !self.at_any(SET_OPS) {
            self.tail_clauses();
        }
        self.close(k::SELECT_STATEMENT, m);
    }

    fn select_clause(&mut self) {
        const MYSQL_MODIFIERS: &[&str] = &[
            "high_priority", "straight_join", "sql_small_result", "sql_big_result", "sql_buffer_result", "sql_no_cache",
            "sql_cache", "sql_calc_found_rows",
        ];
        let m = self.mark();
        self.bump_as(k::KEYWORD);
        // Modifiers.
        let mm = self.mark();
        if self.at_any(&["distinct", "all"]) && !self.is_t(1, T::Comma) {
            self.bump_as(k::KEYWORD);
            self.close(k::SELECT_CLAUSE_MODIFIER, mm);
            if self.at("on") && self.is_t(1, T::LParen) {
                self.bump_as(k::KEYWORD);
                self.bracketed(|p| {
                    p.list(|p| p.expression());
                });
            }
        } else if self.at("top") && self.d == Dialect::Tsql {
            self.bump_as(k::KEYWORD);
            if self.is_t(0, T::LParen) {
                self.bracketed(|p| {
                    p.expression();
                });
            } else {
                self.bump();
            }
            self.eat("percent");
            if self.at("with") && self.is(1, "ties") {
                self.eat_seq(&["with", "ties"]);
            }
            self.close(k::SELECT_CLAUSE_MODIFIER, mm);
        }
        while self.d == Dialect::Mysql && self.at_any(MYSQL_MODIFIERS) {
            let mm = self.mark();
            self.bump_as(k::KEYWORD);
            self.close(k::SELECT_CLAUSE_MODIFIER, mm);
        }
        self.indent();
        self.list(|p| p.select_element());
        self.dedent();
        self.close(k::SELECT_CLAUSE, m);
    }

    fn select_element(&mut self) -> bool {
        let m = self.mark();
        if self.d == Dialect::Tsql && (self.is_t(0, T::Var) || self.at_name()) && self.is_op(1, "=") {
            // `@v = expr`, `alias = expr`
            self.bump();
            self.bump();
            self.expression();
            self.close(k::SELECT_CLAUSE_ELEMENT, m);
            return true;
        }
        if self.at_op("*") {
            let r = self.mark();
            self.bump();
            self.close(k::OBJECT_REFERENCE, r);
        } else if !self.element() {
            return false;
        }
        self.alias();
        self.close(k::SELECT_CLAUSE_ELEMENT, m);
        true
    }

    fn from_clause(&mut self) {
        let m = self.mark();
        self.bump_as(k::KEYWORD);
        self.list(|p| p.from_expression());
        self.close(k::FROM_CLAUSE, m);
    }

    pub fn from_expression(&mut self) -> bool {
        let m = self.mark();
        if self.is_t(0, T::LParen) && (!self.at_query_start() || self.nested_join_bracket()) {
            // A bracketed join: `(a JOIN b ON ...)`.
            self.bracketed(|p| {
                p.from_element();
                while p.at_join() {
                    p.join();
                }
            });
            while self.at_join() {
                self.join();
            }
            self.close(k::FROM_EXPRESSION, m);
            return true;
        }
        let cp = self.cp();
        self.implicit();
        if !self.from_element() {
            self.rewind(cp);
            return false;
        }
        self.dedent();
        while self.at_join() {
            self.join();
        }
        self.close(k::FROM_EXPRESSION, m);
        true
    }

    /// `((select ..) AS a JOIN ..)`: a bracket whose first item is a bracketed query with an alias.
    fn nested_join_bracket(&self) -> bool {
        if !self.is_t(1, T::LParen) {
            return false;
        }
        let mut depth = 0;
        let mut n = 1;
        while let Some(t) = self.t(n) {
            match t {
                T::LParen => depth += 1,
                T::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            n += 1;
        }
        self.is_t(n + 1, T::Word) && !self.at_any_at(n + 1, &["union", "intersect", "except", "minus", "order", "limit", "offset", "fetch"])
    }

    fn at_join(&self) -> bool {
        let mut n = 0;
        if self.at("natural") && self.d != Dialect::Tsql {
            n += 1;
        }
        if self.is_t(n, T::Word) && ["inner", "left", "right", "full", "cross", "outer"].iter().any(|w| self.is(n, w)) {
            n += 1;
            if self.is(n, "outer") {
                n += 1;
            }
            if self.is_t(n, T::Word) && ["semi", "anti", "asof"].iter().any(|w| self.is(n, w)) {
                n += 1;
            }
        }
        if self.at_any(&["straight_join"]) {
            return true;
        }
        if self.d == Dialect::Tsql && self.at_any(&["cross", "outer"]) && self.is(1, "apply") {
            return true;
        }
        self.is(n, "join")
    }

    fn join(&mut self) {
        let m = self.mark();
        let mut unconditional = self.at_any(&["cross", "natural"]) || self.at("straight_join") && false;
        while self.at_any(&["natural", "inner", "left", "right", "full", "cross", "outer", "semi", "anti", "asof"]) {
            self.bump_as(k::KEYWORD);
        }
        if self.at("apply") {
            unconditional = true;
            self.bump_as(k::KEYWORD);
        } else if !self.eat_any(&["join", "straight_join"]) {
            self.error(self.pos, "expected JOIN");
        }
        self.indent();
        self.from_element();
        while self.at_join() {
            break;
        }
        self.dedent();
        if !unconditional {
            if self.at("on") {
                self.indent();
                let o = self.mark();
                self.bump_as(k::KEYWORD);
                self.implicit();
                self.bracketed_expression();
                self.dedent();
                self.close(k::JOIN_ON_CONDITION, o);
                self.dedent();
            } else if self.at("using") {
                self.indent();
                self.bump_as(k::KEYWORD);
                self.indent();
                self.bracketed(|p| {
                    p.list(|p| p.eat_name());
                });
                self.dedent();
                self.dedent();
            }
        }
        self.close(k::JOIN_CLAUSE, m);
    }

    /// A table in a `FROM`: a name, a bracketed query or table, a table function, with its alias.
    pub fn from_element(&mut self) -> bool {
        let m = self.mark();
        let t = self.mark();
        if self.at("lateral") && !self.is_t(1, T::Dot) {
            self.bump_as(k::KEYWORD);
        }
        if self.is_t(0, T::LParen) {
            self.bracketed(|p| {
                if p.at_query_start() {
                    p.query();
                } else {
                    p.from_expression();
                }
            });
        } else if self.d == Dialect::Tsql && self.at_any(&["openquery", "openrowset", "openxml", "openjson", "containstable", "freetexttable", "semanticsimilaritytable"]) && self.is_t(1, T::LParen) {
            // A rowset function: a keyword, then a bracket of arguments.
            self.bump_as(k::KEYWORD);
            self.bracketed(|p| {
                p.list(|p| p.rowset_argument());
            });
        } else if self.at("dual") {
            let o = self.mark();
            self.bump_as(k::IDENTIFIER);
            self.close(k::OBJECT_REFERENCE, o);
        } else if self.starts_name(0) {
            if self.name_call_ahead() {
                self.expression_call();
            } else {
                self.obj_ref();
            }
        } else {
            return false;
        }
        self.from_element_rest(m, t)
    }

    fn from_element_rest(&mut self, m: usize, t: usize) -> bool {
        if self.pos == m {
            return false;
        }
        if self.d == Dialect::Postgres && self.at("with") && self.is(1, "ordinality") {
            self.bump_as(k::KEYWORD);
            self.bump_as(k::KEYWORD);
        }
        self.close(k::TABLE_EXPRESSION, t);
        self.alias_in(true);
        self.table_suffix();
        self.close(k::FROM_EXPRESSION_ELEMENT, m);
        true
    }

    /// An argument of a rowset function: expressions, with the option words as keywords.
    fn rowset_argument(&mut self) -> bool {
        let start = self.pos;
        while !self.eof() && !self.is_t(0, T::Comma) && !self.is_t(0, T::RParen) {
            if self.is_t(0, T::Word) && self.keyword_like(0) && !self.is_t(1, T::LParen) && !self.is_t(1, T::Dot) && !self.is_t(0, T::Quoted) {
                self.bump_as(k::KEYWORD);
            } else if !self.element() {
                self.bump();
            }
        }
        self.pos > start
    }

    fn name_call_ahead(&self) -> bool {
        let mut n = 0;
        while self.starts_name(n) && self.is_t(n + 1, T::Dot) {
            n += 2;
        }
        self.starts_name(n) && self.is_t(n + 1, T::LParen)
    }

    fn expression_call(&mut self) {
        self.chain(true);
    }

    /// A table hint: `NOLOCK`, `INDEX(ix)`.
    pub fn table_hint(&mut self) -> bool {
        if !self.is_t(0, T::Word) {
            return false;
        }
        self.bump_as(k::KEYWORD);
        if self.is_t(0, T::LParen) {
            self.bracketed(|p| {
                p.list(|p| p.element());
            });
        }
        true
    }

    /// What can follow a table and its alias: `TABLESAMPLE`, hints, `WITH (NOLOCK)`.
    fn table_suffix(&mut self) {
        // MySQL: `PARTITION (p0)`, `FORCE INDEX (i)`.
        while self.d == Dialect::Mysql && ((self.at("partition") && self.is_t(1, T::LParen)) || (self.at_any(&["force", "use", "ignore"]) && self.at_any_at(1, &["index", "key"]))) {
            while self.is_t(0, T::Word) {
                self.bump_as(k::KEYWORD);
            }
            self.bracketed(|p| {
                p.list(|p| p.obj_ref());
            });
        }
        if self.at("tablesample") {
            self.bump_as(k::KEYWORD);
            self.eat_any(&["bernoulli", "system"]);
            self.bracketed(|p| {
                p.element();
            });
            if self.at("repeatable") {
                self.bump_as(k::KEYWORD);
                self.bracketed(|p| {
                    p.element();
                });
            }
        }
        if self.at("with") && self.is_t(1, T::LParen) && self.d == Dialect::Tsql {
            self.bump_as(k::KEYWORD);
            self.bracketed(|p| {
                p.list(|p| p.table_hint());
            });
        }
    }

    pub fn where_clause(&mut self) {
        if self.is(1, "current") && self.is(2, "of") {
            // `WHERE CURRENT OF cursor`: no clause.
            for _ in 0..3 {
                self.bump_as(k::KEYWORD);
            }
            self.obj_ref();
            return;
        }
        let m = self.mark();
        self.bump_as(k::KEYWORD);
        self.implicit();
        self.bracketed_expression();
        self.dedent();
        self.close(k::WHERE_CLAUSE, m);
    }

    /// `GROUP BY`, `HAVING`, `WINDOW`, `QUALIFY`.
    fn body_clauses(&mut self) {
        if self.at("where") {
            self.where_clause();
        }
        if self.at("group") && self.is(1, "by") {
            let m = self.mark();
            self.eat_seq(&["group", "by"]);
            self.indent();
            if self.at("all") {
                self.bump_as(k::KEYWORD);
            } else {
                self.list(|p| p.element());
            }
            if self.at("with") && self.at_any_at(1, &["rollup", "cube", "totals"]) {
                self.bump_as(k::KEYWORD);
                self.bump_as(k::KEYWORD);
            }
            self.dedent();
            self.close(k::GROUPBY_CLAUSE, m);
        }
        if self.at("having") {
            let m = self.mark();
            self.bump_as(k::KEYWORD);
            self.implicit();
            self.bracketed_expression();
            self.dedent();
            self.close(k::HAVING_CLAUSE, m);
        }
        if self.at("window") && self.is_t(1, T::Word) && self.is(2, "as") {
            self.bump_as(k::KEYWORD);
            self.indent();
            self.list(|p| {
                let ok = p.eat_name();
                if ok {
                    p.eat("as");
                    p.window_spec();
                }
                ok
            });
            self.dedent();
        }
        if self.at("qualify") {
            self.bump_as(k::KEYWORD);
            self.implicit();
            self.expression();
            self.dedent();
        }
    }

    /// `ORDER BY`, `LIMIT`, `OFFSET`, `FETCH`, `FOR UPDATE`: what ends a query.
    pub fn tail_clauses(&mut self) {
        if self.at("order") && self.is(1, "by") {
            self.order_by();
        }
        if self.at("limit") {
            let m = self.mark();
            self.bump_as(k::KEYWORD);
            self.indent();
            self.eat("all");
            self.element();
            if self.is_t(0, T::Comma) {
                self.bump();
                self.element();
            } else if self.at("offset") {
                self.bump_as(k::KEYWORD);
                self.element();
            }
            self.dedent();
            self.close(k::LIMIT_CLAUSE, m);
        }
        self.offset_fetch();
        if self.at("for") && self.is_t(1, T::Word) {
            self.bump_as(k::KEYWORD);
            while self.at_any(&["update", "no", "key", "share", "of", "nowait", "skip", "locked", "read", "only", "xml", "json", "browse", "path", "auto", "raw", "explicit"]) {
                self.bump_as(k::KEYWORD);
            }
            // T-SQL `FOR XML PATH ('r'), TYPE, ROOT ('x')`.
            while self.d == Dialect::Tsql && !self.eof() {
                if self.is_t(0, T::LParen) {
                    self.bracketed(|p| {
                        p.list(|p| p.element());
                    });
                } else if self.is_t(0, T::Comma) && self.is_t(1, T::Word) {
                    self.bump();
                } else if self.is_t(0, T::Word) && !self.at_statement_start() && !self.at_any(&["option", "union", "except", "intersect"]) {
                    self.bump_as(k::KEYWORD);
                } else {
                    break;
                }
            }
        }
    }

    /// `OFFSET n ROWS FETCH NEXT n ROWS ONLY`.
    fn offset_fetch(&mut self) {
        while self.at_any(&["offset", "fetch"]) {
            self.bump_as(k::KEYWORD);
            self.eat_any(&["first", "next"]);
            self.element();
            self.eat_any(&["row", "rows", "percent", "only"]);
            self.eat_any(&["only"]);
            if self.at("with") && self.is(1, "ties") {
                self.eat_seq(&["with", "ties"]);
            }
        }
    }

    pub fn order_by(&mut self) {
        let m = self.mark();
        self.eat_seq(&["order", "by"]);
        self.indent();
        self.list(|p| {
            if !p.element() {
                return false;
            }
            p.eat_any(&["asc", "desc"]);
            if p.at("nulls") && p.is_t(1, T::Word) {
                p.bump_as(k::KEYWORD);
                p.bump_as(k::KEYWORD);
            }
            true
        });
        // T-SQL: the paging belongs to the clause.
        if self.d == Dialect::Tsql {
            self.offset_fetch();
        }
        self.dedent();
        self.close(k::ORDERBY_CLAUSE, m);
    }
}
