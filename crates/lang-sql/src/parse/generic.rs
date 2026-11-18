//! A statement the parser has no rules for (most DDL): its tokens up to the `;`, brackets nested,
//! the words named by the place they stand in: the word after `TABLE`, `TO`, `ON` is a name, the
//! first word of a column definition is a name and the word after it a type, expressions after
//! `DEFAULT` and in `CHECK (...)` parse as expressions, the rest are keywords when the dialect's
//! tables say so.
//!
//! concept: recursive-descent-parser

use super::P;
use super::lexer::T;
use crate::generated::kind as k;

/// Words after which a name follows.
const INTRODUCERS: &[&str] = &[
    "TABLE",
    "VIEW",
    "INDEX",
    "SCHEMA",
    "DATABASE",
    "SEQUENCE",
    "TRIGGER",
    "TYPE",
    "ROLE",
    "USER",
    "EXTENSION",
    "DOMAIN",
    "TO",
    "ON",
    "INTO",
    "REFERENCES",
    "FROM",
    "JOIN",
    "TRUNCATE",
    "OWNER",
    "CONSTRAINT",
    "COLUMN",
    "TABLESPACE",
    "TABLES",
    "SCHEMAS",
    "SEQUENCES",
    "COLLATION",
    "RULE",
    "POLICY",
    "PUBLICATION",
    "SUBSCRIPTION",
    "SERVER",
    "WRAPPER",
    "LANGUAGE",
    "CAST",
    "DICTIONARY",
    "CONFIGURATION",
    "PARSER",
    "TEMPLATE",
    "OPERATOR",
    "FAMILY",
    "CLASS",
    "GROUP",
    "STATISTICS",
    "TRANSFORM",
    "EVENT",
    "SYNONYM",
    "ASSEMBLY",
    "CATALOG",
    "LOGIN",
    "PLAN",
    "WORKLOAD",
    "RESOURCE",
    "ENDPOINT",
    "ROUTE",
    "CERTIFICATE",
    "AGGREGATE",
    "FUNCTION",
    "PROCEDURE",
    "ROUTINE",
    "EXECUTE",
    "USE",
    "DESCRIBE",
    "DESC",
    "EXISTS",
    "ONLY",
    "INHERITS",
    "METHOD",
    "NAMES",
    "CHARSET",
    "COLLATE",
    "ROUTINES",
    "FUNCTIONS",
    "TYPES",
    "ATTRIBUTE",
    "OF",
    "RENAME",
];

/// Words that are never a name, whatever precedes them.
const NEVER_NAMES: &[&str] = &[
    "DELETE", "UPDATE", "INSERT", "SELECT", "ALL", "CASCADE", "RESTRICT", "NO", "SET", "NULL",
    "BY", "OF", "ACCESS", "CURRENT", "NONE", "DEPENDS", "OWNED", "COMMIT", "DROP", "ALTER",
    "RENAME",
];

/// Words that sit between an introducer and the name it introduces.
const MODIFIERS: &[&str] = &[
    "IF",
    "NOT",
    "EXISTS",
    "ONLY",
    "CONCURRENTLY",
    "OR",
    "REPLACE",
    "TABLE",
    "VIEW",
    "MATERIALIZED",
    "TEMP",
    "TEMPORARY",
    "UNLOGGED",
    "GLOBAL",
    "LOCAL",
    "FOREIGN",
    "RECURSIVE",
    "UNIQUE",
    "CLUSTERED",
    "NONCLUSTERED",
    "COLUMNSTORE",
    "VOLATILE",
    "SCHEMA",
    "DEFAULT",
    "PRIVILEGES",
    "MAPPING",
    "VALUE",
    "FUNCTION",
    "PROCEDURE",
    "SEQUENCE",
    "TYPE",
    "COLUMN",
    "CONSTRAINT",
    "INDEX",
];

/// Words that open a table constraint: no column name follows them.
const CONSTRAINT_STARTS: &[&str] = &[
    "CONSTRAINT",
    "PRIMARY",
    "FOREIGN",
    "UNIQUE",
    "CHECK",
    "INDEX",
    "KEY",
    "LIKE",
    "EXCLUDE",
    "FULLTEXT",
    "SPATIAL",
    "PERIOD",
];

