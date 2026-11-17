//! Expressions: one flat chain of operands and operators, the way the tree keeps them (no
//! precedence nodes); brackets, calls, `CASE` and subqueries nest.

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
    "FROM", "WHERE", "AND", "OR", "ON", "USING", "THEN", "WHEN", "ELSE", "END", "IN", "IS", "AS",
    "BETWEEN", "LIKE", "GROUP", "ORDER", "HAVING", "LIMIT", "UNION", "INTERSECT", "EXCEPT", "SET",
    "VALUES", "JOIN", "INNER", "LEFT", "RIGHT", "FULL", "CROSS", "NATURAL", "OUTER", "WINDOW",
    "OFFSET", "FETCH", "FOR", "INTO", "RETURNING", "OVER", "ASC", "DESC", "WITH", "SELECT",
];

const BARE_FUNCTIONS: &[&str] = &[
    "CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP", "CURRENT_USER", "CURRENT_ROLE",
    "CURRENT_SCHEMA", "CURRENT_CATALOG", "LOCALTIME", "LOCALTIMESTAMP", "SESSION_USER",
    "SYSTEM_USER", "USER", "NOW",
];

const COMPARE: &[&str] = &["=", "<", ">", "<=", ">=", "<>", "!=", "!<", "!>", "<=>"];
const ARITH: &[&str] = &["+", "-", "*", "/", "%"];

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
        let mut c = Chain { terms: 0, lone: true };
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
            if self.at_any_op(&["~", "!!", "@", "|/", "||/", "#"]) && !self.is_t(1, T::Comma) {
                self.bump();
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
                self.bump();
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
                p.list(|p| p.expression());
            }
        });
    }

    pub fn at_query_start(&self) -> bool {
        self.at_any(&["select", "with", "values", "table"]) && !self.is_t(1, T::Dot)
            || (self.is_t(0, T::LParen) && self.paren_starts_query())
    }

    /// Does the bracket at the cursor (possibly several nested) open a query?
    fn paren_starts_query(&self) -> bool {
        let mut n = 0;
        while self.is_t(n, T::LParen) {
            n += 1;
        }
        self.is(n, "select") || self.is(n, "with")
    }

    fn word_operand(&mut self, c: &mut Chain) -> bool {
        let mut buf = [0u8; 40];
        let up = super::dialect::upper(self.text(0), &mut buf);
        match up {
            "NULL" | "TRUE" | "FALSE" if !self.is_t(1, T::Dot) => {
                self.bump_as(k::WORD_LITERAL);
                return true;
            }
            "CASE" => {
                self.case_expr();
                c.lone = false;
                return true;
            }
            "INTERVAL" if matches!(self.t(1), Some(T::Str | T::Number)) => {
                self.bump_as(k::KEYWORD);
                self.bump();
                if self.is_t(0, T::Word) && self.keyword_like(0) && !self.reserved(0) {
                    self.bump_as(k::KEYWORD);
                }
                return true;
            }
            "DATE" | "TIME" | "TIMESTAMP" | "DATETIME" | "TIMESTAMPTZ" if self.is_t(1, T::Str) => {
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
    fn name_then_paren(&self) -> bool {
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
        loop {
            let dotted = self.is_t(1, T::Dot);
            if dotted {
                self.bump_as(k::IDENTIFIER);
                self.bump();
            } else {
                self.bump_as(k::FUNCTION_IDENTIFIER);
                break;
            }
        }
        if self.pos - nm > 1 {
            self.kinds[self.pos - 1] = k::WORD;
        }
        self.close(k::FUNCTION_NAME, nm);
        self.function_contents();
        self.function_tail();
        self.close(k::FUNCTION, m);
        true
    }

    pub fn function_contents(&mut self) {
        let m = self.mark();
        self.bracketed(|p| p.args());
        self.close(k::FUNCTION_CONTENTS, m);
    }

    /// `FILTER (...)`, `WITHIN GROUP (...)`, `OVER ...`.
    fn function_tail(&mut self) {
        if self.at_any(&["ignore", "respect"]) && self.is(1, "nulls") {
            self.over_clause();
            return;
        }
        while self.at("filter") && self.is_t(1, T::LParen) || (self.at("within") && self.is(1, "group")) {
            self.bump_as(k::KEYWORD);
            self.eat("group");
            self.bracketed(|p| {
                if p.at("where") {
                    p.where_clause();
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
            self.bracketed(|p| {
                let m = p.mark();
                if p.at_name() && !p.at_any(&["partition", "order"]) {
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
        } else {
            self.eat_name();
        }
        self.dedent();
        self.close(k::OVER_CLAUSE, m);
    }

    fn frame(&mut self) {
        if !self.at_any(&["rows", "range", "groups"]) {
            return;
        }
        self.bump_as(k::KEYWORD);
        while let Some(T::Word) = self.t(0) {
            if self.at_any(&["between", "and", "unbounded", "preceding", "following", "current", "row", "exclude", "no", "others", "ties", "group"]) {
                self.bump_as(k::KEYWORD);
            } else if self.chain(true).terms == 0 {
                break;
            }
        }
        while self.chain(true).terms > 0 {
            while self.at_any(&["preceding", "following", "and"]) {
                self.bump_as(k::KEYWORD);
            }
        }
    }

    pub fn partition_by(&mut self) {
        self.eat_seq(&["partition", "by"]);
        self.indent();
        self.list(|p| p.expression());
        self.dedent();
    }

    /// Arguments of a call: `*`, expressions, and the keywords between them.
    fn args(&mut self) {
        if self.at_op("*") && matches!(self.t(1), Some(T::RParen | T::Comma)) {
            self.bump();
            return;
        }
        loop {
            if self.at_any(&["distinct", "all"]) && !self.is_t(1, T::Comma) {
                self.bump_as(k::KEYWORD);
            }
            let before = self.pos;
            if self.at_query_start() && self.at_any(&["select", "with"]) {
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
                if !self.type_name() {
                    self.expression();
                }
            } else if self.at("order") && self.is(1, "by") {
                self.order_by();
            } else if self.at_any(&["from", "for", "in", "using", "separator", "on", "returning", "with", "without"]) {
                self.bump_as(k::KEYWORD);
                if self.at_any(&["unique", "keys", "time"]) {
                    self.bump_as(k::KEYWORD);
                }
                if !self.expression() {
                    break;
                }
            } else if self.at_any(&["asc", "desc", "nulls", "first", "last", "both", "leading", "trailing"]) {
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
            } else if self.is_t(0, T::Dot) && self.pos > 0 && self.kinds[self.pos - 1] == k::R_PAREN {
                self.bump();
                if self.is_t(0, T::Word) || self.is_t(0, T::Quoted) {
                    self.eat_name();
                }
                c.lone = false;
            } else if self.at("collate") && self.is_t(1, T::Word) | self.is_t(1, T::Quoted) {
                self.bump_as(k::KEYWORD);
                self.obj_ref();
                c.lone = false;
            } else if self.at("at") && self.is(1, "time") && self.is(2, "zone") {
                self.eat_seq(&["at", "time", "zone"]);
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
            loop {
                if self.is_t(0, T::Colon) {
                    self.bump_as(k::SLICE);
                } else if !self.element() {
                    break;
                }
                if !(self.is_t(0, T::Comma) || self.is_t(0, T::Colon)) {
                    break;
                }
                if self.is_t(0, T::Comma) {
                    self.bump();
                }
            }
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
                if COMPARE.contains(&text) {
                    let m = self.mark();
                    self.bump();
                    self.close(k::COMPARISON_OPERATOR, m);
                } else if ARITH.contains(&text) {
                    self.bump_as(k::BINARY_OPERATOR);
                } else if text == "?" || text == "$" || text == "\\" {
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
                    return self.after_is();
                }
                self.eat_any(&["unknown", "document", "normalized"]);
                return self.after_is();
            }
            "NOT" if self.is(1, "in") || self.is(1, "like") || self.is(1, "between") || self.is(1, "ilike") || self.is(1, "similar") || self.is(1, "rlike") || self.is(1, "regexp") || self.is(1, "glob") => {
                self.bump_as(k::KEYWORD);
                return self.infix_word(no_logic, between);
            }
            "IN" => {
                self.bump_as(k::KEYWORD);
                if self.is_t(0, T::LParen) {
                    self.bracketed(|p| {
                        if p.at_query_start() {
                            p.query();
                        } else {
                            p.list(|p| p.element());
                        }
                    });
                    return self.after_is();
                }
            }
            "BETWEEN" => {
                self.bump_as(k::KEYWORD);
                self.eat_any(&["symmetric", "asymmetric"]);
                *between += 1;
            }
            "LIKE" | "ILIKE" | "RLIKE" | "REGEXP" | "GLOB" | "MATCH" | "OVERLAPS" | "ESCAPE" | "MEMBER" => {
                self.bump_as(k::KEYWORD);
                if up == "MEMBER" {
                    self.eat("of");
                }
            }
            "SIMILAR" if self.is(1, "to") => {
                self.eat_seq(&["similar", "to"]);
            }
            "ISNULL" | "NOTNULL" => self.bump_as(k::KEYWORD),
            "DIV" | "MOD" | "XOR" if self.d == super::Dialect::Mysql => self.bump_as(k::BINARY_OPERATOR),
            _ => return false,
        }
        true
    }

    /// After `IS NULL` or `IN (..)` the chain may end.
    fn after_is(&mut self) -> bool {
        matches!(self.t(0), Some(T::Word | T::Op)) && !self.at_any(&["then", "else", "end", "when", "from", "where", "group", "order", "having", "limit", "union", "on", "as", "desc", "asc"]) && self.chain_continues()
    }

    fn chain_continues(&self) -> bool {
        match self.t(0) {
            Some(T::Op) => COMPARE.contains(&self.text(0)) || ARITH.contains(&self.text(0)),
            Some(T::Word) => self.at_any(&["and", "or", "is", "not", "in", "like", "between", "ilike", "collate"]),
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
