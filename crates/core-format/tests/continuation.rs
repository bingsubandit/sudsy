use core_format::shapes::continuation::{Continued, quote_starts};

#[test]
fn quote_lines_are_found() {
    assert_eq!(quote_starts("a \"x \\\n y\" \\\n b"), [false, true, false]);
    // a backslash escapes a double quote, not a single one
    assert_eq!(quote_starts("a \"\\\"\" \\\n b"), [false, false]);
    assert_eq!(quote_starts("a 'x\\' \\\n b"), [false, false]);
    // an apostrophe in a comment line opens no quote
    assert_eq!(quote_starts("a \\\n# it's\n b"), [false, false, false]);
}

#[test]
fn comments_go_before_their_line() {
    let laid = "a \\\n&& b \\\n&& c";
    let source = "a \\\n# one\n&& b \\\n# two\n&& c";
    assert_eq!(
        Continued::place_comments(source, laid),
        Some(vec![(1, "# one".to_string()), (2, "# two".to_string())])
    );
    // comments before the same line share its place
    assert_eq!(
        Continued::place_comments("a \\\n# x\n  # y\n&& b", "a \\\n&& b"),
        Some(vec![(1, "# x".to_string()), (1, "# y".to_string())])
    );
    // a comment whose next line is not in the laid-out text has no place
    assert_eq!(Continued::place_comments("a \\\n# x\nz", "a"), None);
}