/// Words after which an expression follows.
const EXPR_AFTER: &[&str] = &[
    "DEFAULT", "WHERE", "WHEN", "THEN", "ELSE", "IF", "WHILE", "RETURN", "RETURNS", "CALL",
    "UPDATE",
];

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
    /// Before `ON`/`TO` of a GRANT: the permission words, all keywords.
    perms: bool,
    /// The next word is the type of the column just named.
    type_slot: bool,
    /// After the columns of a `CREATE TABLE`: `ENGINE = x` style options.
    options: bool,
    /// The statement is a `CREATE`.
    create: bool,
    /// The statement is a `CREATE` of a procedure: its parameters keep a blank before the bracket.
    procedure: bool,
    /// `ALTER AGGREGATE ..`: the name after `RENAME TO` is a function name.
    aggregate: bool,
    /// The statement is a `CREATE`/`ALTER POLICY`: its roles are names.
    policy: bool,
    /// The statement is a `PREPARE`: the bracket after its name lists types.
    prepare: bool,
    /// After `ATTRIBUTE name`: a type follows.
    attribute: bool,
    /// The statement is a `CREATE TYPE`: the name is a type identifier.
    type_stmt: bool,
    /// Inside the brackets of a `CREATE INDEX`: the items are column names (with an operator class).
    index: bool,
    /// The statement is a `CREATE INDEX`.
    index_stmt: bool,
    /// The statement is a `CREATE STATISTICS`: expressions follow `ON`.
    statistics: bool,
    /// What `CURRENT_USER` and the like are in a role position of this statement.
    role_kind: Option<core_syntax::SyntaxKind>,
}

