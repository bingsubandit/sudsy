//! Expressions: one flat chain of operands and operators, the way the tree keeps them (no
//! precedence nodes); brackets, calls, `CASE` and subqueries nest.
//!
//! concept: precedence-expression-parser

use super::lexer::T;
use super::{MAX_DEPTH, P};
use crate::generated::kind as k;

/// What a chain held: how many operands, and whether it was one plain term (a literal, a
/// column, a call), which a select element or a group key keeps without an expression node.
pub(super) struct Chain {
    pub terms: u32,
    pub lone: bool,
}

/// Words that open a clause or a statement: no operand starts with them.
const NOT_OPERAND: &[&str] = &[
    "FROM",
    "WHERE",
    "AND",
    "OR",
    "ON",
    "USING",
    "THEN",
    "WHEN",
    "ELSE",
    "END",
    "IN",
    "IS",
    "AS",
    "BETWEEN",
    "LIKE",
    "GROUP",
    "ORDER",
    "HAVING",
    "LIMIT",
    "UNION",
    "INTERSECT",
    "EXCEPT",
    "SET",
    "VALUES",
    "JOIN",
    "INNER",
    "FULL",
    "CROSS",
    "NATURAL",
    "OUTER",
    "WINDOW",
    "OFFSET",
    "FETCH",
    "FOR",
    "INTO",
    "RETURNING",
    "OVER",
    "ASC",
    "DESC",
    "WITH",
    "SELECT",
];

pub(super) const BARE_FUNCTIONS: &[&str] = &[
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_USER",
    "CURRENT_ROLE",
    "CURRENT_SCHEMA",
    "CURRENT_CATALOG",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "SESSION_USER",
    "SYSTEM_USER",
    "USER",
    "NOW",
];

/// Functions whose first argument is a date part.
const DATE_FUNCTIONS: &[&str] = &[
    "DATEADD",
    "DATEDIFF",
    "DATEDIFF_BIG",
    "DATEPART",
    "DATENAME",
    "DATE_PART",
    "DATE_TRUNC",
    "DATETRUNC",
    "EXTRACT",
    "TIMESTAMPADD",
    "TIMESTAMPDIFF",
    "DATE_ADD",
    "DATE_SUB",
    "TIMESTAMP_ADD",
    "TIMESTAMP_SUB",
    "DATE_DIFF",
];

const COMPARE: &[&str] = &["=", "<", ">", "<=", ">=", "<>", "!=", "!<", "!>", "<=>"];

/// A comparison operator, also when T-SQL blanks sit inside it (`> =`).
fn is_compare(text: &str) -> bool {
    COMPARE.contains(&text)
        || (text.contains(char::is_whitespace)
            && COMPARE.contains(&text.replace(char::is_whitespace, "").as_str()))
}
const ARITH: &[&str] = &["+", "-", "*", "/", "%"];
/// Postgres operators the layout treats as arithmetic.
const PG_BINARY: &[&str] = &[
    "^", "->", "->>", "#>", "#>>", "@>", "<@", "<->", "?|", "?&", "@@", "@?",
];

const FRAME_WORDS: &[&str] = &[
    "between",
    "and",
    "unbounded",
    "preceding",
    "following",
    "current",
    "row",
    "exclude",
    "no",
    "others",
    "ties",
    "group",
];

