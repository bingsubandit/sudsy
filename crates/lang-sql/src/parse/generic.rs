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
    "DESCRIBE", "DESC", "EXISTS", "ONLY", "INHERITS", "METHOD", "NAMES", "CHARSET", "COLLATE", "ROUTINES", "FUNCTIONS",
    "TYPES", "ATTRIBUTE", "OF", "RENAME",
];

/// Words that are never a name, whatever precedes them.
const NEVER_NAMES: &[&str] = &["DELETE", "UPDATE", "INSERT", "SELECT", "ALL", "CASCADE", "RESTRICT", "NO", "SET", "NULL", "BY", "OF", "ACCESS", "CURRENT", "NONE", "DEPENDS", "OWNED", "COMMIT", "DROP", "ADD", "ALTER", "MAPPING"];

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
    /// The statement is a `CREATE`.
    create: bool,
    /// `CREATE STATISTICS`.
    statistics: bool,
    /// `CREATE [MATERIALIZED] VIEW`.
    view: bool,
    /// `ALTER AGGREGATE ..`: the name after `RENAME TO` is a function name.
    aggregate: bool,
    /// The statement is a `CREATE`/`ALTER POLICY`: its roles are names.
    policy: bool,
    /// The statement is a `PREPARE`: the bracket after its name lists types.
    prepare: bool,
    /// After `ATTRIBUTE name`: a type follows.
    attribute: bool,
    /// What `CURRENT_USER` and the like are in a role position of this statement.
    role_kind: Option<core_syntax::SyntaxKind>,
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
        let second = if self.is_t(1, T::Word) { self.text(1).to_string() } else { String::new() };
        let row = self.d.statement_row(self.text(0), &second);
        self.index = self.at("create") && (1..3).any(|i| self.is(i, "index"));
        self.row = row.filter(|r| r.iter().any(|e| !e.starts_with('@')));
        let role_kind = row.and_then(|r| r.iter().find_map(|e| e.strip_prefix("@role="))).map(|c| match c {
            "w" => k::WORD,
            "i" => k::IDENTIFIER,
            _ => k::KEYWORD,
        });
        let grant = self.at_any(&["grant", "revoke", "deny"]) || (self.at("alter") && self.is(2, "privileges"));
        let create_table = self.at("create") && (1..6).any(|i| self.is(i, "table"));
        let table_stmt = self.at_any(&["create", "alter"]) && (1..4).any(|i| self.is(i, "table"));
        self.generic_run(0, Walk { create_table, table_stmt, grant, create: self.at("create"), statistics: self.at("create") && self.is(1, "statistics"), view: self.at("create") && (1..4).any(|i| self.is(i, "view")), role_kind, prepare: self.at("prepare"), policy: (1..3).any(|i| self.is(i, "policy")), ..Walk::default() });
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
                    let columns = (w.after_name || self.prev_text_is("table")) && w.create_table && depth == 0;
                    let params = w.after_name && (w.function_name || w.prepare);
                    self.generic_bracket(depth, columns, params, w.create_table || w.table_stmt);
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
                    if self.d == super::Dialect::Tsql && self.starts_name(0) {
                        // `OBJECT::schema.name`
                        self.generic_name(false);
                    } else {
                        self.type_name();
                    }
                    w.after_name = false;
                    w.expr = false;
                }
                T::Op if (self.at_op("-") || self.at_op("+")) && self.is_t(1, T::Number) && self.sign_here() => {
                    self.bump_as(k::SIGN);
                }
                T::Op => {
                    w.value = self.at_op("=") && !w.columns;
                    w.expr = true;
                    w.expect_name = false;
                    w.after_name = false;
                    if self.at_op("=") && self.d == super::Dialect::Tsql {
                        let m = self.mark();
                        self.bump();
                        self.close(k::COMPARISON_OPERATOR, m);
                    } else {
                        self.bump();
                    }
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
            self.generic_bracket(0, false, false, false);
        }
    }

    fn generic_bracket(&mut self, depth: usize, columns: bool, params: bool, table: bool) {
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
                    table_stmt: table,
                    ..Walk::default()
                };
                p.generic_run(depth + 1, w);
            }
        });
    }

    /// A name (dotted): an object reference. The last part is a function name when `function`.
    pub(super) fn generic_name(&mut self, function: bool) {
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

    /// `-4` after a word: the sign touches the number and a space comes before it.
    fn sign_here(&self) -> bool {
        let at = |n: usize| self.code.get(self.pos + n).map(|&c| self.lex[c as usize]);
        let (Some(op), Some(num)) = (at(0), at(1)) else { return false };
        let space_before = self.pos > 0 && self.code.get(self.pos - 1).is_some_and(|&c| self.lex[c as usize].end != op.start);
        op.end == num.start && space_before
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
        // `CREATE POLICY .. USING (expr)`, `CREATE TABLE .. AS EXECUTE f()`, `CREATE STATISTICS .. ON expr, ..`.
        if word && !dotted && matches!(up.as_str(), "USING" | "CHECK") && w.policy && self.is_t(1, T::LParen) {
            self.bump_as(k::KEYWORD);
            self.bracketed(|p| {
                p.expression();
            });
            return;
        }
        if word && !dotted && up == "EXECUTE" && w.create_table && self.name_call_ahead_from(1) {
            self.bump_as(k::KEYWORD);
            self.chain(false);
            return;
        }
        if word && !dotted && up == "ON" && w.statistics {
            self.bump_as(k::KEYWORD);
            self.list(|p| p.element());
            return;
        }
        if word && !dotted && up == "ENUM" && self.prev_text_is("as") && self.is_t(1, T::LParen) {
            self.bump_as(k::KEYWORD);
            return;
        }
        if word && !dotted && up == "VALUE" && self.prev_text_is("rename") {
            self.bump_as(k::KEYWORD);
            w.expect_name = false;
            return;
        }
        // `CREATE FUNCTION add(..)`: the name before the bracket is a function name, reserved or not.
        if word && !dotted && self.is_t(1, T::LParen) && (["function", "procedure", "method", "routine"].iter().any(|x| self.prev_text_is(x)) || (w.function_name && self.prev_kind() == Some(T::Comma))) {
            self.generic_name(true);
            w.expect_name = false;
            w.after_name = true;
            w.function_name = true;
            return;
        }
        // `USING hash`: the index method is a name; `(key)`: so is a bare column.
        if word && !dotted && self.index && self.d == super::Dialect::Postgres && (self.prev_text_is("using") || (depth > 0 && matches!(self.prev_kind(), Some(T::LParen | T::Comma)) && matches!(self.t(1), Some(T::Comma | T::RParen)))) {
            self.bump_as(k::IDENTIFIER);
            return;
        }
        // An operator class after an index column: a name, not a call.
        if word && !dotted && self.index && depth > 0 && self.is_t(1, T::LParen) && self.prev_kind() == Some(T::Word) && !self.prev_text_is("using") {
            self.bump_as(k::IDENTIFIER);
            return;
        }
        // `CREATE VIEW v WITH (opt = value)`: names and values are identifiers.
        if word && !dotted && (up == "WITH" && (w.view || w.create_table) || up == "SET" && w.table_stmt) && self.d == super::Dialect::Postgres && self.is_t(1, T::LParen) {
            self.bump_as(k::KEYWORD);
            self.options_bracket(false);
            return;
        }
        // T-SQL `WITH (...)`, `OPTION (...)`: an option list.
        if word && self.d == super::Dialect::Tsql && matches!(up.as_str(), "WITH" | "OPTION") && self.is_t(1, T::LParen) && !w.columns && !w.params {
            self.bump_as(k::KEYWORD);
            self.options_bracket(true);
            *w = Walk { create_table: w.create_table, table_stmt: w.table_stmt, ..Walk::default() };
            return;
        }
        // `ON ASYMMETRIC KEY::name`: the two words before `::` are the class.
        if word && self.d == super::Dialect::Tsql && self.is_t(1, T::Word) && self.is_t(2, T::Cast) {
            self.bump_as(k::KEYWORD);
            self.bump_as(k::KEYWORD);
            w.expect_name = false;
            return;
        }
        // T-SQL `OBJECT::name`: the class before `::` is a keyword.
        if word && self.d == super::Dialect::Tsql && self.is_t(1, T::Cast) {
            self.bump_as(k::KEYWORD);
            w.expect_name = false;
            return;
        }
        // `PREPARE p AS UPDATE ...`: a statement of its own.
        if word && !dotted && matches!(up.as_str(), "UPDATE" | "DELETE" | "INSERT" | "MERGE") && self.prev_text_is("as") {
            self.statement_body();
            return;
        }
        // A value of a setting (`SET x = on, off`) and the key before `=`.
        if word && !dotted && !w.columns && !w.params && !w.options && self.row.is_none() && (w.value || self.is_op(1, "=") || (self.d == super::Dialect::Postgres && self.prev_text_is("set") && self.is(1, "to"))) {
            let kind = match self.d {
                super::Dialect::Tsql if self.keyword_like(0) => k::KEYWORD,
                super::Dialect::Tsql => k::IDENTIFIER,
                _ => k::WORD,
            };
            self.bump_as(kind);
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
        if word && !dotted && !w.grant && (up == "SELECT" || (up == "WITH" && (self.is(1, "recursive") || self.cte_ahead()))) {
            self.query();
            *w = Walk::default();
            return;
        }
        // The first word of a definition is its name; a parameter that is only a type is a type.
        let constraint = word && !w.params && CONSTRAINT_STARTS.contains(&up.as_str()) && (self.d == super::Dialect::Mysql || !matches!(up.as_str(), "KEY" | "FULLTEXT" | "SPATIAL"));
        if w.item_start && !constraint {
            w.item_start = false;
            if w.params && word && (super::types::is_type_word(&up) || (self.d == super::Dialect::Postgres && matches!(self.t(1), Some(T::Comma | T::RParen)))) && !self.is_t(1, T::Word) {
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
        if word && !dotted && w.after_name && w.columns && self.is_t(1, T::LParen) && !self.reserved(0) {
            self.type_name();
            w.after_name = false;
            return;
        }
        if quoted && w.after_name && (w.columns || w.params) {
            self.type_name();
            w.after_name = false;
            return;
        }
        if word && depth > 0 && !w.columns && !w.params && super::types::is_type_word(&up) && self.is(1, "as") {
            self.type_name();
            return;
        }
        // The word after a column's name is its type, known or not.
        let column_type = self.d == super::Dialect::Postgres && w.columns && !w.options && !self.reserved(0) && !constraint && !matches!(up.as_str(), "GENERATED" | "COLLATE" | "REFERENCES" | "DEFAULT" | "NOT" | "NULL" | "AS" | "COMMENT" | "CHARACTER" | "CHARSET");
        if word && !dotted && w.after_name && (super::types::is_type_word(&up) || column_type) && (w.columns || w.params || depth > 0 || w.attribute) {
            self.type_name();
            w.after_name = false;
            return;
        }
        let bare_function = word && super::expr::BARE_FUNCTIONS.contains(&up.as_str()) && !self.is_t(1, T::LParen);
        if bare_function {
            // A role or schema in a list of names (`TO current_user, public`) is a keyword.
            let role = !w.expr
                && matches!(up.as_str(), "CURRENT_USER" | "SESSION_USER" | "CURRENT_ROLE" | "USER" | "CURRENT_SCHEMA")
                && (matches!(self.prev_kind(), Some(T::Comma)) || ["to", "by", "for", "schema"].iter().any(|x| self.prev_text_is(x)));
            let kind = match (role, up.as_str()) {
                (false, _) => k::FUNCTION_IDENTIFIER,
                (true, "CURRENT_USER" | "SESSION_USER") if (w.policy || w.grant) && !matches!(self.prev_kind(), Some(T::Comma)) => k::IDENTIFIER,
                // A role reference refuses the reserved words, which then match the keyword alternative.
                (true, _) if w.role_kind == Some(k::IDENTIFIER) && self.reserved(0) => k::KEYWORD,
                _ => w.role_kind.unwrap_or(k::KEYWORD),
            };
            self.bump_as(kind);
            if role {
                w.name_list = true;
                w.expect_name = false;
                w.after_name = false;
                return;
            }
            *w = Walk { create_table: w.create_table, table_stmt: w.table_stmt, grant: w.grant, ..Walk::default() };
            return;
        }
        // An item of a bracketed list that is a bare word: a name (`(no)`, `(name)`).
        let bare_item = word && depth > 0 && w.table_stmt && !w.columns && !w.params && !self.reserved(0) && matches!(self.prev_kind(), Some(T::LParen | T::Comma)) && matches!(self.t(1), Some(T::Comma | T::RParen));
        let in_row = self.row.is_some_and(|r| r.contains(up.as_str()));
        let name_here = bare_item || w.expect_name && !in_row && !self.reserved(0) && !MODIFIERS.contains(&up.as_str()) && !INTRODUCERS.contains(&up.as_str()) && !NEVER_NAMES.contains(&up.as_str());
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
        let kind = match self.row {
            Some(row) if word => {
                if self.prev_word().is_some_and(|p| row.contains(format!("{p}>").as_str())) {
                    k::IDENTIFIER
                } else if row.contains(up.as_str()) || INTRODUCERS.contains(&up.as_str()) || MODIFIERS.contains(&up.as_str()) || matches!(self.prev_kind(), None | Some(T::Semi | T::Data)) {
                    k::KEYWORD
                } else if matches!(up.as_str(), "TRUE" | "FALSE") {
                    k::WORD_LITERAL
                } else {
                    k::IDENTIFIER
                }
            }
            _ if word && matches!(up.as_str(), "TRUE" | "FALSE") => k::WORD_LITERAL,
            _ if self.keyword_like(0) || self.reserved(0) => k::KEYWORD,
            _ => k::IDENTIFIER,
        };
        let after_character = self.prev_text_is("character");
        if word && !dotted && self.d == super::Dialect::Postgres && w.create && self.prev_word().as_deref() == Some("USING") && !self.is_t(1, T::LParen) {
            self.bump_as(k::WORD);
            return;
        }
        let first = matches!(self.prev_kind(), None | Some(T::Semi | T::Data));
        let rename_to = up == "TO" && w.aggregate && self.prev_text_is("rename");
        self.bump_as(kind);
        if kind == k::KEYWORD {
            if matches!(up.as_str(), "FUNCTION" | "PROCEDURE" | "PROC" | "AGGREGATE" | "ROUTINE") {
                w.function_name = true;
                if up == "AGGREGATE" && self.d == super::Dialect::Postgres && self.starts_name(0) {
                    self.aggregate_signature(w.create);
                    w.expect_name = false;
                    w.after_name = false;
                    w.aggregate = true;
                    return;
                }
            } else if !MODIFIERS.contains(&up.as_str()) && !INTRODUCERS.contains(&up.as_str()) {
                w.function_name = false;
            }
            w.attribute = up == "ATTRIBUTE";
            if INTRODUCERS.contains(&up.as_str()) || (up == "SET" && after_character) || (up == "ALTER" && !first) {
                w.expect_name = true;
            } else if !MODIFIERS.contains(&up.as_str()) {
                w.expect_name = false;
                w.name_list = false;
            }
            w.function_name |= rename_to;
            w.after_name = false;
            w.expr = EXPR_AFTER.contains(&up.as_str()) || (up == "AS" && w.columns);
            if (matches!(up.as_str(), "AS" | "RETURNS") && self.at_type_word() && !self.at("enum")) || (up == "RETURNS" && self.d == super::Dialect::Postgres && self.is_t(0, T::Word) && !self.reserved(0) && !self.at_any(&["setof", "trigger"])) {
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

    /// The word before the cursor, uppercased.
    fn prev_word(&self) -> Option<String> {
        let c = *self.code.get(self.pos.checked_sub(1)?)?;
        let l = self.lex[c as usize];
        (l.kind == T::Word).then(|| self.src[l.start as usize..l.end as usize].to_ascii_uppercase())
    }

    fn prev_text_is(&self, word: &str) -> bool {
        self.pos.checked_sub(1).and_then(|i| self.code.get(i)).is_some_and(|&c| self.src[self.lex[c as usize].start as usize..self.lex[c as usize].end as usize].eq_ignore_ascii_case(word))
    }

    /// `WITH name AS (`, `WITH name (cols) AS (`: a common table expression, not a `WITH` option.
    fn cte_ahead(&self) -> bool {
        if !self.is_t(1, T::Word) {
            return false;
        }
        let mut n = 2;
        if self.is_t(2, T::LParen) {
            let mut depth = 0;
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
            n += 1;
        }
        self.is(n, "as") && (self.is_t(n + 1, T::LParen) || self.is(n + 1, "materialized") || self.is(n + 1, "not"))
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

impl P<'_> {
    /// Does a (dotted) name followed by a bracket start `n` tokens ahead?
    fn name_call_ahead_from(&self, n: usize) -> bool {
        let mut n = n;
        while self.starts_name(n) && self.is_t(n + 1, T::Dot) {
            n += 2;
        }
        self.starts_name(n) && self.is_t(n + 1, T::LParen)
    }
}
