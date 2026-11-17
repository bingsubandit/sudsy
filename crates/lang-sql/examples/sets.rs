//! Temporary: dumps a dialect's keyword sets and lexer matchers. `sets <dialect> <outdir>`.
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_dialects::kind_to_dialect;
use std::io::Write;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let kind = match a[1].as_str() {
        "postgres" => DialectKind::Postgres,
        "mysql" => DialectKind::Mysql,
        "tsql" => DialectKind::Tsql,
        _ => DialectKind::Ansi,
    };
    let d = kind_to_dialect(&kind, None).unwrap();
    for label in ["reserved_keywords", "unreserved_keywords", "bare_functions", "datetime_units"] {
        let mut v: Vec<&str> = d.sets(label).into_iter().collect();
        v.sort();
        let mut f = std::fs::File::create(format!("{}/{}.{}.txt", a[2], a[1], label)).unwrap();
        for w in v {
            writeln!(f, "{w}").unwrap();
        }
    }
    let mut f = std::fs::File::create(format!("{}/{}.lexer.txt", a[2], a[1])).unwrap();
    for m in d.lexer_matchers() {
        writeln!(f, "{m:?}").unwrap();
    }
}
