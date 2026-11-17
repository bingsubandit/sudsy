use provider_api::format_testing::{
    assert_file_names, assert_formats, assert_lossless, assert_parses, bench_input_files,
    check_corpus,
};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_java::provider(), "A.java", input)
}

/// Every `.java` file of the pinned corpus (`tests/conformance/fetch-java-corpus.sh`) parses,
/// formats with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-java --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (_, unparsed) = check_corpus(lang_java::provider(), "A.java", "java", &["java"]);
    // rezel 0.0.0 fails to lex a text block with an escaped `\"""` and to parse a chain of
    // about 300 method calls (docs/bugs.md).
    let known = ["FormatterTest.java", "WellKnownMutability.java"];
    let unknown: Vec<_> = unparsed
        .iter()
        .filter(|u| !known.iter().any(|k| u.contains(k)))
        .collect();
    assert!(
        unknown.is_empty(),
        "valid files with syntax errors: {unknown:#?}"
    );
    assert_eq!(unparsed.len(), known.len(), "a known failure parses now");
}

#[test]
fn file_names() {
    assert_file_names(
        &lang_java::provider(),
        &["A.java", "src/main/java/a/B.java"],
        &["A.class", "A.javascript", "pom.xml"],
    );
}

#[test]
fn bench_inputs_parse() {
    for (name, text) in bench_input_files("java") {
        assert_parses(&lang_java::provider(), &name, &text);
    }
}

/// Modern syntax: records, sealed types, switch patterns and guards, text blocks, modules.
#[test]
fn java_21_syntax_parses() {
    let p = lang_java::provider();
    assert_parses(
        &p,
        "A.java",
        r#"package a.b;
import static java.util.Map.*;
public sealed interface Shape permits Circle, Square {}
record Circle(double r) implements Shape { Circle { if (r < 0) throw new IllegalArgumentException(); } }
final class Square implements Shape { int s; }
class T<X extends Number & Comparable<X>> {
  @SuppressWarnings({"a", "b"}) <Y> Y f(Object o, int... xs) throws Exception {
    var t = """
        text \{o}
        """;
    return switch (o) {
      case Circle(double r) when r > 1 -> (Y) (Object) r;
      case String s -> null;
      default -> { yield null; }
    };
  }
}
"#,
    );
    assert_parses(
        &p,
        "module-info.java",
        "open module a.b { requires transitive c; exports d to e, f; }\n",
    );
}

#[test]
fn syntax_errors_stay_lossless() {
    let parsed = assert_lossless(&lang_java::provider(), "A.java", "class A { int x = ; \n");
    assert!(!parsed.errors.is_empty());
}

/// google-java-format's layout: blank lines between members, wrapped parameters, `throws` and
/// string concatenations, one-line chains, enum constants one per line.
#[test]
fn layout_follows_google_java_format() {
    let input = "package a.b;
import java.util.List;
class A<T> extends B implements C {
  private final int x=1;
  A(int x){this.x=x;}
  public <R> R fold(R init,java.util.function.BiFunction<R,? super T,R> f,boolean parallel,int limit) throws Exception{
    if(x>1)return init; else if(x<0){return null;}
    String s=\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"+init+\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"+limit+\"cccccccccccccccccccccccccccc\";
    return items.stream().filter(v->v!=null).map(Object::toString).collect(Collectors.toList());
  }
}
enum E { A, B, // two
  C; }
";
    assert_eq!(
        fmt(input),
        "package a.b;

import java.util.List;

class A<T> extends B implements C {
  private final int x = 1;

  A(int x) {
    this.x = x;
  }

  public <R> R fold(
      R init, java.util.function.BiFunction<R, ? super T, R> f, boolean parallel, int limit)
      throws Exception {
    if (x > 1) return init;
    else if (x < 0) {
      return null;
    }
    String s =
        \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"
            + init
            + \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"
            + limit
            + \"cccccccccccccccccccccccccccc\";
    return items.stream().filter(v -> v != null).map(Object::toString).collect(Collectors.toList());
  }
}

enum E {
  A,
  B, // two
  C;
}
"
    );
}

/// A line comment keeps its line: after an operator the operand moves below it, after `else`
/// the `if` does.
#[test]
fn line_comments_keep_their_line() {
    fmt("class A {
  void f() {
    int x = a + // first
        b;
    if (a) {
    } else // note
    if (b) {
    }
    foo(
        1, // one
        2);
  }
}
");
}

/// The AOSP style is four columns of indentation (continuations double it).
#[test]
fn aosp_is_a_four_column_indent() {
    let mut subject =
        provider_api::format_testing::Formatting::new(lang_java::provider(), "A.java");
    subject
        .options
        .push(("indent_size", core_config::OptionValue::Int(4)));
    assert_eq!(
        core_format::testing::assert_formats(&subject, "class A { void f() { g(); } }\n"),
        "class A {\n    void f() {\n        g();\n    }\n}\n"
    );
}

/// The fields of the roles resolve to the parts a layout or binder reads, and the tree fits the
/// bodies.
#[test]
fn role_fields_resolve() {
    use provider_api::{LanguageProvider, SourceFile};
    let p = lang_java::provider();
    let g = p.grammar();
    let src = "class A { int[] f(int a) { x = a > 1 ? a : -a; b.c(a).d[1] = new E(a); i++; return a; } }\n";
    let tree = p.parse(&SourceFile::new("A.java", 1, src)).unwrap().tree;
    core_syntax::shape::ShapeChecker::new(g)
        .check(&tree)
        .unwrap();
    let field = |kind: &str, nth: usize, field: &str| {
        let node = tree
            .descendants()
            .filter(|n| g.name(n.kind()) == kind)
            .nth(nth)
            .unwrap();
        g.slot(&node, field)
            .map(|e| e.to_string().trim().to_string())
    };
    assert_eq!(field("MethodDeclaration", 0, "name").unwrap(), "f");
    assert_eq!(field("ClassDeclarationCore", 0, "name").unwrap(), "A");
    assert_eq!(field("MethodInvocation", 0, "callee").unwrap(), "c");
    assert_eq!(field("MethodInvocation", 0, "object").unwrap(), "b");
    assert_eq!(field("ObjectCreationExpression", 0, "callee").unwrap(), "E");
    assert_eq!(field("AssignmentExpression", 0, "left").unwrap(), "x");
    assert_eq!(field("AssignmentExpression", 0, "operator").unwrap(), "=");
    assert_eq!(field("BinaryExpression", 0, "left").unwrap(), "a");
    assert_eq!(field("BinaryExpression", 0, "operator").unwrap(), ">");
    assert_eq!(field("TernaryExpression", 0, "else").unwrap(), "-a");
    assert_eq!(field("UpdateExpression", 0, "operator").unwrap(), "++");
    assert_eq!(field("ArrayAccess", 0, "arguments").unwrap(), "1");
}

/// Every corpus file that parses fits the bodies of `java.grammar`.
///
/// `cargo test --profile corpus -p lang-java --test integration -- --ignored fits_the_grammar`
#[test]
#[ignore]
fn corpus_fits_the_grammar() {
    provider_api::format_testing::check_corpus_shapes(lang_java::provider(), "java", &["java"]);
}
