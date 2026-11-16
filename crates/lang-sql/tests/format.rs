//! Every expectation passes the shared checks (lossless, meaning, comments, idempotency).

use core_format::DirectiveTable;
use core_format::testing::assert_formats;
use provider_api::format::format_file;
use provider_api::format_testing::Formatting;
use provider_api::{LanguageProvider, SourceFile};

#[test]
fn lays_out_lists_and_keeps_comments() {
    let input = "-- header\nSELECT /* inline */ a,b -- trailing\nFROM t;\n/* block\n   comment */\nselect 1;\n";
    let subject = Formatting::new(lang_sql::provider(), "a.sql");
    let want = "-- header\nSELECT /* inline */\n    a,\n    b -- trailing\nFROM t;\n/* block\n   comment */\nselect 1;\n";
    assert_eq!(assert_formats(&subject, input), want);
}

fn formats(input: &str) -> String {
    assert_formats(&Formatting::new(lang_sql::provider(), "a.sql"), input)
}

#[test]
fn long_lines_break_at_the_last_bracket_and_the_commas() {
    let call = "select a from t where x = sch.fname(arg_number_one, arg_number_two, arg_number_three, arg_number_four);\n";
    assert_eq!(
        formats(call),
        "select a from t\nwhere x = sch.fname(\n    arg_number_one,\n    arg_number_two,\n    arg_number_three,\n    arg_number_four\n);\n"
    );
    let list = "select * from table_one, table_two, table_three, table_four, table_five, table_six, table_seven, table_eight, table_nine;\n";
    assert_eq!(
        formats(list),
        "select *\nfrom table_one,\n    table_two,\n    table_three,\n    table_four,\n    table_five,\n    table_six,\n    table_seven,\n    table_eight,\n    table_nine;\n"
    );
}

#[test]
fn a_trailing_comment_over_the_width_moves_above() {
    let input = "select a from t where b = 1 and c = 2 and d = 3 and e = 4 -- a long comment that does not fit on the line\n;\n";
    let out = formats(input);
    assert!(
        out.starts_with("select a from t\n-- a long comment"),
        "{out}"
    );
    assert!(!out.contains("4 --"), "{out}");
}

#[test]
fn spacing_of_names_brackets_and_prefix_operators() {
    assert_eq!(
        formats("select max (a) from t;\n"),
        "select max(a) from t;\n"
    );
    assert_eq!(
        formats("select ~ 1, ~ ~ 2;\n"),
        "select\n    ~1,\n    ~ ~2;\n"
    );
    assert_eq!(
        formats("with c as\n(select 1)\nselect * from c;\n"),
        "with c as (select 1)\n\nselect * from c;\n"
    );
}

#[test]
fn errors_are_refused() {
    let provider = lang_sql::provider();
    let file = SourceFile::new("a.sql", 1, "SELEC FROM WHERE ;;");
    let options = provider.option_schema().defaults();
    assert!(format_file(&provider, &file, &options, &DirectiveTable::default()).is_err());
}
