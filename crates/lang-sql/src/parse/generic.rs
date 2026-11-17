//! A statement the parser has no rules for (most DDL): its tokens up to the `;`, brackets nested,
//! the words named by the place they stand in: the word after `TABLE`, `TO`, `ON` is a name, the
//! first word of a column definition is a name and the word after it a type, the rest are
//! keywords when the dialect's tables say so.

use super::lexer::T;
use super::P;
use crate::generated::kind as k;

/// Words after which a name follows.
const INTRODUCERS: &[&str] = &[
    "TABLE", "VIEW", "INDEX", "SCHEMA", "DATABASE", "SEQUENCE", "TRIGGER", "TYPE", "ROLE", "USER", "EXTENSION", "DOMAIN",
    "TO", "ON", "INTO", "REFERENCES", "FROM", "JOIN", "TRUNCATE", "OWNER", "RENAME", "CONSTRAINT", "COLUMN", "TABLESPACE",
    "TABLES", "SCHEMAS", "SEQUENCES", "OWNED", "ONLY", "COLLATION", "RULE", "POLICY", "PUBLICATION", "SUBSCRIPTION", "SERVER",
    "WRAPPER", "LANGUAGE", "CAST", "DICTIONARY", "CONFIGURATION", "PARSER", "TEMPLATE", "OPERATOR", "FAMILY", "CLASS", "GROUP",
    "STATISTICS", "TRANSFORM", "EVENT", "MATERIALIZED", "FOREIGN", "SYNONYM", "ASSEMBLY", "CATALOG", "LOGIN", "PLAN",
    "WORKLOAD", "RESOURCE", "ENDPOINT", "ROUTE", "CERTIFICATE", "AGGREGATE",
];

/// Words that sit between an introducer and the name it introduces.
const MODIFIERS: &[&str] = &["IF", "NOT", "EXISTS", "ONLY", "CONCURRENTLY", "OR", "REPLACE", "TABLE", "VIEW", "MATERIALIZED", "TEMP", "TEMPORARY", "UNLOGGED", "GLOBAL", "LOCAL", "FOREIGN", "RECURSIVE", "UNIQUE", "CLUSTERED", "NONCLUSTERED", "COLUMNSTORE", "VOLATILE", "SCHEMA", "DEFAULT", "PRIVILEGES"];

/// Words that open a column definition's constraint or a table constraint: no name follows them.
const CONSTRAINT_STARTS: &[&str] = &["CONSTRAINT", "PRIMARY", "FOREIGN", "UNIQUE", "CHECK", "INDEX", "KEY", "LIKE", "EXCLUDE", "FULLTEXT", "SPATIAL", "PERIOD"];

#[derive(Default, Clone, Copy)]
struct Walk {
    /// The next word is a name.
    expect_name: bool,
    /// The previous token was a name.
    after_name: bool,
    /// The next word is the first of an item in a list of column definitions.
    item_start: bool,
    /// Inside the brackets of a table's columns.
    columns: bool,
    /// The previous word was `FUNCTION` or `PROCEDURE`.
    function_name: bool,
    /// The statement is a `CREATE TABLE`.
    create_table: bool,
}

impl P<'_> {
    /// Takes the statement at the cursor token by token; never fails.
    pub fn generic(&mut self) {
        let create_table = self.at("create") && (1..6).any(|i| self.is(i, "table"));
        self.generic_run(0, Walk { create_table, ..Walk::default() });
    }

    fn generic_run(&mut self, depth: usize, mut w: Walk) {
        while let Some(t) = self.t(0) {
            match t {
                T::Semi => break,
                T::RParen | T::RSquare if depth > 0 => break,
                T::LParen => {
                    let creating = w.expect_name || w.after_name;
                    let columns = w.after_name && w.create_table && depth == 0;
                    self.generic_bracket(depth, columns, creating && w.function_name);
                    w.expect_name = false;
                    w.after_name = false;
                }
                T::Word => self.generic_word(&mut w, depth),
                T::Quoted => {
                    w.expect_name = false;
                    w.after_name = true;
                    w.item_start = false;
                    self.bump();
                }
                T::Comma => {
                    w.item_start = w.columns;
                    w.expect_name = false;
                    w.after_name = false;
                    self.bump();
                }
                T::Dot => {
                    w.expect_name = true;
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

    fn generic_bracket(&mut self, depth: usize, columns: bool, params: bool) {
        if depth as u32 >= super::MAX_DEPTH {
            self.bump();
            return;
        }
        let _ = params;
        self.bracketed(|p| {
            let w = Walk {
                item_start: columns,
                columns,
                ..Walk::default()
            };
            p.generic_run(depth + 1, w);
        });
    }

    fn generic_word(&mut self, w: &mut Walk, depth: usize) {
        let mut buf = [0u8; 40];
        let up = super::dialect::upper(self.text(0), &mut buf).to_string();
        let dotted = self.is_t(1, T::Dot) || self.prev_is_dot();
        // A query inside a statement (`CREATE VIEW v AS SELECT ...`) parses as one.
        if !dotted && depth == 0 && (up == "SELECT" || (up == "WITH" && self.is_t(1, T::Word) && (self.is(2, "as") || self.is(1, "recursive")))) {
            self.query();
            *w = Walk::default();
            return;
        }
        let kind = if dotted {
            if self.is_t(1, T::Dot) && w.function_name {
                k::IDENTIFIER
            } else if w.function_name {
                k::FUNCTION_IDENTIFIER
            } else {
                k::IDENTIFIER
            }
        } else if w.item_start && !CONSTRAINT_STARTS.contains(&up.as_str()) {
            w.item_start = false;
            w.after_name = true;
            self.bump_as(k::IDENTIFIER);
            return;
        } else if w.expect_name && !MODIFIERS.contains(&up.as_str()) {
            if w.function_name {
                k::FUNCTION_IDENTIFIER
            } else {
                k::IDENTIFIER
            }
        } else if w.after_name && super::types::is_type_word(&up) && (w.columns || depth > 0) {
            self.type_name();
            w.after_name = false;
            return;
        } else if self.keyword_like(0) || self.reserved(0) {
            k::KEYWORD
        } else {
            k::IDENTIFIER
        };
        let is_name = kind == k::IDENTIFIER || kind == k::FUNCTION_IDENTIFIER;
        let next_dot = self.is_t(1, T::Dot);
        self.bump_as(kind);
        if kind == k::KEYWORD {
            w.function_name = matches!(up.as_str(), "FUNCTION" | "PROCEDURE" | "PROC" | "AGGREGATE");
            if INTRODUCERS.contains(&up.as_str()) {
                w.expect_name = true;
            } else if !MODIFIERS.contains(&up.as_str()) {
                w.expect_name = false;
            }
            w.after_name = false;
        } else {
            w.expect_name = next_dot;
            w.after_name = is_name && !next_dot;
            if !next_dot {
                w.function_name = false;
            }
        }
    }

    fn prev_is_dot(&self) -> bool {
        self.pos > 0 && self.kinds[self.pos - 1] == k::DOT && self.is_dot_before()
    }

    fn is_dot_before(&self) -> bool {
        self.pos > 0 && self.code.get(self.pos - 1).is_some_and(|&c| self.lex[c as usize].kind == T::Dot)
    }
}
