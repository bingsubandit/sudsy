use core_format::doc::*;
use core_format::printer::{Continuation, PrintOptions, print};

fn continued(mut d: Doc) -> String {
    let opts = PrintOptions {
        continuation: Some(Continuation {
            escape: '\\',
            comment: '#',
        }),
        ..PrintOptions::default()
    };
    print(&mut d, &opts)
}

#[test]
fn every_line_break_of_code_is_a_continuation() {
    let d = concat(vec![
        text("a"),
        indent(concat(vec![
            hardline(),
            text("&& b"),
            hardline(),
            text("&& c"),
        ])),
    ]);
    assert_eq!(continued(d), "a \\\n  && b \\\n  && c");
}

#[test]
fn a_comment_line_and_a_line_inside_text_end_without_one() {
    let d = concat(vec![
        text("a"),
        hardline(),
        text("# note"),
        hardline(),
        text("echo \"x  \n  y\""),
        hardline(),
        text("b"),
    ]);
    assert_eq!(continued(d), "a \\\n# note\necho \"x  \n  y\" \\\nb");
}

#[test]
fn a_blank_line_gets_the_escape_character_alone() {
    let d = concat(vec![text("a"), hardline(), hardline(), text("b")]);
    assert_eq!(continued(d), "a \\\n\\\nb");
}
