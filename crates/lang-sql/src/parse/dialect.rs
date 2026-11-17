//! Dialect modes of the one parser: keyword tables and the few syntax switches.

use std::collections::{HashMap, HashSet};
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

    /// A date part (`DAY`, `HOUR`): a keyword in the first argument of the date functions.
    pub fn is_unit(self, word: &str) -> bool {
        let mut buf = [0u8; 40];
        sets().units.contains(upper(word, &mut buf))
    }

    /// The statement table's row of a statement that starts `first second`: the words that are
    /// keywords in it (`W>`: the word after `W` is an identifier). The two-word row, else the one-word row.
    pub fn statement_row(self, first: &str, second: &str) -> Option<&'static HashSet<&'static str>> {
        let name = ["ansi", "postgres", "mysql", "tsql"][self as usize];
        let (mut a, mut b) = ([0u8; 40], [0u8; 40]);
        let (f, s) = (upper(first, &mut a), upper(second, &mut b));
        let rows = &sets().rows;
        rows.get(format!("{name}|{f} {s}").as_str()).or_else(|| rows.get(format!("{name}|{f}").as_str()))
    }

    /// A keyword of this dialect: what a word that no rule claims is taken for.
    pub fn is_keyword(self, upper: &str) -> bool {
        let letter = ['a', 'p', 'm', 't'][self as usize];
        sets().all.get(upper).is_some_and(|d| d.contains(letter) || d.contains('a')) || self.is_reserved(upper)
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
    /// Keywords by dialect: the letters `a`, `p`, `m`, `t` of the dialects that have the word.
    all: HashMap<&'static str, &'static str>,
    units: HashSet<&'static str>,
    /// `dialect|FIRST SECOND` (or `dialect|FIRST`) -> the keywords of that statement.
    rows: HashMap<&'static str, HashSet<&'static str>>,
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
        all: include_str!("data/keywords.txt").lines().filter_map(|l| l.split_once(' ')).collect(),
        units: words(include_str!("data/units.txt")),
        rows: include_str!("data/statements.txt")
            .lines()
            .filter_map(|l| {
                let (key, list) = l.rsplit_once('|')?;
                Some((key, list.split(' ').collect()))
            })
            .collect(),
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
