//! A statement the parser has no rules for (most DDL): its tokens up to the `;`, brackets nested,
//! the words named by the place they stand in: the word after `TABLE`, `TO`, `ON` is a name, the
//! first word of a column definition is a name and the word after it a type, expressions after
//! `DEFAULT` and in `CHECK (...)` parse as expressions, the rest are keywords when the dialect's
//! tables say so.

use super::lexer::T;
use super::P;
use crate::generated::kind as k;

/// Words after which a name follows.
const INTRODUCERS: &[&str] = &[
    "TABLE", "VIEW", "INDEX", "SCHEMA", "DATABASE", "SEQUENCE", "TRIGGER", "TYPE", "ROLE", "USER", "EXTENSION", "DOMAIN",
    "TO", "ON", "INTO", "REFERENCES", "FROM", "JOIN", "TRUNCATE", "OWNER", "CONSTRAINT", "COLUMN", "TABLESPACE",
    "TABLES", "SCHEMAS", "SEQUENCES", "COLLATION", "RULE", "POLICY", "PUBLICATION", "SUBSCRIPTION", "SERVER",
    "WRAPPER", "LANGUAGE", "CAST", "DICTIONARY", "CONFIGURATION", "PARSER", "TEMPLATE", "OPERATOR", "FAMILY", "CLASS", "GROUP",
    "STATISTICS", "TRANSFORM", "EVENT", "SYNONYM", "ASSEMBLY", "CATALOG", "LOGIN", "PLAN", "WORKLOAD", "RESOURCE",
    "ENDPOINT", "ROUTE", "CERTIFICATE", "AGGREGATE", "FUNCTION", "PROCEDURE", "ROUTINE", "EXECUTE", "USE",
    "DESCRIBE", "DESC", "EXISTS", "ONLY", "INHERITS", "METHOD", "NAMES", "CHARSET", "COLLATE",
];

/// Words that are never a name, whatever precedes them.
const NEVER_NAMES: &[&str] = &["DELETE", "UPDATE", "INSERT", "SELECT", "ALL", "CASCADE", "RESTRICT", "NO", "SET", "NULL", "BY", "OF", "ACCESS", "CURRENT", "NONE", "DEPENDS", "OWNED"];

/// Words that sit between an introducer and the name it introduces.
const MODIFIERS: &[&str] = &[
    "IF", "NOT", "EXISTS", "ONLY", "CONCURRENTLY", "OR", "REPLACE", "TABLE", "VIEW", "MATERIALIZED", "TEMP", "TEMPORARY",
    "UNLOGGED", "GLOBAL", "LOCAL", "FOREIGN", "RECURSIVE", "UNIQUE", "CLUSTERED", "NONCLUSTERED", "COLUMNSTORE", "VOLATILE",
    "SCHEMA", "DEFAULT", "PRIVILEGES", "FUNCTION", "PROCEDURE", "SEQUENCE", "TYPE", "COLUMN", "CONSTRAINT", "INDEX",
];

/// Words that open a table constraint: no column name follows them.
const CONSTRAINT_STARTS: &[&str] = &["CONSTRAINT", "PRIMARY", "FOREIGN", "UNIQUE", "CHECK", "INDEX", "KEY", "LIKE", "EXCLUDE", "FULLTEXT", "SPATIAL", "PERIOD"];

/// Words after which an expression follows.
const EXPR_AFTER: &[&str] = &["DEFAULT", "WHERE", "WHEN", "THEN", "ELSE", "IF", "WHILE", "RETURN", "RETURNS", "CALL", "UPDATE"];

#[derive(Default, Clone, Copy)]
struct Walk {
    /// The next word is a name.
    expect_name: bool,
    /// The previous token was a name.
    after_name: bool,
    /// The next word is the first of an item in a list of definitions.
    item_start: bool,
    /// Inside the brackets of a table's columns.
    columns: bool,
    /// Inside the parameters of a function.
    params: bool,
    /// The names that follow are function names.
    function_name: bool,
    /// The statement is a `CREATE TABLE`.
    create_table: bool,
    /// An expression may follow.
    expr: bool,
    /// The statement alters or creates a table.
    table_stmt: bool,
    /// The last name was one of a list that a comma continues.
    name_list: bool,
    /// After `=` or `TO` of a setting: the words are values, not keywords.
    value: bool,
    /// The statement is a `GRANT` or `REVOKE`: `select` there is a privilege.
    grant: bool,
    /// After the columns of a `CREATE TABLE`: `ENGINE = x` style options.
    options: bool,
}

