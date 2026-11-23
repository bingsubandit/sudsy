//! Parse, round trip and the shared format checks on SQL probes and the bench inputs.

use core_syntax::{print, trivia};
use lang_sql::generated::kind;
use provider_api::format_testing::{Formatting, assert_file_names};
use provider_api::{LanguageProvider, SourceFile};

fn parse(text: &str, dialect: &str) -> provider_api::ParseResult {
    let mut file = SourceFile::new("a.sql", 1, text);
    file.language_version = Some(dialect.into());
    lang_sql::provider().parse(&file).unwrap()
}

/// Every check on one file in one dialect.
fn check(text: &str, dialect: &str) {
    let parsed = parse(text, dialect);
    assert!(
        parsed.errors.is_empty(),
        "{dialect}: {:?}",
        parsed.errors.first()
    );
    assert_eq!(print(&parsed.tree), text, "{dialect}: round trip");
    let grammar = lang_sql::provider().grammar().clone();
    trivia::check_attachment(&parsed.tree, |k| grammar.is_trivia(k))
        .unwrap_or_else(|e| panic!("{dialect}: {e}"));
    let unknown = (parsed.tree.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind::UNKNOWN);
    assert!(unknown.is_none(), "{dialect}: unknown token {unknown:?}");
    // The shared subject parses by file name, which is ansi.
    if dialect == "ansi" {
        core_format::testing::assert_formats(&Formatting::new(lang_sql::provider(), "a.sql"), text);
    }
}

const PROBE: &str = "-- header
SELECT a, b AS c, /* inline */ count(*) -- trailing
FROM t1 JOIN t2 ON t1.id = t2.id
WHERE x IN (1, 2, 3) AND y = 'it''s'
GROUP BY a;

WITH cte AS (SELECT 1) SELECT * FROM cte;
INSERT INTO t (a) VALUES (1);
";

#[test]
fn probes_in_every_dialect() {
    for dialect in lang_sql::DIALECTS {
        check(PROBE, dialect);
    }
    check("", "ansi");
    check("\n\n", "ansi");
    check("-- only a comment", "ansi");
    check("SELECT [a b] FROM [t];", "tsql");
    check("SELECT `a` FROM `t` LIMIT 1;", "mysql");
    check("SELECT a::int, $1 FROM t;", "postgres");
}

/// Constructs the dialect parsers once refused: each parses without errors and round-trips.
#[test]
fn dialect_constructs_parse() {
    let postgres = [
        "SELECT (@@ f1) AS c FROM t;",
        "SELECT overlay(B'01' placing '001' from 2 for 3);",
        "SELECT CAST('42' AS text COLLATE \"C\");",
        "SELECT concat_ws(',', variadic 10);",
        "SELECT EXTRACT(MICROSECOND FROM TIME WITH TIME ZONE '2020-05-26 13:30:25-04');",
        "SELECT * FROM (TABLE t) AS s (a, b);",
        "TABLE t;",
        "SELECT (c AT TIME ZONE 'Africa/Cairo')::time FROM t;",
        "REPACK t USING INDEX;",
        "SELECT * FROM a CROSS JOIN LATERAL f(x) AS t(c1 text, c2 bool);",
        "SELECT 1 WHERE h = ANY ((SELECT 1)::int[]);",
        "INSERT INTO t (a[1:5], b[1].q, c) VALUES (1, 2, 3);",
        "INSERT INTO t VALUES (1) ON CONFLICT (k int4_ops (fillfactor = 10), j COLLATE \"C\" DESC) DO NOTHING;",
        "SELECT * FROM (a JOIN b USING (i) AS x) AS xx;",
        "SELECT * FROM (a LEFT JOIN b FULL JOIN c ON false ON false);",
        "SELECT 1 FROM t GROUP BY GROUPING SETS ((id), (val));",
        "SELECT sum(u) OVER (ROWS 2 PRECEDING EXCLUDE GROUP), SUBSTRING('a' SIMILAR NULL ESCAPE '#');",
        "SELECT a WHERE b IN ((SELECT 1), (SELECT 2));",
        "CREATE TABLE t (c int, CONSTRAINT x EXCLUDE (f WITH =) INITIALLY DEFERRED);",
        "WITH RECURSIVE s (f) AS (SELECT 1) CYCLE f SET c TO 'Y' DEFAULT 'N' USING path SELECT 1;",
        "COPY (SELECT t INTO TEMP x FROM y) TO STDOUT;",
        "CREATE RULE r AS ON INSERT TO t DO INSTEAD (DELETE FROM t; NOTIFY c);",
        "INSERT INTO foo DEFAULT VALUES RETURNING WITH (OLD AS o, NEW AS n) name, o.p;",
        "WITH foo AS (MERGE INTO t USING s ON (true) WHEN MATCHED THEN DELETE) SELECT * FROM foo;",
    ];
    for text in postgres {
        check(text, "postgres");
    }
    let tsql = [
        "SELECT CASE WHEN 1 > = 1 THEN 1 WHEN 1 !\n < 1 THEN 2 WHEN 1 ! = 1 THEN 3 END;",
        "UPDATE p WITH (TABLOCK) SET a = 1 WHERE b = 2;",
        "EXECUTE dbo.Proc @p2 = 'A' WITH RECOMPILE;",
        "EXECUTE ('x') AS USER = 'dbo' WITH RESULT SETS ((a INT NOT NULL, b INT));",
        "DECLARE @x XML (DOCUMENT dbo.S);",
        "SELECT * FROM OPENJSON(@j) WITH (m VARCHAR(3), n tinyint '$.n') AS t;",
        "CREATE FUNCTION f () RETURNS @t TABLE (i int) AS BEGIN RETURN END;",
        "SELECT 'a' 'b', 'c' AS 'd';",
        "DECLARE @x XML (dbo.SomeSchemaCollection);",
        "SELECT * FROM Sales.Customer TABLESAMPLE SYSTEM (10 PERCENT);",
    ];
    for text in tsql {
        check(text, "tsql");
    }
    for text in [
        "ALTER TABLE t MODIFY COLUMN c varchar(255) NOT NULL;",
        "ALTER TABLE t CHANGE COLUMN a b INT(11) NULL;",
        "SELECT DATE_ADD(CURDATE(), INTERVAL -30 DAY), @@global.time_zone;",
        "REPLACE tbl VALUE (1, 2), (11, 22);",
    ] {
        check(text, "mysql");
    }
}

