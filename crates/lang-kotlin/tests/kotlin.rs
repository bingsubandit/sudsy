//! Parse, round trip and the shared format checks on the bench inputs and on probes of recent
//! syntax: every file parses without errors, prints back byte for byte, satisfies the trivia
//! attachment rule and has no `Unknown` token. The ignored test runs the same on the corpus
//! (`tests/conformance/fetch-kotlin-corpus.sh`).

use core_format::testing::Subject;
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

/// ktlint's import order: ASCII, `java.`, `javax.` and `kotlin.` after the others, aliases last,
/// no duplicates; a comment among the imports leaves the list as written.
#[test]
fn imports_sort_like_ktlint() {
    assert_eq!(
        fmt(
            "import kotlin.io.println\nimport b.C as D\nimport java.util.List\nimport a.B\nimport a.B\nimport z.Z\n"
        ),
        "import a.B\nimport z.Z\nimport java.util.List\nimport kotlin.io.println\nimport b.C as D\n"
    );
    assert_eq!(
        fmt("import b.C\n// x\nimport a.B\n"),
        "import b.C\n// x\nimport a.B\n"
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

/// A branch that fits on a line at the source's column but not after the indentation: formatting
/// the output changes nothing more.
#[test]
fn a_layout_wrapped_branch_is_stable() {
    let call = format!("callWithLongName({})", "x".repeat(112));
    // (not `fmt`: its meaning check compares with the rewrite of the input, not of the output)
    let formatting = Formatting::new(provider(), "A.kt");
    let out = Subject::format(&formatting, &format!("fun f() {{\nif (c) {call}\n}}\n")).unwrap();
    assert_eq!(Subject::format(&formatting, &out).unwrap(), out);
}

/// An annotated primary constructor goes below the name with everything after it one level in; a
/// raw string before `trimIndent()` takes the indentation of its code; the arguments of a
/// supertype call are one level deeper; the `;` after enum entries stays at the entries' level.
#[test]
fn ktlint_class_header_raw_string_and_enum_semicolon() {
    assert_eq!(
        fmt(
            "class A @Inject constructor(val a: Int) : B(1) {\n    val s = \"\"\"\n        x\n    \"\"\".trimIndent()\n}\n"
        ),
        "class A\n    @Inject\n    constructor(\n        val a: Int,\n    ) : B(1) {\n        val s =\n            \"\"\"\n            x\n            \"\"\".trimIndent()\n    }\n"
    );
    assert_eq!(
        fmt("enum class E {\n    A,\n    B,\n    ;\n}\n"),
        "enum class E {\n    A,\n    B,\n    ;\n}\n"
    );
}

/// Supertypes one per line after a `constructor` keyword put the body one level further in; a
/// constructor below the name keeps a single supertype on the line of its `)`.
#[test]
fn ktlint_body_after_wrapped_supertypes_and_own_line_constructor() {
    assert_eq!(
        fmt("class A private constructor() : B(), C {\n    fun f() {}\n}\n"),
        "class A private constructor() :\n    B(),\n    C {\n        fun f() {}\n    }\n"
    );
    assert_eq!(
        fmt("class A(val a: Int) : B(), C {\n    fun f() {}\n}\n"),
        "class A(\n    val a: Int,\n) : B(),\n    C {\n    fun f() {}\n}\n"
    );
    assert_eq!(
        fmt("class A\n@Inject\nconstructor(a: Int) :\n    B {\n    fun f() {}\n}\n"),
        "class A\n    @Inject\n    constructor(\n        a: Int,\n    ) : B {\n        fun f() {}\n    }\n"
    );
}

/// A raw string without a `trimIndent()`-like call keeps its bytes, and the safety check refuses
/// a re-indent of it; the same re-indent before `trimIndent()` is allowed.
#[test]
fn a_plain_raw_string_keeps_its_bytes() {
    let p = provider();
    let options = p.option_schema().defaults();
    let verify = |before: &str, after: &str| {
        let tree = p.parse(&SourceFile::new("A.kt", 1, before)).unwrap().tree;
        core_format::safety::verify(p.grammar(), p.formatter(), &options, &tree, after, |t| {
            p.parse(&SourceFile::new("A.kt", 1, t))
                .map(|r| r.tree)
                .map_err(|e| e.to_string())
        })
    };
    let plain = "fun f() {\n    val s = \"\"\"\n        a\n          b\n    \"\"\"\n}\n";
    let moved = "fun f() {\n    val s = \"\"\"\n    a\n      b\n    \"\"\"\n}\n";
    assert_eq!(
        Subject::format(&Formatting::new(provider(), "A.kt"), plain).unwrap(),
        plain
    );
    assert!(verify(plain, moved).is_err());
    let trim = plain.replace("\"\"\"\n}", "\"\"\".trimIndent()\n}");
    let trim_moved = moved.replace("\"\"\"\n}", "\"\"\".trimIndent()\n}");
    assert!(verify(&trim, &trim_moved).is_ok());
}

/// A `when` entry with several conditions keeps them at its own level; a comment after `{` of a `try`, `catch` or `finally` or
/// between `)` and a braced body moves inside, one after another `{` stays; `+=` moves a multi-line value below the operator.
#[test]
fn when_entries_comments_in_braces_and_compound_assignment() {
    let when = "fun f() {\n    when (x) {\n        B,\n        C,\n        -> {\n            g()\n        }\n    }\n}\n";
    assert_eq!(fmt(when), when);
    assert_eq!(
        fmt(
            "fun f() {\n    if (a) { // c\n        b()\n    }\n    try { // d\n        b()\n    } catch (e: E) { // e\n    }\n}\n"
        ),
        "fun f() {\n    if (a) { // c\n        b()\n    }\n    try {\n        // d\n        b()\n    } catch (e: E) {\n        // e\n    }\n}\n"
    );
    assert_eq!(
        fmt("fun f() {\n    t += g(1) {\n        h()\n    }\n}\n"),
        "fun f() {\n    t +=\n        g(1) {\n            h()\n        }\n}\n"
    );
}

/// A one-line block comment that ends its line is a line comment (also after the separator of a
/// parameter list that breaks); one with code after it stays; a trailing comment after a bare `if`
/// body stays on the body's line when the braces go in.
#[test]
fn single_line_block_comments_and_braced_bodies() {
    assert_eq!(
        fmt("val a = 1 /* c */\nval b = 2 /* d */ + 3\n"),
        "val a = 1 // c\nval b = 2 /* d */ + 3\n"
    );
    assert_eq!(fmt("/** doc */\nval a = 1\n"), "/** doc */\nval a = 1\n");
    let long = "fun f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Int, /* c */ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: Int, cccccccccccccccccccccccccccccc: Int, ddddddddddddd: Int /* e */) {}\n";
    let once = fmt(long);
    assert!(
        once.contains("Int, // c\n") && once.contains("Int, // e\n"),
        "{once}"
    );
    assert_eq!(fmt(&once), once);
    // (comments the layout moves to the end of a line: after `->`, after `(`)
    fmt("fun f(x: Int) = when (x) {\n    1 -> 2\n    else -> /* c */ g(x)\n}\n");
    fmt("class A(/* a */ val x: Int,\n        /* b */ val y: Int) : B() {}\n");
    assert_eq!(
        fmt(
            "fun f() {\n    if (a) {\n        w()\n    } else if (b) y = 2 /* c */\n    else z = 3 // d\n}\n"
        ),
        "fun f() {\n    if (a) {\n        w()\n    } else if (b) {\n        y = 2 // c\n    } else {\n        z = 3 // d\n    }\n}\n"
    );
}