impl P<'_> {
    /// Takes the statement at the cursor token by token; never fails.
    pub fn generic(&mut self) {
        if self.d == super::Dialect::Mysql && self.at("declare") && !self.is_any_after_declare() {
            self.bump_as(k::KEYWORD);
            let w = Walk { item_start: true, params: true, ..Walk::default() };
            self.generic_run(0, w);
            return;
        }
        let grant = self.at_any(&["grant", "revoke", "deny"]);
        let create_table = self.at("create") && (1..6).any(|i| self.is(i, "table"));
        let table_stmt = self.at_any(&["create", "alter"]) && (1..4).any(|i| self.is(i, "table"));
        self.generic_run(0, Walk { create_table, table_stmt, grant, ..Walk::default() });
    }

    /// `DECLARE CONTINUE HANDLER`, `DECLARE c CURSOR`, `DECLARE x CONDITION`: not a variable.
    fn is_any_after_declare(&self) -> bool {
        self.is(1, "continue") || self.is(1, "exit") || self.is(1, "undo") || self.is(2, "cursor") || self.is(2, "condition")
    }

    fn generic_run(&mut self, depth: usize, mut w: Walk) {
        while let Some(t) = self.t(0) {
            if !matches!(t, T::Word | T::Quoted | T::Comma) {
                w.value = false;
            }
            match t {
                T::Semi => break,
                T::RParen | T::RSquare if depth > 0 => break,
                T::RParen => {
                    self.error(self.pos, "unmatched )");
                    self.bump();
                }
                T::LParen => {
                    let columns = w.after_name && w.create_table && depth == 0;
                    let params = w.after_name && w.function_name;
                    self.generic_bracket(depth, columns, params);
                    w.options |= columns;
                    w.expect_name = false;
                    w.after_name = false;
                    w.item_start = false;
                }
                T::Word | T::Quoted => self.generic_word(&mut w, depth),
                T::Comma => {
                    w.item_start = w.columns || w.params;
                    w.expect_name = (w.expect_name || w.name_list) && !w.item_start;
                    w.after_name = false;
                    self.bump();
                }
                T::Cast => {
                    self.bump();
                    self.type_name();
                    w.after_name = false;
                    w.expr = false;
                }
                T::Op => {
                    w.value = self.at_op("=") && !w.expr && !w.columns;
                    w.expr = true;
                    w.expect_name = false;
                    w.after_name = false;
                    self.bump();
                }
                _ => {
                    w.expect_name = false;
                    w.after_name = false;
                    self.bump();
                }
            }
        }
    }

    /// The bracket at the cursor, if any, with generic contents.
    pub fn generic_bracket_run(&mut self) {
        if self.is_t(0, T::LParen) {
            self.generic_bracket(0, false, false);
        }
    }

    fn generic_bracket(&mut self, depth: usize, columns: bool, params: bool) {
        if depth as u32 >= super::MAX_DEPTH {
            self.bump();
            return;
        }
        self.bracketed(|p| {
            if p.at_query_start() {
                p.query();
            } else {
                let w = Walk {
                    item_start: columns || params,
                    columns,
                    params,
                    ..Walk::default()
                };
                p.generic_run(depth + 1, w);
            }
        });
    }

    /// A name (dotted): an object reference. The last part is a function name when `function`.
    fn generic_name(&mut self, function: bool) {
        let m = self.mark();
        let node = if function { k::FUNCTION_NAME } else { k::OBJECT_REFERENCE };
        loop {
            let more = self.is_t(1, T::Dot) && self.starts_name(2);
            let kind = if self.is_t(0, T::Quoted) {
                k::QUOTED_IDENTIFIER
            } else if function && !more {
                k::FUNCTION_IDENTIFIER
            } else {
                k::IDENTIFIER
            };
            self.bump_as(kind);
            if more {
                self.bump();
                if self.is_op(0, "*") {
                    self.bump();
                    break;
                }
            } else {
                break;
            }
        }
        self.close(node, m);
    }

    fn prev_kind(&self) -> Option<T> {
        self.pos.checked_sub(1).and_then(|i| self.code.get(i)).map(|&c| self.lex[c as usize].kind)
    }

    fn generic_word(&mut self, w: &mut Walk, depth: usize) {
        let mut buf = [0u8; 40];
        let up = super::dialect::upper(self.text(0), &mut buf).to_string();
        let quoted = self.is_t(0, T::Quoted);
        let dotted = self.is_t(1, T::Dot) || self.prev_is_dot();
        let word = !quoted;
        // A value of a setting (`SET x = on, off`) and the key before `=`.
        if word && !dotted && !w.columns && !w.params && !w.options && (w.value || self.is_op(1, "=")) {
            self.bump_as(k::WORD);
            return;
        }
        // `ENGINE = InnoDB` after the columns of a table.
        if w.options && word && depth == 0 && self.is_op(1, "=") {
            self.bump_as(k::WORD);
            let m = self.mark();
            self.bump();
            self.close(k::COMPARISON_OPERATOR, m);
            if self.is_t(0, T::Word) {
                self.bump_as(k::WORD);
            }
            return;
        }
        // A query inside a statement (`CREATE VIEW v AS SELECT ...`) parses as one.
        if word && !dotted && !w.grant && (up == "SELECT" || (up == "WITH" && (self.is(1, "recursive") || (self.is_t(1, T::Word) && self.is(2, "as") && (self.is_t(3, T::LParen) || self.is(3, "materialized") || self.is(3, "not")))))) {
            self.query();
            *w = Walk::default();
            return;
        }
        // The first word of a definition is its name; a parameter that is only a type is a type.
        if w.item_start && !(word && CONSTRAINT_STARTS.contains(&up.as_str()) && !w.params) {
            w.item_start = false;
            if w.params && word && super::types::is_type_word(&up) && !self.is_t(1, T::Word) {
                self.type_name();
                return;
            }
            if w.params && word && matches!(up.as_str(), "IN" | "OUT" | "INOUT" | "VARIADIC") {
                self.bump_as(if self.d == super::Dialect::Postgres { k::KEYWORD } else { k::WORD });
                w.item_start = true;
                return;
            }
            self.generic_name(false);
            w.after_name = true;
            return;
        }
        w.item_start = false;
        // `CAST (int AS bool)`: types.
        if word && depth > 0 && !w.columns && !w.params && super::types::is_type_word(&up) && self.is(1, "as") {
            self.type_name();
            return;
        }
        if word && !dotted && w.after_name && super::types::is_type_word(&up) && (w.columns || w.params || depth > 0) {
            self.type_name();
            w.after_name = false;
            return;
        }
        let bare_function = word && super::expr::BARE_FUNCTIONS.contains(&up.as_str()) && !self.is_t(1, T::LParen);
        if bare_function {
            self.bump_as(k::FUNCTION_IDENTIFIER);
            *w = Walk { create_table: w.create_table, table_stmt: w.table_stmt, grant: w.grant, ..Walk::default() };
            return;
        }
        // An item of a bracketed list that is a bare word: a name (`(no)`, `(name)`).
        let bare_item = word && depth > 0 && !w.columns && !w.params && !self.reserved(0) && matches!(self.prev_kind(), Some(T::LParen | T::Comma)) && matches!(self.t(1), Some(T::Comma | T::RParen));
        let name_here = bare_item || w.expect_name && !self.reserved(0) && !MODIFIERS.contains(&up.as_str()) && !INTRODUCERS.contains(&up.as_str()) && !NEVER_NAMES.contains(&up.as_str());
        let calls = (w.expr || (depth > 0 && !w.columns && !w.params)) && !self.reserved(0) || super::expr::BARE_FUNCTIONS.contains(&up.as_str());
        if (word || quoted) && calls && (self.is_t(1, T::LParen) || (w.expr && self.name_then_paren())) && !matches!(up.as_str(), "CHECK" | "ROW" | "THAN") {
            self.chain(false);
            w.expr = false;
            return;
        }
        if quoted || dotted || name_here {
            let function = w.function_name;
            self.generic_name(function);
            w.name_list = true;
            w.expect_name = false;
            w.after_name = true;
            return;
        }
        // Keywords that take an expression.
        if word && up == "CHECK" && self.is_t(1, T::LParen) {
            self.bump_as(k::KEYWORD);
            self.bracketed(|p| {
                p.expression();
            });
            return;
        }
        let kind = if self.keyword_like(0) || self.reserved(0) { k::KEYWORD } else { k::IDENTIFIER };
        self.bump_as(kind);
        if kind == k::KEYWORD {
            if matches!(up.as_str(), "FUNCTION" | "PROCEDURE" | "PROC" | "AGGREGATE" | "ROUTINE") {
                w.function_name = true;
            } else if !MODIFIERS.contains(&up.as_str()) && !INTRODUCERS.contains(&up.as_str()) {
                w.function_name = false;
            }
            if INTRODUCERS.contains(&up.as_str()) || (up == "SET" && self.prev_text_is("character")) {
                w.expect_name = true;
            } else if !MODIFIERS.contains(&up.as_str()) {
                w.expect_name = false;
                w.name_list = false;
            }
            w.after_name = false;
            w.expr = EXPR_AFTER.contains(&up.as_str());
            if matches!(up.as_str(), "RETURNS" | "AS") && self.at_type_word() {
                self.type_name();
            }
            if (w.columns || w.create_table || w.table_stmt) && !w.options && up == "DEFAULT" && self.expr_ahead() {
                self.chain(false);
            }
        } else {
            w.expect_name = false;
            w.after_name = true;
        }
    }

    fn prev_text_is(&self, word: &str) -> bool {
        self.pos.checked_sub(1).and_then(|i| self.code.get(i)).is_some_and(|&c| self.src[self.lex[c as usize].start as usize..self.lex[c as usize].end as usize].eq_ignore_ascii_case(word))
    }

    fn at_type_word(&self) -> bool {
        self.is_t(0, T::Word) && super::types::is_type_word(self.text(0))
    }

    /// Does an expression start at the cursor (not a keyword that ends the definition)?
    fn expr_ahead(&self) -> bool {
        match self.t(0) {
            Some(T::Number | T::Str | T::Var) => true,
            Some(T::LParen) => true,
            Some(T::Op) => self.at_op("-") || self.at_op("+"),
            Some(T::Word) => !self.reserved(0) || self.at_any(&["null", "true", "false", "current_timestamp", "current_date", "case"]),
            _ => false,
        }
    }

    fn prev_is_dot(&self) -> bool {
        self.pos > 0 && self.code.get(self.pos - 1).is_some_and(|&c| self.lex[c as usize].kind == T::Dot)
    }
}
