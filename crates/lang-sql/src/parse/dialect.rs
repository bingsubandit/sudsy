//! Dialect modes of the one parser: keyword tables and the few syntax switches.

use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialect {
    Ansi,
    Postgres,
    Mysql,
    Tsql,
}

impl Dialect {
    pub fn from_name(name: &str) -> Dialect {
        match name {
            "postgres" => Dialect::Postgres,
            "mysql" => Dialect::Mysql,
            "tsql" => Dialect::Tsql,
            _ => Dialect::Ansi,
        }
    }

    /// Reserved words: they never name something unquoted.
    pub fn is_reserved(self, upper: &str) -> bool {
        sets().reserved[self as usize].contains(upper) || CLAUSE_WORDS.contains(&upper)
    }

    /// Any keyword of any dialect: what a word that no rule claims is taken for.
    pub fn is_keyword(self, upper: &str) -> bool {
        sets().all.contains(upper) || self.is_reserved(upper)
    }
}

/// Words that start a clause or join a statement to the next: they end an expression and cannot
/// be an alias in any dialect.
const CLAUSE_WORDS: &[&str] = &[
    "FROM", "WHERE", "GROUP", "HAVING", "ORDER", "LIMIT", "OFFSET", "FETCH", "UNION", "INTERSECT",
    "EXCEPT", "MINUS", "WINDOW", "ON", "USING", "INTO", "VALUES", "RETURNING", "FOR", "WHEN",
    "THEN", "ELSE", "END", "JOIN", "INNER", "LEFT", "RIGHT", "FULL", "CROSS", "NATURAL", "OUTER",
    "SELECT", "SET", "AS", "AND", "OR", "NOT", "IS", "IN", "LIKE", "BETWEEN", "ILIKE", "ESCAPE",
    "CASE", "WITH", "OVER", "TABLE", "STRAIGHT_JOIN", "APPLY", "LATERAL", "QUALIFY", "CONNECT",
    "START", "PIVOT", "UNPIVOT", "TABLESAMPLE", "OPTION", "ISNULL", "NOTNULL", "COLLATE",
    "SIMILAR", "OVERLAPS", "ANY", "ALL", "SOME", "EXISTS", "TO",
];

struct Sets {
    reserved: [HashSet<&'static str>; 4],
    all: HashSet<&'static str>,
}

fn words(text: &'static str) -> HashSet<&'static str> {
    text.lines().filter(|l| !l.is_empty()).collect()
}

fn sets() -> &'static Sets {
    static SETS: OnceLock<Sets> = OnceLock::new();
    SETS.get_or_init(|| Sets {
        reserved: [
            words(include_str!("data/ansi.reserved.txt")),
            words(include_str!("data/postgres.reserved.txt")),
            words(include_str!("data/mysql.reserved.txt")),
            words(include_str!("data/tsql.reserved.txt")),
        ],
        all: words(include_str!("data/keywords.txt")),
    })
}

/// Uppercases `word` into `buf` (words longer than the buffer are no keyword).
pub fn upper<'a>(word: &str, buf: &'a mut [u8; 40]) -> &'a str {
    if word.len() > buf.len() || !word.is_ascii() {
        return "";
    }
    let out = &mut buf[..word.len()];
    out.copy_from_slice(word.as_bytes());
    out.make_ascii_uppercase();
    std::str::from_utf8(out).unwrap_or("")
}
