//! Data types: `int`, `varchar(10)`, `double precision`, `timestamp with time zone`, `int[]`.

use super::lexer::T;
use super::P;
use crate::generated::kind as k;

/// Words that can follow the first word of a type name.
const MORE: &[(&str, &[&str])] = &[
    ("DOUBLE", &["PRECISION"]),
    ("CHARACTER", &["VARYING", "LARGE"]),
    ("CHAR", &["VARYING"]),
    ("NATIONAL", &["CHARACTER", "CHAR"]),
    ("BINARY", &["VARYING", "LARGE"]),
    ("LONG", &["VARCHAR", "VARBINARY", "TEXT"]),
    ("BIT", &["VARYING"]),
    ("LARGE", &["OBJECT"]),
];

const TRAILERS: &[&str] = &["UNSIGNED", "SIGNED", "ZEROFILL"];

impl P<'_> {
    /// A data type; false when no type starts here.
    pub fn type_name(&mut self) -> bool {
        if !(self.is_t(0, T::Word) || self.is_t(0, T::Quoted)) || self.reserved_not_type(0) {
            return false;
        }
        // A qualified type name: the words before the last dot are names.
        while self.is_t(1, T::Dot) && self.starts_name(2) {
            self.eat_name();
            self.bump();
        }
        let head = self.text(0).to_ascii_uppercase();
        self.bump_as(k::TYPE_IDENTIFIER);
        if let Some((_, more)) = MORE.iter().find(|(w, _)| *w == head) {
            while self.at_any(more) {
                self.bump_as(k::TYPE_IDENTIFIER);
            }
        }
        self.type_args();
        // `timestamp (3) with time zone`, `time without time zone`.
        if matches!(head.as_str(), "TIMESTAMP" | "TIME" | "TIMESTAMPTZ")
            && self.at_any(&["WITH", "WITHOUT"])
            && self.is(1, "time")
        {
            self.bump_as(k::TYPE_IDENTIFIER);
            self.bump_as(k::TYPE_IDENTIFIER);
            self.bump_as(k::TYPE_IDENTIFIER);
        }
        while self.at_any(TRAILERS) {
            self.bump_as(k::TYPE_IDENTIFIER);
        }
        while self.is_t(0, T::LSquare) {
            self.bump();
            if !self.is_t(0, T::RSquare) {
                self.indent();
                self.split();
                self.expression();
                self.dedent();
            }
            self.eat_t(T::RSquare);
        }
        true
    }

    fn reserved_not_type(&self, n: usize) -> bool {
        self.is_t(n, T::Word) && self.reserved(n) && !is_type_word(self.text(n))
    }

    /// `(10)`, `(10, 2)`, `(max)`: the arguments of a type.
    pub fn type_args(&mut self) {
        if !self.is_t(0, T::LParen) {
            return;
        }
        let m = self.mark();
        self.bracketed(|p| {
            p.list(|p| {
                if p.is_t(0, T::Number) || p.is_t(0, T::Str) {
                    p.bump();
                    true
                } else if p.is_t(0, T::Word) && !p.is_t(1, T::LParen) {
                    let kind = if p.keyword_like(0) { k::KEYWORD } else { k::IDENTIFIER };
                    p.bump_as(kind);
                    true
                } else {
                    false
                }
            });
            // `char(10 byte)`, `decimal(10, 2)`.
            while p.is_t(0, T::Word) {
                p.bump_as(k::KEYWORD);
            }
        });
        self.close(k::BRACKETED_ARGUMENTS, m);
    }
}

const TYPE_WORDS: &[&str] = &[
    "INT", "INTEGER", "BIGINT", "SMALLINT", "TINYINT", "MEDIUMINT", "SERIAL", "BIGSERIAL", "SMALLSERIAL", "VARCHAR", "CHAR",
    "CHARACTER", "NVARCHAR", "NCHAR", "TEXT", "NTEXT", "STRING", "BOOLEAN", "BOOL", "BIT", "DATE", "TIME", "TIMESTAMP",
    "TIMESTAMPTZ", "DATETIME", "DATETIME2", "SMALLDATETIME", "INTERVAL", "NUMERIC", "DECIMAL", "NUMBER", "FLOAT", "REAL",
    "DOUBLE", "MONEY", "SMALLMONEY", "UUID", "UNIQUEIDENTIFIER", "JSON", "JSONB", "XML", "BYTEA", "BLOB", "CLOB",
    "BINARY", "VARBINARY", "LONGTEXT", "MEDIUMTEXT", "TINYTEXT", "LONGBLOB", "MEDIUMBLOB", "TINYBLOB", "YEAR", "ENUM",
    "GEOMETRY", "GEOGRAPHY", "INET", "CIDR", "MACADDR", "OID", "SQL_VARIANT", "IMAGE", "ROWVERSION", "TIMESTAMP",
];

/// Is `word` a data type name (a word that follows a column name in a definition)?
pub fn is_type_word(word: &str) -> bool {
    TYPE_WORDS.iter().any(|t| t.eq_ignore_ascii_case(word))
}
