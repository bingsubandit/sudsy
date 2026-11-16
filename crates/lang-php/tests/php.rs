use provider_api::format_testing::{
    assert_file_names, assert_formats, assert_lossless, assert_parses, bench_input_files,
    check_corpus,
};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
#[allow(dead_code)]
fn fmt(input: &str) -> String {
    assert_formats(lang_php::provider(), "a.php", input)
}

/// Every `.php` file of the pinned corpus (`tests/conformance/fetch-php-corpus.sh`) parses,
/// formats with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-php --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (_, unparsed) = check_corpus(lang_php::provider(), "a.php", "php", &["php"]);
    assert!(
        unparsed.is_empty(),
        "valid files with syntax errors: {unparsed:#?}"
    );
}

#[test]
fn file_names() {
    assert_file_names(
        &lang_php::provider(),
        &["a.php", "views/a.phtml", "lib/a.inc"],
        &["a.phpt", "a.js", "composer.json"],
    );
}

#[test]
fn bench_inputs_parse() {
    for (name, text) in bench_input_files("php") {
        assert_parses(&lang_php::provider(), &name, &text);
    }
}

/// Inline HTML around the tags, `?>` ending a line comment, short echo tags, alternative syntax.
#[test]
fn inline_html_and_tags_parse() {
    let p = lang_php::provider();
    assert_parses(
        &p,
        "a.phtml",
        "#!/usr/bin/env php\n<html>\n<?php foreach ($xs as $x): // each ?>\n  <li><?= $x ?></li>\n<?php endforeach ?>\n</html>\n",
    );
}

/// Syntax of PHP 8.0 to 8.5: attributes, enums, readonly, property hooks, match, first-class
/// callables, named arguments, heredocs, interpolation, the pipe operator.
#[test]
fn php_8_syntax_parses() {
    assert_parses(
        &lang_php::provider(),
        "a.php",
        r#"<?php
declare(strict_types=1);
namespace App\Models;
use Foo\{Bar, Baz as Qux};
#[Attribute(Attribute::TARGET_CLASS)]
final class A extends B implements C {
    use T { f as protected g; }
    public const VOID = 1;
    public function __construct(private readonly int|string $x = 1, public private(set) ?array $y = null) {}
    public string $name { get => $this->name; set (string $v) { $this->name = trim($v); } }
    public static function f(int ...$xs): static { return new static(...$xs); }
}
enum Suit: string { case Hearts = 'H'; case Spades = 'S'; }
$f = strlen(...);
$g = fn($x) => $x * 2;
$r = match (true) { $a > 1, $b => 'x', default => throw new E("m"), };
$s = "a {$b['c']} $d[0] ${e} $f->g";
$h = <<<EOT
    text $x {$y->z}
    EOT;
$n = <<<'RAW'
raw $x
RAW;
$v = $x |> trim(...) |> strlen(...);
list('a' => $p, [$q, , $r]) = $arr;
echo (int) $x, (string) $y, (void) $z;
"#,
    );
}

/// Inline text is output: a file that ends in it keeps its end byte for byte.
#[test]
fn inline_text_at_the_end_is_kept() {
    for s in [
        "<p>hi</p>\n\n",
        "<p>hi</p>",
        "<?php\n$a = 1;\n?>\n<p>x</p>\n\n",
        "<?php $a;\n?>\n  ",
    ] {
        assert_eq!(fmt(s), s);
    }
}

#[test]
fn syntax_errors_stay_lossless() {
    let parsed = assert_lossless(
        &lang_php::provider(),
        "a.php",
        "<?php\nclass A { function f( { \n",
    );
    assert!(!parsed.errors.is_empty());
}