impl P<'_> {
    /// An expression in its own node. False when none starts here.
    pub fn expression(&mut self) -> bool {
        let m = self.mark();
        if self.chain(false).terms == 0 {
            return false;
        }
        self.close(k::EXPRESSION, m);
        true
    }

    /// An expression whose node is left out when it is one plain term.
    pub fn element(&mut self) -> bool {
        let m = self.mark();
        let c = self.chain(false);
        if c.terms == 0 {
            return false;
        }
        if !c.lone {
            self.close(k::EXPRESSION, m);
        }
        true
    }

    pub(super) fn chain(&mut self, no_logic: bool) -> Chain {
        let mut c = Chain {
            terms: 0,
            lone: true,
        };
        let mut between = 0;
        loop {
            while self.prefix() {
                c.lone = false;
            }
            if !self.operand(&mut c) {
                break;
            }
            c.terms += 1;
            self.postfix(&mut c);
            if !self.infix(no_logic, &mut between) {
                break;
            }
            c.lone = false;
        }
        c
    }

    fn prefix(&mut self) -> bool {
        if self.is_t(0, T::Op) {
            let sign = self.at_op("-") || self.at_op("+");
            if sign {
                self.bump_as(k::SIGN);
                return true;
            }
            if self.at_any_op(&["~", "!!", "@", "|/", "||/", "#", "@@", "@-@", "?-", "?|"])
                && !self.is_t(1, T::Comma)
            {
                if self.at_op("~") {
                    self.bump_as(k::PREFIX);
                } else {
                    self.bump();
                }
                return true;
            }
            return false;
        }
        if self.at("not") && !self.is(1, "in") && !self.is(1, "like") && !self.is(1, "between") {
            self.bump_as(k::KEYWORD);
            return true;
        }
        if self.at_any(&["any", "some", "all"]) && self.is_t(1, T::LParen) {
            self.bump_as(k::KEYWORD);
            return true;
        }
        false
    }

    fn at_any_op(&self, ops: &[&str]) -> bool {
        ops.iter().any(|o| self.at_op(o))
    }

    fn operand(&mut self, c: &mut Chain) -> bool {
        match self.t(0) {
            Some(T::Number | T::Str | T::Var) => {
                let string = self.is_t(0, T::Str);
                self.bump();
                // `'a'` and `'b'` on separate lines are one string literal.
                while string && self.is_t(0, T::Str) && self.newline_before() {
                    self.bump();
                }
                true
            }
            Some(T::Op) if self.at_op("?") => {
                self.bump();
                true
            }
            Some(T::Colon) if self.is_t(1, T::Word) && self.d != super::Dialect::Tsql => {
                self.bump_as(k::PREFIX);
                self.bump_as(k::IDENTIFIER);
                true
            }
            Some(T::LParen) => {
                self.paren_expr();
                c.lone = false;
                true
            }
            Some(T::LSquare)
                if matches!(self.d, super::Dialect::Postgres | super::Dialect::Ansi) =>
            {
                self.array_accessor();
                true
            }
            Some(T::Quoted) => self.name_or_call(c),
            Some(T::Word) => self.word_operand(c),
            _ => false,
        }
    }

    /// `( ... )` in an expression: a subquery, or one or more expressions.
    fn paren_expr(&mut self) {
        self.bracketed(|p| {
            if p.at_query_start() {
                p.query();
            } else {
                p.tuple();
            }
        });
    }

    /// The items of a bracket: one expression, or a list whose items drop the node when plain.
    fn tuple(&mut self) {
        let m = self.mark();
        let c = self.chain(false);
        if c.terms == 0 {
            return;
        }
        let tuple = self.is_t(0, T::Comma);
        if !tuple || !c.lone {
            self.close(k::EXPRESSION, m);
        }
        while self.is_t(0, T::Comma) {
            self.bump();
            if !self.element() {
                break;
            }
        }
    }

    /// An expression, or one bracket that holds all of it: `ON (a = b)`, `WHERE (x)`.
    pub fn bracketed_expression(&mut self) -> bool {
        if self.is_t(0, T::LParen) && !self.at_query_start() && self.paren_ends_expr() {
            self.bracketed(|p| {
                p.expression();
            });
            return true;
        }
        self.expression()
    }

    /// Does the bracket at the cursor end the expression it starts?
    fn paren_ends_expr(&self) -> bool {
        let mut depth = 0usize;
        let mut n = 0;
        while let Some(t) = self.t(n) {
            match t {
                T::LParen | T::LSquare => depth += 1,
                T::RParen | T::RSquare => {
                    depth -= 1;
                    if depth == 0 {
                        n += 1;
                        break;
                    }
                }
                _ => {}
            }
            n += 1;
        }
        match self.t(n) {
            None | Some(T::Semi | T::RParen | T::Comma) => true,
            Some(T::Word) => !(self.at_op_word(n)),
            _ => false,
        }
    }

    /// Is the word `n` ahead one that continues an expression?
    fn at_op_word(&self, n: usize) -> bool {
        [
            "and", "or", "is", "not", "in", "like", "between", "ilike", "collate", "overlaps", "at",
        ]
        .iter()
        .any(|w| self.is(n, w))
    }

    pub fn at_query_start(&self) -> bool {
        self.at_any(&["select", "with", "table"]) && !self.is_t(1, T::Dot)
            || self.at("values") && (self.is_t(1, T::LParen) || self.is(1, "row"))
            || (self.is_t(0, T::LParen) && self.paren_starts_query())
    }

    /// Does the bracket at the cursor (possibly several nested) open a query?
    fn paren_starts_query(&self) -> bool {
        let mut n = 0;
        while self.is_t(n, T::LParen) {
            n += 1;
        }
        if !(self.is(n, "select")
            || self.is(n, "with")
            || (self.is(n, "values") && (self.is_t(n + 1, T::LParen) || self.is(n + 1, "row")))
            || (self.is(n, "table") && self.starts_name(n + 1)))
        {
            return false;
        }
        // `(select ..) = 0 and x`: a bracket that goes on as an expression is not a query.
        // The same for the inner bracket of `((select ..)::int[])`.
        if n > 1 && self.bracket_end_is_operator(1) {
            return false;
        }
        !self.bracket_end_is_operator(0)
    }

    /// Does an operator follow the bracket that opens at token `from`?
    fn bracket_end_is_operator(&self, from: usize) -> bool {
        core_syntax::spans::bracket_followed_by(
            |i| self.t(i),
            from,
            &T::LParen,
            &T::RParen,
            |i| {
                self.is_t(i, T::Op)
                    || self.is_t(i, T::Cast)
                    || (self.is_t(i, T::Word) && self.at_op_word(i))
            },
        )
    }

    fn word_operand(&mut self, c: &mut Chain) -> bool {
        let mut buf = [0u8; 40];
        let up = super::dialect::upper(self.text(0), &mut buf);
        match up {
            "NULL" | "TRUE" | "FALSE" if !self.is_t(1, T::Dot) => {
                self.bump_as(k::WORD_LITERAL);
                return true;
            }
            "EXISTS"
                if self.is_t(1, T::LParen)
                    && !(self.d == super::Dialect::Postgres && self.touches_next()) =>
            {
                self.bump_as(k::KEYWORD);
                self.bracketed(|p| {
                    if p.at_query_start() {
                        p.query();
                    } else {
                        p.expression();
                    }
                });
                c.lone = false;
                return true;
            }
            "NEXT" if self.is(1, "value") && self.is(2, "for") => {
                self.bump_as(k::KEYWORD);
                self.bump_as(k::KEYWORD);
                self.bump_as(k::KEYWORD);
                self.obj_ref();
                if self.at("over") {
                    self.over_clause();
                }
                c.lone = false;
                return true;
            }
            "DEFAULT" if !self.is_t(1, T::Dot) => {
                self.bump_as(k::KEYWORD);
                return true;
            }
            "CASE" => {
                self.case_expr();
                c.lone = false;
                return true;
            }
            "INTERVAL"
                if matches!(self.t(1), Some(T::Str | T::Number))
                    || ((self.is_op(1, "-") || self.is_op(1, "+")) && self.is_t(2, T::Number)) =>
            {
                self.bump_as(k::KEYWORD);
                if self.is_t(0, T::Op) {
                    self.bump();
                }
                self.bump();
                if self.is_t(0, T::Word) && self.keyword_like(0) && !self.reserved(0) {
                    self.bump_as(k::KEYWORD);
                }
                return true;
            }
            "TIMESTAMP" | "TIME"
                if self.at_any_at(1, &["with", "without"])
                    && self.is(2, "time")
                    && self.is(3, "zone")
                    && self.is_t(4, T::Str) =>
            {
                let kind = if self.d == super::Dialect::Postgres {
                    k::TYPE_IDENTIFIER
                } else {
                    k::KEYWORD
                };
                for _ in 0..4 {
                    self.bump_as(kind);
                }
                self.bump();
                return true;
            }
            "CURRENT" if self.is(1, "of") => {
                self.bump_as(k::KEYWORD);
                self.bump_as(k::KEYWORD);
                self.obj_ref();
                return true;
            }
            "DATE" | "TIME" | "TIMESTAMP" | "DATETIME" | "TIMESTAMPTZ" if self.is_t(1, T::Str) => {
                self.bump_as(if self.d == super::Dialect::Postgres {
                    k::TYPE_IDENTIFIER
                } else {
                    k::KEYWORD
                });
                self.bump();
                return true;
            }
            // `box '(0,0)'`: a typed literal of any type name.
            _ if self.d == super::Dialect::Postgres
                && self.is_t(0, T::Word)
                && self.is_t(1, T::Str)
                && !self.reserved(0)
                && (!self.keyword_like(0) || super::types::is_type_word(up))
                && !self.prev_is_plain_word() =>
            {
                self.bump_as(k::TYPE_IDENTIFIER);
                self.bump();
                return true;
            }
            "SELECT" | "WITH" if self.d != super::Dialect::Tsql || !self.is_t(1, T::Op) => {
                self.query();
                c.lone = false;
                return true;
            }
            "ARRAY" if self.is_t(1, T::LSquare) => {
                self.bump_as(k::KEYWORD);
                self.array_accessor();
                c.lone = false;
                return true;
            }
            _ => {}
        }
        if BARE_FUNCTIONS.contains(&up) && !self.is_t(1, T::LParen) && !self.is_t(1, T::Dot) {
            self.bump_as(k::FUNCTION_IDENTIFIER);
            return true;
        }
        let call = self.is_t(1, T::LParen) && !NOT_OPERAND.contains(&up) || self.name_then_paren();
        if call {
            return self.function(c);
        }
        if self.reserved(0) && !self.is_t(1, T::Dot) {
            return false;
        }
        self.name_or_call(c)
    }

    /// `schema.name(`: a dotted name that ends in a bracket.
    pub(super) fn name_then_paren(&self) -> bool {
        let mut n = 0;
        while self.starts_name(n) && self.is_t(n + 1, T::Dot) {
            n += 2;
        }
        n > 0 && self.starts_name(n) && self.is_t(n + 1, T::LParen)
    }

    fn name_or_call(&mut self, c: &mut Chain) -> bool {
        if self.name_then_paren() || (self.starts_name(0) && self.is_t(1, T::LParen)) {
            return self.function(c);
        }
        self.obj_ref()
    }

    /// A function call: `name(args)` with what follows it (`FILTER`, `OVER`).
    fn function(&mut self, _c: &mut Chain) -> bool {
        let m = self.mark();
        let nm = self.mark();
        let nm_start = self.pos;
        loop {
            let dotted = self.is_t(1, T::Dot);
            if dotted {
                self.bump_as(if self.is_t(0, T::Quoted) {
                    k::QUOTED_IDENTIFIER
                } else {
                    k::IDENTIFIER
                });
                self.bump();
            } else {
                // Keywords of T-SQL that take a bracket.
                let keyword = self.d == super::Dialect::Tsql
                    && self.pos == nm_start
                    && [
                        "CONVERT",
                        "TRY_CONVERT",
                        "REPLICATE",
                        "ISJSON",
                        "OPENJSON",
                        "JSON_ARRAYAGG",
                        "JSON_OBJECT",
                    ]
                    .iter()
                    .any(|w| self.at(w));
                self.bump_as(if keyword {
                    k::KEYWORD
                } else {
                    k::FUNCTION_IDENTIFIER
                });
                break;
            }
        }
        if self.pos - nm > 1 {
            self.kinds[self.pos - 1] = k::WORD;
        }
        let name = self.src[self.lex[self.code[self.pos - 1] as usize].start as usize
            ..self.lex[self.code[self.pos - 1] as usize].end as usize]
            .to_ascii_uppercase();
        let dates = DATE_FUNCTIONS.contains(&name.as_str());
        self.close(k::FUNCTION_NAME, nm);
        self.type_first =
            self.d == super::Dialect::Tsql && matches!(name.as_str(), "CONVERT" | "TRY_CONVERT");
        self.function_contents(dates);
        self.function_tail();
        self.close(k::FUNCTION, m);
        true
    }

    pub fn function_contents(&mut self, dates: bool) {
        let m = self.mark();
        self.bracketed(|p| p.args(dates));
        self.close(k::FUNCTION_CONTENTS, m);
    }

    /// `FILTER (...)`, `WITHIN GROUP (...)`, `OVER ...`.
    fn function_tail(&mut self) {
        if self.at_any(&["ignore", "respect"]) && self.is(1, "nulls") {
            self.over_clause();
            return;
        }
        while self.at("filter") && self.is_t(1, T::LParen)
            || (self.at("within") && self.is(1, "group"))
        {
            self.bump_as(k::KEYWORD);
            self.eat("group");
            self.bracketed(|p| {
                if p.at("where") {
                    p.bump_as(k::KEYWORD);
                    p.expression();
                } else {
                    p.order_by();
                }
            });
        }
        if self.at("over") {
            self.over_clause();
        }
    }

    fn over_clause(&mut self) {
        let m = self.mark();
        self.indent();
        if self.at_any(&["ignore", "respect"]) {
            self.bump_as(k::KEYWORD);
            self.eat("nulls");
        }
        self.eat("over");
        if self.is_t(0, T::LParen) {
            self.window_spec();
        } else {
            self.eat_name();
        }
        self.dedent();
        self.close(k::OVER_CLAUSE, m);
    }

    /// `(name PARTITION BY .. ORDER BY .. ROWS ..)`.
    pub(super) fn window_spec(&mut self) {
        self.bracketed(|p| {
            let m = p.mark();
            if p.at_name() && !p.at_any(&["partition", "order", "rows", "range", "groups"]) {
                p.eat_name();
            }
            if p.at("partition") {
                p.partition_by();
            }
            if p.at("order") {
                p.order_by();
            }
            p.frame();
            p.close(k::WINDOW_SPECIFICATION, m);
        });
    }

    fn frame(&mut self) {
        if !self.at_any(&["rows", "range", "groups"]) {
            return;
        }
        self.bump_as(k::KEYWORD);
        while let Some(T::Word) = self.t(0) {
            if self.frame_word() {
                self.bump_as(k::KEYWORD);
            } else if self.chain(true).terms == 0 {
                break;
            }
        }
        while self.chain(true).terms > 0 {
            while self.frame_word() {
                self.bump_as(k::KEYWORD);
            }
        }
    }

    /// A word of a window frame clause at the cursor (not one that qualifies a name).
    fn frame_word(&self) -> bool {
        core_syntax::access::is_keyword_word(FRAME_WORDS, self.text(0), self.is_t(1, T::Dot))
    }

    pub fn partition_by(&mut self) {
        self.eat_seq(&["partition", "by"]);
        self.indent();
        self.list(|p| p.expression());
        self.dedent();
    }

    /// Arguments of a call: `*`, expressions, and the keywords between them.
    fn args(&mut self, dates: bool) {
        if self.at_op("*") && matches!(self.t(1), Some(T::RParen | T::Comma)) {
            self.bump();
            return;
        }
        loop {
            if self.at_any(&["distinct", "all"]) && !self.is_t(1, T::Comma) {
                self.bump_as(k::KEYWORD);
            }
            if self.d == super::Dialect::Postgres
                && self.at("variadic")
                && matches!(self.t(1), Some(T::Word | T::Str | T::Number))
            {
                self.bump_as(k::KEYWORD);
            }
            let before = self.pos;
            if std::mem::take(&mut self.type_first) && self.type_name() {
                // the type of `CONVERT`
            } else if self.at_any(&["signed", "unsigned"]) && self.d == super::Dialect::Mysql {
                self.bump_as(k::KEYWORD);
                self.eat_any(&["integer", "int"]);
            } else if dates
                && self.is_t(0, T::Word)
                && self.d.is_unit(self.text(0))
                && matches!(self.t(1), Some(T::Comma | T::RParen) | Some(T::Word))
            {
                self.bump_as(k::KEYWORD);
            } else if self.at_query_start() && self.at_any(&["select", "with"]) {
                let m = self.mark();
                self.query();
                self.close(k::EXPRESSION, m);
            } else {
                self.expression();
            }
            self.arg_tail();
            if self.is_t(0, T::Comma) {
                self.bump();
            } else if self.pos == before || self.is_t(0, T::RParen) || self.eof() {
                break;
            }
        }
    }

    /// The words inside an argument: `AS type`, `FROM x`, `FOR n`, `USING c`, `ORDER BY ...`.
    fn arg_tail(&mut self) {
        loop {
            if self.at("as") && self.is_t(1, T::Word) {
                self.bump_as(k::KEYWORD);
                if self.at_any(&["signed", "unsigned"]) && self.d == super::Dialect::Mysql {
                    self.bump_as(k::KEYWORD);
                    self.eat_any(&["integer", "int"]);
                } else if !self.type_name() {
                    self.expression();
                }
                if self.at("collate") {
                    self.bump_as(k::KEYWORD);
                    self.bump_as(k::IDENTIFIER);
                }
            } else if self.at("order") && self.is(1, "by") {
                self.order_by();
            } else if self.at_any(&["empty", "error"])
                && self.at_any_at(1, &["on", "array", "object"])
            {
                // `JSON_TABLE(... COLUMNS (...) EMPTY ARRAY ON ERROR)`
                while self.at_any(&["empty", "error", "array", "object", "on"]) {
                    self.bump_as(k::KEYWORD);
                }
            } else if self.at("columns") && self.is_t(1, T::LParen) {
                // `JSON_TABLE(doc, '$' COLUMNS (c int PATH '$.c'))`
                self.bump_as(k::KEYWORD);
                self.generic_columns_bracket();
            } else if self.at_any(&[
                "from",
                "for",
                "in",
                "using",
                "separator",
                "on",
                "returning",
                "with",
                "without",
                "placing",
            ]) {
                self.bump_as(k::KEYWORD);
                let typed = self.at("time")
                    && (self.is_t(1, T::Str) || self.is(1, "with") || self.is(1, "without"));
                if self.at_any(&["unique", "keys", "time"]) && !typed {
                    self.bump_as(k::KEYWORD);
                }
                if self.at_any(&["with", "without"]) && self.is(1, "time") && self.is(2, "zone") {
                    for _ in 0..3 {
                        self.bump_as(k::KEYWORD);
                    }
                }
                if !self.expression() {
                    break;
                }
            } else if self.at("value") || self.is_t(0, T::Colon) {
                if self.is_t(0, T::Colon) {
                    self.bump();
                } else {
                    self.bump_as(k::KEYWORD);
                }
                if !self.expression() {
                    break;
                }
            } else if self.at_any(&[
                "asc", "desc", "nulls", "first", "last", "both", "leading", "trailing",
            ]) {
                self.bump_as(k::KEYWORD);
            } else {
                break;
            }
        }
    }

    fn postfix(&mut self, c: &mut Chain) {
        loop {
            if self.is_t(0, T::Cast) {
                self.bump();
                if !self.type_name() {
                    self.chain(true);
                }
                c.lone = false;
            } else if self.is_t(0, T::LSquare) && self.d != super::Dialect::Tsql {
                self.array_accessor();
                c.lone = false;
            } else if self.is_t(0, T::Dot) && self.pos > 0 && self.kinds[self.pos - 1] == k::R_PAREN
            {
                self.bump();
                if self.is_t(0, T::Word) || self.is_t(0, T::Quoted) {
                    self.eat_name();
                } else if self.at_op("*") {
                    self.bump();
                }
                c.lone = false;
            } else if self.at("collate") && self.is_t(1, T::Word) | self.is_t(1, T::Quoted) {
                self.bump_as(k::KEYWORD);
                self.obj_ref();
                c.lone = false;
            } else if self.at("at") && self.is(1, "time") && self.is(2, "zone") {
                self.eat_seq(&["at", "time", "zone"]);
                self.operand(c);
                c.lone = false;
            } else {
                break;
            }
        }
    }

    /// `[i]`, `[i:j]`, `[1, 2]`.
    pub fn array_accessor(&mut self) {
        let m = self.mark();
        self.bump();
        self.indent();
        self.split();
        if self.depth < MAX_DEPTH {
            self.depth += 1;
            core_syntax::access::subscript_items(self);
            self.depth -= 1;
        }
        self.dedent();
        self.eat_t(T::RSquare);
        self.close(k::ARRAY_ACCESSOR, m);
    }

    /// The operator between two operands; false at the end of the chain.
    fn infix(&mut self, no_logic: bool, between: &mut u32) -> bool {
        match self.t(0) {
            Some(T::Op) => {
                let text = self.text(0);
                if is_compare(text) {
                    let m = self.mark();
                    self.bump();
                    self.close(k::COMPARISON_OPERATOR, m);
                } else if ARITH.contains(&text)
                    || (self.d == super::Dialect::Postgres && PG_BINARY.contains(&text))
                {
                    self.bump_as(k::BINARY_OPERATOR);
                } else if (text == "?" && self.d != super::Dialect::Postgres)
                    || text == "$"
                    || text == "\\"
                {
                    return false;
                } else {
                    self.bump();
                }
                true
            }
            Some(T::Word) => self.infix_word(no_logic, between),
            _ => false,
        }
    }

    fn infix_word(&mut self, no_logic: bool, between: &mut u32) -> bool {
        let mut buf = [0u8; 40];
        let up = super::dialect::upper(self.text(0), &mut buf);
        match up {
            "AND" if *between > 0 => {
                *between -= 1;
                self.bump_as(k::KEYWORD);
            }
            "AND" | "OR" if !no_logic => self.bump_as(k::BINARY_KEYWORD),
            "IS" => {
                self.bump_as(k::KEYWORD);
                self.eat("not");
                if self.eat_seq(&["distinct", "from"]) {
                    return true;
                }
                if self.at_any(&["null", "true", "false"]) {
                    self.bump_as(k::WORD_LITERAL);
                    return self.after_is() && self.infix(no_logic, between);
                }
                self.eat_any(&["unknown", "document"]);
                if self.eat("json") {
                    self.eat_any(&["value", "array", "object", "scalar"]);
                    if self.at_any(&["with", "without"]) && self.is(1, "unique") {
                        self.bump_as(k::KEYWORD);
                        self.bump_as(k::KEYWORD);
                        self.eat("keys");
                    }
                } else {
                    self.eat_any(&["nfc", "nfd", "nfkc", "nfkd"]);
                    self.eat("normalized");
                }
                return self.after_is() && self.infix(no_logic, between);
            }
            "NOT"
                if self.is(1, "in")
                    || self.is(1, "like")
                    || self.is(1, "between")
                    || self.is(1, "ilike")
                    || self.is(1, "similar")
                    || self.is(1, "rlike")
                    || self.is(1, "regexp")
                    || self.is(1, "glob") =>
            {
                self.bump_as(k::KEYWORD);
                return self.infix_word(no_logic, between);
            }
            "IN" => {
                self.bump_as(k::KEYWORD);
                if self.is_t(0, T::LParen) {
                    self.bracketed(|p| {
                        if p.at_query_start() {
                            p.query();
                            // `IN ((select 1), (select 2))`: the first one was a list element
                            if p.is_t(0, T::Comma) {
                                p.bump();
                                p.list(|p| p.element());
                            }
                        } else {
                            p.list(|p| p.element());
                        }
                    });
                    return self.after_is() && self.infix(no_logic, between);
                }
            }
            "BETWEEN" => {
                self.bump_as(k::KEYWORD);
                self.eat_any(&["symmetric", "asymmetric"]);
                *between += 1;
            }
            "LIKE" | "ILIKE" | "RLIKE" | "REGEXP" | "GLOB" | "MATCH" | "OVERLAPS" | "ESCAPE"
            | "MEMBER" => {
                self.bump_as(k::KEYWORD);
                if up == "MEMBER" {
                    self.eat("of");
                }
            }
            "SIMILAR" if self.is(1, "to") || self.is(1, "null") || self.is_t(1, T::Str) => {
                self.bump_as(k::KEYWORD);
                self.eat("to");
            }
            "ISNULL" | "NOTNULL" => {
                self.bump_as(k::KEYWORD);
                return self.after_is() && self.infix(no_logic, between);
            }
            "OPERATOR" if self.d == super::Dialect::Postgres && self.is_t(1, T::LParen) => {
                self.bump_as(k::KEYWORD);
                self.bracketed(|p| {
                    while !p.eof() && !p.is_t(0, T::RParen) {
                        p.bump();
                    }
                });
            }
            "DIV" | "MOD" | "XOR" if self.d == super::Dialect::Mysql => {
                self.bump_as(k::BINARY_OPERATOR)
            }
            _ => return false,
        }
        true
    }

    /// After `IS NULL` or `IN (..)` the chain may end.
    fn after_is(&mut self) -> bool {
        matches!(self.t(0), Some(T::Word | T::Op))
            && !self.at_any(&[
                "then", "else", "end", "when", "from", "where", "group", "order", "having",
                "limit", "union", "on", "as", "desc", "asc",
            ])
            && self.chain_continues()
    }

    fn chain_continues(&self) -> bool {
        match self.t(0) {
            Some(T::Op) => is_compare(self.text(0)) || ARITH.contains(&self.text(0)),
            Some(T::Word) => self.at_any(&[
                "and", "or", "is", "not", "in", "like", "between", "ilike", "collate",
            ]),
            _ => false,
        }
    }

    fn case_expr(&mut self) {
        let m = self.mark();
        self.bump_as(k::KEYWORD);
        if !self.at("when") {
            self.expression();
        }
        self.implicit();
        self.depth += 1;
        while self.at("when") && self.depth < MAX_DEPTH {
            let w = self.mark();
            self.bump_as(k::KEYWORD);
            self.implicit();
            self.expression();
            self.dedent();
            self.indent();
            self.eat("then");
            self.implicit();
            self.expression();
            self.dedent();
            self.dedent();
            self.close(k::WHEN_CLAUSE, w);
        }
        if self.at("else") {
            let e = self.mark();
            self.bump_as(k::KEYWORD);
            self.implicit();
            self.expression();
            self.dedent();
            self.close(k::ELSE_CLAUSE, e);
        }
        self.depth -= 1;
        self.dedent();
        if !self.eat("end") {
            self.error(self.pos, "missing END");
        }
        self.close(k::CASE_EXPRESSION, m);
    }
}
