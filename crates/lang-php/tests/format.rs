//! Layout of PER-CS as PHP-CS-Fixer applies it (docs/ai/formatting/php.md); every case also passes the
//! shared checks (round trip, meaning, idempotency).

use provider_api::format_testing::assert_formats;

#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_php::provider(), "a.php", input)
}

#[test]
fn spacing_and_braces_follow_per_cs() {
    assert_eq!(
        fmt(
            "<?php\nclass A extends B{\nfunction f($a,$b){\nif($a){return $a?1:2;}elseif ($b)\n{foo( $a , $b );}\n}\n}\n"
        ),
        "<?php\n\nclass A extends B\n{\n    public function f($a, $b)\n    {\n        if ($a) {\n            return $a ? 1 : 2;\n        } elseif ($b) {\n            foo($a, $b);\n        }\n    }\n}\n"
    );
}

#[test]
fn multiline_lists_get_one_item_per_line_and_a_trailing_comma() {
    assert_eq!(
        fmt(
            "<?php\n\nfoo($a,\n    $b);\nfoo($a, function () {\n    x();\n}, $b);\n$x = [\n    1, 2,\n    3\n];\nfunction f($a,\n    $b) {}\n"
        ),
        "<?php\n\nfoo(\n    $a,\n    $b,\n);\nfoo($a, function () {\n    x();\n}, $b);\n$x = [\n    1, 2,\n    3,\n];\nfunction f(\n    $a,\n    $b,\n) {}\n"
    );
}

#[test]
fn source_spacing_stays_where_per_cs_has_no_rule() {
    assert_eq!(
        fmt("<?php\n\n$aaaa   = 1;\n$z = [  'a'  =>  1 ,  'bb' => 2 ];\n"),
        "<?php\n\n$aaaa   = 1;\n$z = [  'a'  =>  1 ,  'bb' => 2 ];\n"
    );
}

#[test]
fn code_rewrites_of_per_cs() {
    assert_eq!(
        fmt(
            "<?php\n$a = array(1, 2);\nclass A { var $x; function f(int|null $a) {} }\n$o = new B;\n"
        ),
        "<?php\n\n$a = [1, 2];\nclass A\n{\n    public $x;\n    public function f(int|null $a) {}\n}\n$o = new B();\n"
    );
}

#[test]
fn blank_lines_and_comments_stay() {
    assert_eq!(
        fmt(
            "<?php\n// c\n$a = 1;\n\n\n/* d */\n\n\n$b = 2;\nif ($a):\n    foo();\nelse:\n    bar();\nendif;\n"
        ),
        "<?php\n\n// c\n$a = 1;\n\n\n/* d */\n\n\n$b = 2;\nif ($a):\n    foo();\nelse:\n    bar();\nendif;\n"
    );
}

#[test]
fn comments_beside_operators_and_closers_are_stable() {
    // (assert_formats checks the second format changes nothing)
    assert_eq!(
        fmt("<?php\n\nif ($a || // one\n    $b\n    // two\n) {\n    foo(1 /* c */\n    );\n}\n"),
        "<?php\n\nif ($a // one\n    || $b\n// two\n) {\n    foo(\n        1, /* c */\n    );\n}\n"
    );
}

/// A language that has no `column_list_min_commas` option has no column lists, and a braced list formats.
#[test]
fn a_braced_list_formats_without_the_column_list_option() {
    let src = "<?php\n\nclass A\n{\n    use T {\n        a as b;\n    }\n}\n";
    assert_eq!(fmt(src), src);
}
