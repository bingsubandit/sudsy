//! Parse, round trip and the shared format checks on the bench inputs and on probes of recent
//! syntax: every file parses without errors, prints back byte for byte, satisfies the trivia
//! attachment rule and has no `Unknown` token. The ignored test runs the same on the corpus
//! (`tests/conformance/fetch-kotlin-corpus.sh`).

use core_syntax::{print, trivia};
use lang_kotlin::generated::kind;
use lang_kotlin::provider;
use provider_api::format_testing::{
    Formatting, assert_file_names, assert_parses, bench_input_files, check_corpus,
};
use provider_api::{LanguageProvider, SourceFile};

fn parse(name: &str, text: &str) -> provider_api::ParseResult {
    provider().parse(&SourceFile::new(name, 1, text)).unwrap()
}

/// The text parses, prints back and satisfies the attachment rule.
#[track_caller]
fn check(name: &str, text: &str) {
    assert_parses(&provider(), name, text);
}

/// Syntax of Kotlin 1.x and 2.x: context parameters, guards, multi-dollar strings, backing
/// fields, value classes, `expect`/`actual`, contracts, nested comments and strings.
const PROBES: &str = r##"#!/usr/bin/env kotlin
@file:Suppress("a")
package a.b // trailing
import a.b.C as D
/* a /* nested */ b */
/** KDoc [x] */
@JvmInline value class Id(val v: Int)
expect class E { fun f() }
class W<T> : Comparable<T> where T : Any { override fun compareTo(o: T) = 0 }
actual typealias T = String
data object X
context(a: A, _: Logger)
fun g() {}
fun interface F { fun run(x: Int): Int }
sealed interface S { data object A : S }
class C<T : Any>(val x: Int, private val y: String = "s") : Base(), I by d {
    val city: StateFlow<String>
        field = MutableStateFlow("")
    var n = 0
        private set
    constructor() : this(1)
    companion object { const val K = 1_000L }
}
fun <T> g(x: T & Any, vararg s: String, f: suspend (Int) -> Unit = {}): Int? = when (val y = x) {
    is String if y.isNotEmpty() -> 1
    in 1..5, !in 6..9 -> 2
    else -> null
}
val s = $$"price: $x $${y} $$$z"
val t = """
    raw $x ${y + 1}
"""
val r = 0..<10
val `my val` = if (a) 1 else 2
fun h() { for ((k, v) in m) { @Suppress("x") val z = a?.b!!.c ?: return }; loop@ while (true) break@loop }
"##;

#[test]
fn kotlin_syntax_parses() {
    check("A.kt", PROBES);
}

#[test]
fn scripts_parse_as_scripts() {
    let gradle = "plugins {\n    id(\"x\")\n}\ndependencies { implementation(\"a:b:1\") }\n";
    check("build.gradle.kts", gradle);
    assert!(!parse("a.kt", gradle).errors.is_empty());
}

#[test]
fn crlf_text_keeps_its_line_breaks() {
    let text = PROBES.replace('\n', "\r\n");
    check("A.kt", &text);
    let tree = parse("A.kt", &text).tree;
    assert_eq!(print(&tree), text);
}

#[test]
fn syntax_errors_are_reported() {
    let parsed = parse("A.kt", "fun f( { }\nval x = 1\n");
    assert!(!parsed.errors.is_empty());
    assert_eq!(print(&parsed.tree), "fun f( { }\nval x = 1\n");
    let grammar = provider().grammar().clone();
    trivia::check_attachment(&parsed.tree, |k| grammar.is_trivia(k)).unwrap();
}

/// Shapes the parser cannot take are refused as one error and the file stays as written.
#[test]
fn hostile_nesting_is_refused() {
    for text in [
        format!("val x = {}1{}\n", "(".repeat(3000), ")".repeat(3000)),
        format!("val x = {}a\n", "!".repeat(5000)),
        format!("@A({}) class B\n", "@A(".repeat(30) + &")".repeat(30)),
    ] {
        let parsed = parse("A.kt", &text);
        assert_eq!(parsed.errors.len(), 1);
        assert_eq!(print(&parsed.tree), text);
    }
    // Deep but allowed: parses on the large stack.
    check(
        "A.kt",
        &format!("val x = {}1{}\n", "(".repeat(1500), ")".repeat(1500)),
    );
}

#[test]
fn no_unknown_tokens_in_probes() {
    let tree = parse("A.kt", PROBES).tree;
    let unknown = (tree.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind::UNKNOWN);
    assert!(unknown.is_none(), "{unknown:?}");
}

#[test]
fn file_names() {
    assert_file_names(
        &provider(),
        &["A.kt", "build.gradle.kts", "src/main/kotlin/a/B.kt"],
        &["A.kotlin", "A.k", "A.ktx"],
    );
}

#[test]
fn bench_inputs_parse() {
    for (name, text) in bench_input_files("kt") {
        check(&name, &text);
    }
}

/// Every file of the pinned corpus parses, formats with the same tokens (the safety check) and
/// idempotently. The two BOM-prefixed test inputs of ktlint are read without the BOM.
///
/// `cargo test --profile corpus -p lang-kotlin --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (_, sources) = check_corpus(provider(), "A.kt", "kotlin", &["kt"]);
    let (_, scripts) = check_corpus(provider(), "A.kts", "kotlin", &["kts"]);
    // The PSI parser test inputs hold deliberate errors (`psi/` of the Kotlin repository).
    let unknown: Vec<_> = (sources.iter().chain(&scripts))
        .filter(|u| !u.contains("/testData/psi/"))
        .collect();
    assert!(
        unknown.is_empty(),
        "valid files with syntax errors: {unknown:#?}"
    );
}

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    core_format::testing::assert_formats(&Formatting::new(provider(), "A.kt"), input)
}

#[test]
fn spacing_follows_ktlint() {
    assert_eq!(
        fmt("fun f(a:Int,b : Int):Int{return a+b}\nval x=if(a)1 else 2\n"),
        "fun f(\n    a: Int,\n    b: Int,\n): Int = a + b\n\nval x = if (a) 1 else 2\n"
    );
    assert_eq!(fmt("val s = 0 .. 5\n"), "val s = 0..5\n");
}

#[test]
fn indentation_follows_the_nesting() {
    assert_eq!(
        fmt("class A {\nfun f() {\nif (a) {\nb()\n}\n}\n}\n"),
        "class A {\n    fun f() {\n        if (a) {\n            b()\n        }\n    }\n}\n"
    );
    assert_eq!(
        fmt("val x = foo(\n1,\n2\n)\nval s = \"\"\"\n  raw\n\"\"\"\n"),
        "val x =\n    foo(\n        1,\n        2,\n    )\nval s = \"\"\"\n  raw\n\"\"\"\n"
    );
}

#[test]
fn blank_lines_between_declarations() {
    assert_eq!(
        fmt(
            "package a\nimport b.C\nclass X {\n\n    val a = 1\n    val b = 2\n    fun f() {}\n    fun g() {\n\n        x()\n\n\n    }\n}\n"
        ),
        "package a\nimport b.C\n\nclass X {\n    val a = 1\n    val b = 2\n\n    fun f() {}\n\n    fun g() {\n        x()\n    }\n}\n"
    );
}