#[test]
fn syntax_errors_are_reported_and_lossless() {
    for text in ["SELEC FROM WHERE ;;", "SELECT (1;", "select 'unterminated"] {
        let parsed = parse(text, "ansi");
        assert!(!parsed.errors.is_empty(), "{text:?}");
        assert_eq!(print(&parsed.tree), text);
    }
}

/// Deep nesting is an error or flat tokens, never a stack overflow (a 1 MB stack here).
#[test]
fn deep_nesting_does_not_overflow() {
    let n = 20_000;
    let inputs = [
        format!("SELECT {}1{};", "(".repeat(n), ")".repeat(n)),
        format!(
            "{}SELECT 1{};",
            "SELECT * FROM (".repeat(n),
            ") t".repeat(n)
        ),
        format!(
            "SELECT {}1{};",
            "CASE WHEN a THEN ".repeat(n),
            " END".repeat(n)
        ),
        format!("SELECT {}a;", "NOT ".repeat(n)),
        format!("SELECT {}a;", "- ".repeat(n)),
        format!("SELECT {}1{};", "ARRAY[".repeat(n), "]".repeat(n)),
        format!("SELECT {}a;", "f(".repeat(n)),
    ];
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            for text in &inputs {
                for dialect in lang_sql::DIALECTS {
                    assert_eq!(print(&lang_sql::parse(text, dialect).tree), *text);
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn nodes_are_named_after_segment_types() {
    let parsed = parse("SELECT a FROM t WHERE a = 1;", "ansi");
    let kinds: Vec<_> = parsed.tree.descendants().map(|n| n.kind()).collect();
    for want in [
        kind::SELECT_STATEMENT,
        kind::SELECT_CLAUSE,
        kind::FROM_CLAUSE,
        kind::WHERE_CLAUSE,
        kind::OBJECT_REFERENCE,
    ] {
        assert!(kinds.contains(&want), "{want:?}");
    }
}

#[test]
fn bench_inputs() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/inputs/sql");
    for entry in std::fs::read_dir(dir).unwrap() {
        check(
            &std::fs::read_to_string(entry.unwrap().path()).unwrap(),
            "ansi",
        );
    }
}

#[test]
fn matches_the_extension() {
    assert_file_names(
        &lang_sql::provider(),
        &["a.sql", "dir/b.SQL"],
        &["a.sqlx", "sql"],
    );
}

/// A backslash escapes the quote in MySQL and ANSI, and is a character in PostgreSQL and T-SQL.
#[test]
fn backslash_escape_depends_on_the_dialect() {
    let strings = |text: &str, dialect: &str| {
        let parsed = parse(text, dialect);
        (parsed.tree.descendants_with_tokens())
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == kind::STRING)
            .map(|t| t.text().to_string())
            .collect::<Vec<_>>()
    };
    let text = r"SELECT 'a\' + 'b';";
    assert_eq!(strings(text, "mysql")[0], r"'a\' + '");
    assert_eq!(strings(text, "postgres"), [r"'a\'", "'b'"]);
    assert_eq!(strings(r"SELECT E'a\'b';", "postgres"), [r"E'a\'b'"]);
}