impl P<'_> {
    /// Takes the statement at the cursor token by token; never fails.
    pub fn generic(&mut self) {
        if self.d == super::Dialect::Mysql && self.at("declare") && !self.is_any_after_declare() {
            self.bump_as(k::KEYWORD);
            let w = Walk {
                item_start: true,
                params: true,
                ..Walk::default()
            };
            self.generic_run(0, w);
            return;
        }
        let second = (1..4)
            .map(|i| self.text(i))
            .find(|t| {
                ![
                    "OR",
                    "REPLACE",
                    "TEMP",
                    "TEMPORARY",
                    "RECURSIVE",
                    "GLOBAL",
                    "LOCAL",
                    "UNLOGGED",
                ]
                .contains(&t.to_ascii_uppercase().as_str())
            })
            .filter(|_| self.is_t(1, T::Word))
            .unwrap_or("")
            .to_string();
        let row = self.d.statement_row(self.text(0), &second);
        self.row = row.filter(|r| r.iter().any(|e| !e.starts_with('@')));
        self.paren_word = row.is_some_and(|r| r.contains("@paren=w"));
        let role_kind = row
            .and_then(|r| r.iter().find_map(|e| e.strip_prefix("@role=")))
            .map(|c| match c {
                "w" => k::WORD,
                "i" => k::IDENTIFIER,
                _ => k::KEYWORD,
            });
        let grant = self.at_any(&["grant", "revoke", "deny"])
            || (self.at("alter") && self.is(1, "default") && self.is(2, "privileges"));
        let create_table = self.at("create") && (1..6).any(|i| self.is(i, "table"));
        let table_stmt = self.at_any(&["create", "alter"]) && (1..4).any(|i| self.is(i, "table"));
        self.generic_run(
            0,
            Walk {
                create_table,
                table_stmt,
                grant,
                perms: grant && self.d == super::Dialect::Tsql,
                create: self.at("create"),
                procedure: self.at("create")
                    && (1..4).any(|i| self.is(i, "procedure") || self.is(i, "proc")),
                role_kind,
                type_stmt: self.at("create") && self.is(1, "type"),
                statistics: self.at("create") && self.is(1, "statistics"),
                index_stmt: self.at("create") && (1..4).any(|i| self.is(i, "index")),
                prepare: self.at("prepare"),
                policy: (1..3).any(|i| self.is(i, "policy")),
                ..Walk::default()
            },
        );
    }

    /// `DECLARE CONTINUE HANDLER`, `DECLARE c CURSOR`, `DECLARE x CONDITION`: not a variable.
    fn is_any_after_declare(&self) -> bool {
        self.is(1, "continue")
            || self.is(1, "exit")
            || self.is(1, "undo")
            || self.is(2, "cursor")
            || self.is(2, "condition")
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
                    let columns = (w.after_name && w.create_table && depth == 0)
                        || (w.type_stmt && depth == 0 && self.prev_text_is("table"));
                    let params = w.after_name && (w.function_name || w.prepare);
                    let signature = w.after_name && w.function_name && !w.procedure;
                    let m = self.mark();
                    self.generic_bracket(
                        depth,
                        columns,
                        params,
                        w.create_table || w.table_stmt,
                        w.after_name && w.index_stmt && depth == 0,
                    );
                    if signature {
                        self.close(k::FUNCTION_CONTENTS, m);
                    }
                    w.options |= columns;
                    w.expect_name = false;
                    w.after_name = false;
                    w.item_start = false;
                }
                T::Word
                    if w.grant
                        && (0..3).any(|j| {
                            self.is_t(j + 1, T::Cast) && (0..=j).all(|i| self.is_t(i, T::Word))
                        }) =>
                {
                    // `ON ASYMMETRIC KEY::name`: the class words are keywords.
                    while self.is_t(0, T::Word) {
                        self.bump_as(k::KEYWORD);
                    }
                    self.bump();
                    self.generic_name(false);
                }
                T::LSquare => {
                    self.bump();
                    self.indent();
                    while !self.eof() && !self.is_t(0, T::RSquare) && !self.is_t(0, T::Semi) {
                        self.bump();
                    }
                    self.dedent();
                    self.eat_t(T::RSquare);
                }
                T::Word | T::Quoted => self.generic_word(&mut w, depth),
                T::Comma => {
                    w.item_start = w.columns || w.params || w.index;
                    w.expr |= w.statistics;
                    w.expect_name = (w.expect_name || w.name_list)
                        && !w.item_start
                        && !self.row.is_some_and(|r| r.contains("@other=k"));
                    w.after_name = false;
                    self.bump();
                }
                T::Cast => {
                    self.bump();
                    self.type_name();
                    w.after_name = false;
                    w.expr = false;
                }
                T::Op
                    if (self.at_op("-") || self.at_op("+"))
                        && self.is_t(1, T::Number)
                        && self.sign_here()
                        && !self.paren_word =>
                {
                    self.bump_as(k::SIGN);
                }
                T::Op if self.row.is_some() && self.at_op("=") && depth > 0 => {
                    let m = self.mark();
                    self.bump();
                    self.close(k::COMPARISON_OPERATOR, m);
                }
                T::Op
                    if self.d == super::Dialect::Tsql
                        && self.at_op("=")
                        && !w.columns
                        && !w.params
                        && !w.expr =>
                {
                    let m = self.mark();
                    self.bump();
                    self.close(k::COMPARISON_OPERATOR, m);
                    w.value = true;
                }
                T::Op => {
                    w.value = self.at_op("=") && !w.columns;
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
            self.generic_bracket(0, false, false, false, false);
        }
    }

    /// The bracket at the cursor as a column list (`(a int, b varchar(10))`).
    pub fn generic_columns_bracket(&mut self) {
        self.generic_bracket(0, true, false, false, false);
    }

    fn generic_bracket(
        &mut self,
        depth: usize,
        columns: bool,
        params: bool,
        table: bool,
        index: bool,
    ) {
        if depth as u32 >= super::MAX_DEPTH {
            self.bump();
            return;
        }
        self.bracketed(|p| {
            if p.at_query_start() {
                p.query();
            } else {
                let w = Walk {
                    item_start: columns || params || index,
                    index,
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
        let node = if function {
            k::FUNCTION_NAME
        } else {
            k::OBJECT_REFERENCE
        };
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
        let (Some(op), Some(num)) = (at(0), at(1)) else {
            return false;
        };
        let space_before = self.pos > 0
            && self
                .code
                .get(self.pos - 1)
                .is_some_and(|&c| self.lex[c as usize].end != op.start);
        op.end == num.start && space_before
    }

    fn prev_kind(&self) -> Option<T> {
        self.pos
            .checked_sub(1)
            .and_then(|i| self.code.get(i))
            .map(|&c| self.lex[c as usize].kind)
    }

    fn generic_word(&mut self, w: &mut Walk, depth: usize) {
        let mut buf = [0u8; 40];
        let up = super::dialect::upper(self.text(0), &mut buf).to_string();
        let quoted = self.is_t(0, T::Quoted);
        let dotted = self.is_t(1, T::Dot) || self.prev_is_dot();
        let word = !quoted;
        // `CONNECTION (a TO b)`: the constraint of a graph edge table.
        if word && up == "CONNECTION" && self.d == super::Dialect::Tsql && self.is_t(1, T::LParen) {
            self.bump_as(k::KEYWORD);
            self.bracketed(|p| {
                p.obj_ref();
                if p.at("to") {
                    p.bump_as(k::KEYWORD);
                    p.obj_ref();
                }
            });
            return;
        }
        // `PREPARE p AS UPDATE ...`: a statement of its own.
        if word
            && !dotted
            && matches!(up.as_str(), "UPDATE" | "DELETE" | "INSERT" | "MERGE")
            && self.prev_text_is("as")
        {
            self.statement_body();
            return;
        }
        // A value of a setting (`SET x = on, off`) and the key before `=`.
        if word
            && !dotted
            && !w.columns
            && !w.params
            && !w.options
            && self.row.is_none()
            && !self.is_t(1, T::LParen)
            && (w.value || self.is_op(1, "="))
        {
            let kind = match (self.d == super::Dialect::Tsql, w.value) {
                (false, _) => k::WORD,
                (true, _) if matches!(up.as_str(), "TRUE" | "FALSE") => k::WORD_LITERAL,
                (true, true) if up.starts_with(|c: char| c.is_ascii_digit()) => k::WORD,
                (true, true) if self.keyword_like(0) => k::KEYWORD,
                (true, true) => k::IDENTIFIER,
                (true, false) if self.keyword_like(0) => k::KEYWORD,
                (true, false) => k::IDENTIFIER,
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
        if word
            && !dotted
            && !w.grant
            && (up == "SELECT"
                || (up == "VALUES" && self.prev_word().as_deref() == Some("AS"))
                || (up == "WITH" && (self.is(1, "recursive") || self.cte_ahead())))
        {
            self.query();
            *w = Walk::default();
            return;
        }
        // The first word of a definition is its name; a parameter that is only a type is a type.
        let typed_key = word
            && up == "KEY"
            && ((self.is_t(1, T::Word) && super::types::is_type_word(self.text(1)))
                || matches!(self.t(1), Some(T::RParen | T::Comma)));
        if w.item_start
            && !(word && !typed_key && CONSTRAINT_STARTS.contains(&up.as_str()) && !w.params)
            && !(w.index && self.is_t(1, T::LParen))
        {
            w.item_start = false;
            if w.params && word && super::types::is_type_word(&up) && !self.is_t(1, T::Word) {
                self.type_name();
                return;
            }
            if w.params && word && matches!(up.as_str(), "IN" | "OUT" | "INOUT" | "VARIADIC") {
                self.bump_as(if self.d == super::Dialect::Postgres {
                    k::KEYWORD
                } else {
                    k::WORD
                });
                w.item_start = true;
                return;
            }
            self.generic_name(false);
            w.after_name = true;
            w.type_slot = w.columns && !w.params;
            return;
        }
        w.item_start = false;
        // The word after a column name is its type.
        if std::mem::take(&mut w.type_slot)
            && word
            && !dotted
            && !self.reserved(0)
            && !CONSTRAINT_STARTS.contains(&up.as_str())
        {
            self.type_name();
            w.after_name = false;
            return;
        }
        // `CAST (int AS bool)`: types.
        if word
            && !dotted
            && up != "CHECK"
            && w.after_name
            && w.columns
            && self.is_t(1, T::LParen)
            && !self.reserved(0)
        {
            self.type_name();
            w.after_name = false;
            return;
        }
        if quoted && w.after_name && (w.columns || w.params) {
            self.type_name();
            w.after_name = false;
            return;
        }
        if word
            && depth > 0
            && !w.columns
            && !w.params
            && super::types::is_type_word(&up)
            && self.is(1, "as")
        {
            self.type_name();
            return;
        }
        if word
            && !dotted
            && w.after_name
            && super::types::is_type_word(&up)
            && self.row.is_none()
            && (w.columns || w.params || depth > 0 || w.attribute)
        {
            self.type_name();
            w.after_name = false;
            return;
        }
        // `GENERATED ALWAYS AS IDENTITY (START WITH 1)`.
        if word
            && up == "IDENTITY"
            && self.d == super::Dialect::Postgres
            && self.is_t(1, T::LParen)
            && self.prev_word().as_deref() == Some("AS")
        {
            self.bump_as(k::KEYWORD);
            self.generic_bracket_run();
            return;
        }
        let bare_function =
            word && super::expr::BARE_FUNCTIONS.contains(&up.as_str()) && !self.is_t(1, T::LParen);
        if bare_function {
            // A role or schema in a list of names (`TO current_user, public`) is a keyword.
            let role = !w.expr
                && matches!(
                    up.as_str(),
                    "CURRENT_USER" | "SESSION_USER" | "CURRENT_ROLE" | "USER" | "CURRENT_SCHEMA"
                )
                && (matches!(self.prev_kind(), Some(T::Comma))
                    || ["to", "by", "for", "schema"]
                        .iter()
                        .any(|x| self.prev_text_is(x)));
            let kind = match (role, up.as_str()) {
                (false, _) => k::FUNCTION_IDENTIFIER,
                (true, "CURRENT_USER") if w.policy => k::IDENTIFIER,
                (true, "CURRENT_ROLE" | "SESSION_USER") if w.role_kind == Some(k::IDENTIFIER) => {
                    k::KEYWORD
                }
                _ => w.role_kind.unwrap_or(k::KEYWORD),
            };
            self.bump_as(kind);
            if role {
                w.name_list = true;
                w.expect_name = false;
                w.after_name = false;
                return;
            }
            *w = Walk {
                create_table: w.create_table,
                table_stmt: w.table_stmt,
                grant: w.grant,
                perms: w.perms,
                ..Walk::default()
            };
            return;
        }
        if self.paren_word
            && word
            && depth > 0
            && !dotted
            && !self.keyword_like(0)
            && !self.is_t(1, T::LParen)
        {
            self.bump_as(k::WORD);
            return;
        }
        // An item of a bracketed list that is a bare word: a name (`(no)`, `(name)`).
        let bare_item = word
            && depth > 0
            && w.table_stmt
            && !w.columns
            && !w.params
            && !self.reserved(0)
            && matches!(self.prev_kind(), Some(T::LParen | T::Comma))
            && matches!(self.t(1), Some(T::Comma | T::RParen));
        let in_row = self.row.is_some_and(|r| r.contains(up.as_str()));
        let name_here = bare_item
            || w.expect_name
                && !in_row
                && !self.reserved(0)
                && !MODIFIERS.contains(&up.as_str())
                && !INTRODUCERS.contains(&up.as_str())
                && !NEVER_NAMES.contains(&up.as_str());
        let calls = (w.expr
            || (depth > 0 && !w.columns && !w.params && !(w.index && w.after_name)))
            && !self.reserved(0)
            || super::expr::BARE_FUNCTIONS.contains(&up.as_str());
        // A T-SQL keyword before a bracket (`WAIT_AT_LOW_PRIORITY (..)`) is no call.
        let keyword_bracket = self.d == super::Dialect::Tsql
            && word
            && !dotted
            && self.keyword_like(0)
            && !super::types::is_type_word(&up)
            && !matches!(
                up.as_str(),
                "CONVERT"
                    | "TRY_CONVERT"
                    | "CAST"
                    | "TRY_CAST"
                    | "ISNULL"
                    | "COALESCE"
                    | "NULLIF"
                    | "REPLICATE"
                    | "AVG"
                    | "MAX"
                    | "MIN"
                    | "SUM"
                    | "COUNT"
            );
        if (word || quoted)
            && calls
            && !keyword_bracket
            && (self.is_t(1, T::LParen) || (w.expr && self.name_then_paren()))
            && !matches!(up.as_str(), "CHECK" | "ROW" | "THAN")
        {
            self.chain(false);
            w.expr = false;
            return;
        }
        if word
            && !dotted
            && w.create
            && self.d == super::Dialect::Postgres
            && up == "ENUM"
            && self.prev_word().as_deref() == Some("AS")
        {
            self.bump_as(k::KEYWORD);
            return;
        }
        if name_here && !dotted && w.type_stmt && self.d == super::Dialect::Postgres {
            self.bump_as(k::TYPE_IDENTIFIER);
            w.expect_name = false;
            w.after_name = true;
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
                if up.starts_with(|c: char| c.is_ascii_digit()) {
                    k::WORD
                } else if matches!(up.as_str(), "TRUE" | "FALSE") {
                    k::WORD_LITERAL
                } else if self.prev_text_is("=") && row.contains("@other=k") && self.keyword_like(0)
                {
                    k::KEYWORD
                } else if self.prev_text_is("=")
                    || self
                        .prev_word()
                        .is_some_and(|p| row.contains(format!("{p}>").as_str()))
                {
                    k::IDENTIFIER
                } else if row.contains(up.as_str())
                    || INTRODUCERS.contains(&up.as_str())
                    || MODIFIERS.contains(&up.as_str())
                    || matches!(self.prev_kind(), None | Some(T::Semi | T::Data))
                {
                    k::KEYWORD
                } else if matches!(up.as_str(), "TRUE" | "FALSE") {
                    k::WORD_LITERAL
                } else if row.contains("@other=w") {
                    k::WORD
                } else if row.contains("@other=k") {
                    k::KEYWORD
                } else {
                    k::IDENTIFIER
                }
            }
            _ if word && matches!(up.as_str(), "TRUE" | "FALSE") => k::WORD_LITERAL,
            _ if w.perms && word && !dotted => k::KEYWORD,
            _ if self.keyword_like(0) || self.reserved(0) => k::KEYWORD,
            _ => k::IDENTIFIER,
        };
        let after_character = self.prev_text_is("character");
        if word
            && !dotted
            && self.d == super::Dialect::Postgres
            && w.create
            && self.prev_word().as_deref() == Some("USING")
        {
            self.bump_as(if self.is_t(1, T::LParen) {
                k::IDENTIFIER
            } else {
                k::WORD
            });
            return;
        }
        let first = matches!(self.prev_kind(), None | Some(T::Semi | T::Data));
        if matches!(up.as_str(), "ON" | "TO" | "FROM") {
            w.perms = false;
        }
        let rename_to = up == "TO" && w.aggregate && self.prev_text_is("rename");
        self.bump_as(kind);
        if kind == k::KEYWORD {
            if matches!(
                up.as_str(),
                "FUNCTION" | "PROCEDURE" | "PROC" | "AGGREGATE" | "ROUTINE" | "METHOD"
            ) {
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
            let exec_call = (up == "EXECUTE" && !first) || (up == "ON" && w.statistics);
            if !w.perms
                && (INTRODUCERS.contains(&up.as_str())
                    || (up == "SET" && after_character)
                    || (up == "ALTER" && !first && self.d == super::Dialect::Postgres))
            {
                w.expect_name = true;
            } else if !MODIFIERS.contains(&up.as_str()) {
                w.expect_name = false;
                w.name_list = false;
            }
            w.function_name |= rename_to;
            w.after_name = false;
            if exec_call {
                w.expect_name = false;
            }
            w.expr = exec_call || EXPR_AFTER.contains(&up.as_str()) || (up == "AS" && w.columns);
            let pg_returns = up == "RETURNS"
                && self.d == super::Dialect::Postgres
                && self.is_t(0, T::Word)
                && !self.at_any(&["table", "setof", "null"])
                && !self.is_t(1, T::Dot)
                && !self.reserved(0);
            if (matches!(up.as_str(), "RETURNS" | "AS")
                || (up == "FROM" && w.type_stmt && self.d == super::Dialect::Tsql))
                && (self.at_type_word() || pg_returns)
                && !self.at("enum")
            {
                self.type_name();
            }
            if (w.columns || w.create_table || w.table_stmt)
                && !w.options
                && up == "DEFAULT"
                && self.expr_ahead()
            {
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
        self.pos
            .checked_sub(1)
            .and_then(|i| self.code.get(i))
            .is_some_and(|&c| {
                self.src[self.lex[c as usize].start as usize..self.lex[c as usize].end as usize]
                    .eq_ignore_ascii_case(word)
            })
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
        self.is(n, "as")
            && (self.is_t(n + 1, T::LParen)
                || self.is(n + 1, "materialized")
                || self.is(n + 1, "not"))
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
            Some(T::Word) => {
                !self.reserved(0)
                    || self.at_any(&[
                        "null",
                        "true",
                        "false",
                        "current_timestamp",
                        "current_date",
                        "case",
                    ])
            }
            _ => false,
        }
    }

    fn prev_is_dot(&self) -> bool {
        self.pos > 0
            && self
                .code
                .get(self.pos - 1)
                .is_some_and(|&c| self.lex[c as usize].kind == T::Dot)
    }
}
