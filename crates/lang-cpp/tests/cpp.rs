//! Lexing, the structure of the tree, and the shared checks (round trip, trivia attachment, the
//! formatter) on probes, the bench inputs and the corpus (`tests/conformance/fetch-cpp-corpus.sh`).

use core_syntax::{SyntaxKind, print};
use lang_cpp::generated::kind;
use lang_cpp::{Dialect, provider};
use provider_api::format_testing::{
    assert_file_names, assert_formats, assert_parses, bench_input_files, check_corpus,
};
use provider_api::{LanguageProvider, SourceFile};

const CPP: &str = r##"// Modern C++
module;
#include <vector>
#include "a.h"
#define MAX(a, b) ((a) > (b) ? (a) : (b)) \
    + 0
export module m;
namespace a::b { template <typename T> concept C = requires(T t) { { t + t } -> std::same_as<T>; };
auto f(auto x) -> decltype(x) { return x; } }
class Foo : public Bar {
public:
  Foo() : a_(1), b_{2} {}
  auto operator<=>(const Foo&) const = default;
  template <class... Ts> void g(Ts&&... ts) noexcept(true) { (h(ts), ...); }
 private:
  int a_ = 0x1'000, b_[3] = {1, 2, 3};
};
task<int> co() { co_await g(); co_return R"x(raw ")x"; }
auto l = [&, y = 1]<class T>(T t) mutable { return t + y; };
enum class E : unsigned { A = 1, B };
struct S { int a; } s{1}, *p = &s;
int main() try { if constexpr (true) { return 1; } else return 2; } catch (...) { throw; }
"##;

const C: &str = r##"/* Kernel style */
#ifndef _H
#define _H
#include <linux/types.h>
static int __init foo_init(void)
{
	struct s *p = &(struct s){ .a = 1, .b = { [0] = 2 } };
	if (x)
		return -EINVAL;
	else if (y) {
		goto out;
	}
	switch (a) {
	case 1:
		break;
	default:
		;
	}
out:
	return 0;
}
module_init(foo_init);
static DEFINE_MUTEX(lock);
typedef struct { int a; } t_t;
#endif /* _H */
"##;

#[test]
fn probes_parse_and_print_back() {
    assert_parses(&provider(Dialect::Cpp), "a.cc", CPP);
    assert_parses(&provider(Dialect::C), "a.c", C);
}

/// The formatter's own cases: spacing, wrapping, braces, blank lines, comments, directives.
const FORMAT: &str = r##"#include <a>
#define X(a)   \
  (a+1)
namespace n{
int x=1;


class C:public B{
public:
  C():a(1),b(2){}
 int f(int*p,int&q)const{return p?*p:q;}
private:
  int a,b;
};
enum E{A,B};
}
static int   very_long_function_name(int first_argument, int second_argument, int third_argument) {
  for(int i=0;i<10;++i){ if(i%2) continue; else break; }
  switch(x){case 1: y(); break; default: return;}
  while(x);
  auto l=[&](int a){return a+1;};
  call(a,b, c); // trailing
  /* block */
  return a?b:c;
}
"##;

#[test]
fn formats() {
    let out = assert_formats(provider(Dialect::Cpp), "a.cc", FORMAT);
    assert!(
        out.contains(
            "namespace n {\nint x = 1;\n\nclass C : public B {\npublic:\n  C() : a(1), b(2) {}\n"
        ),
        "{out}"
    );
    assert!(out.contains("enum E { A, B };\n}"), "{out}");
    assert!(out.contains("  while (x)\n    ;\n"), "{out}");
    assert!(
        out.contains("  switch (x) {\n  case 1:\n    y();\n    break;\n"),
        "{out}"
    );
    assert_formats(provider(Dialect::C), "a.c", C);
}

#[test]
fn formats_probes_and_bench_inputs() {
    assert_formats(provider(Dialect::Cpp), "a.cc", CPP);
    for (folder, dialect, file) in [("c", Dialect::C, "a.c"), ("cpp", Dialect::Cpp, "a.cc")] {
        for (_, text) in bench_input_files(folder) {
            assert_formats(provider(dialect), file, &text);
        }
    }
}

#[test]
fn file_names() {
    assert_file_names(
        &provider(Dialect::Cpp),
        &["a.cc", "a.cpp", "a.cxx", "a.hpp", "a.hh", "a.hxx", "a.ipp"],
        &["a.c", "a.h", "a.rs"],
    );
    assert_file_names(&provider(Dialect::C), &["a.c", "a.h"], &["a.cpp", "a.hpp"]);
}

#[test]
fn bench_inputs_parse() {
    for (folder, dialect) in [("c", Dialect::C), ("cpp", Dialect::Cpp)] {
        for (name, text) in bench_input_files(folder) {
            assert_parses(&provider(dialect), &name, &text);
        }
    }
}

/// The text of every `kind` node, blanks collapsed.
fn texts(src: &str, dialect: Dialect, wanted: SyntaxKind) -> Vec<String> {
    let parsed = provider(dialect)
        .parse(&SourceFile::new("a.cc", 1, src))
        .unwrap();
    assert_eq!(print(&parsed.tree), src);
    (parsed.tree.descendants())
        .filter(|n| n.kind() == wanted)
        .map(|n| {
            n.text()
                .to_string()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

#[test]
fn lines() {
    let src = "int a; int f(int x)\n{ if (x) return 1; else { x; } }\nstruct S { int a; } s;\npublic:\nBEGIN(x)\nint b;\n";
    assert_eq!(
        texts(src, Dialect::Cpp, kind::LINE),
        [
            "int a;",
            "int f(int x) { if (x) return 1; else { x; } }",
            "if (x)",
            "return 1;",
            "else { x; }",
            "x;",
            "struct S { int a; } s;",
            "int a;",
            "public:",
            "BEGIN(x)",
            "int b;",
        ]
    );
}

#[test]
fn c_has_no_cpp_keywords() {
    let src = "int new = 1; class x;";
    let keywords = |dialect| {
        let parsed = provider(dialect)
            .parse(&SourceFile::new("a", 1, src))
            .unwrap();
        (parsed.tree.descendants_with_tokens())
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == kind::KEYWORD)
            .count()
    };
    assert_eq!((keywords(Dialect::C), keywords(Dialect::Cpp)), (1, 3));
}

/// Branches that start and end at the same groups are a `PpGroup` of `PpBranch`es; when a
/// branch leaves a group open (`{` in both) or closes one from outside, only the `Directive`
/// nodes are there.
#[test]
fn conditional_groups() {
    let whole = "#if A\nint a;\n#else\nint b;\n#endif\n";
    assert_eq!(texts(whole, Dialect::Cpp, kind::PP_BRANCH).len(), 2);
    assert_eq!(texts(whole, Dialect::Cpp, kind::PP_GROUP).len(), 1);
    let open = "#if A\nvoid f(int a) {\n#else\nvoid f(long a) {\n#endif\n  g();\n}\n";
    assert!(texts(open, Dialect::Cpp, kind::PP_GROUP).is_empty());
    assert_eq!(texts(open, Dialect::Cpp, kind::BRACE_GROUP).len(), 2);
    let closing = "void f() {\n#if A\n}\n#endif\n";
    assert!(texts(closing, Dialect::Cpp, kind::PP_GROUP).is_empty());
    assert_eq!(texts(closing, Dialect::Cpp, kind::DIRECTIVE).len(), 2);
    let nested = "#if A\n#if B\nx;\n#endif\n#elif C\ny;\n#else\nz;\n#endif\n";
    assert_eq!(texts(nested, Dialect::Cpp, kind::PP_BRANCH).len(), 4);
}

#[test]
fn lexing() {
    let src = "#include <a/b.h>\n#define X(a) a##b \\\n  \"s\"\nint x = 1'000 + 0x1p-3; auto s = u8\"x\" R\"d(\n)d\"_s; char c = '\\'';\n";
    let parsed = assert_parses(&provider(Dialect::Cpp), "a.cc", src);
    let tokens: Vec<String> = (parsed.tree.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| {
            t.kind() == kind::HEADER_NAME
                || t.kind() == kind::STRING
                || t.kind() == kind::CHAR
                || t.kind() == kind::NUMBER
        })
        .map(|t| t.text().to_string())
        .collect();
    assert_eq!(
        tokens,
        [
            "<a/b.h>",
            "\"s\"",
            "1'000",
            "0x1p-3",
            "u8\"x\"",
            "R\"d(\n)d\"_s",
            "'\\''"
        ]
    );
    let directives = texts(src, Dialect::Cpp, kind::DIRECTIVE);
    assert_eq!(
        directives,
        ["#include <a/b.h>", "#define X(a) a##b \\ \"s\""]
    );
}

#[test]
fn never_an_error_but_an_unterminated_comment() {
    let ok = "@ ` \\ ) } ] ' \" x\n#else\n#endif\n#if\n";
    assert!(
        provider(Dialect::Cpp)
            .parse(&SourceFile::new("a.cc", 1, ok))
            .unwrap()
            .errors
            .is_empty()
    );
    let bad = provider(Dialect::Cpp)
        .parse(&SourceFile::new("a.cc", 1, "int a; /* x"))
        .unwrap();
    assert_eq!(bad.errors.len(), 1);
}

/// Every `.c` / `.h` and C++ file of the pinned corpus: `bash tests/conformance/fetch-cpp-corpus.sh &&
/// cargo test --profile corpus -p lang-cpp --test integration -- --ignored --nocapture conformance`.
#[test]
#[ignore = "needs the corpus: tests/conformance/fetch-cpp-corpus.sh"]
fn conformance_c() {
    let (count, unparsed) = check_corpus(provider(Dialect::C), "a.c", "cpp", &["c", "h"]);
    assert!(unparsed.is_empty(), "{count} files: {unparsed:?}");
}

#[test]
#[ignore = "needs the corpus: tests/conformance/fetch-cpp-corpus.sh"]
fn conformance_cpp() {
    let exts = ["cc", "cpp", "cxx", "hpp", "hh", "hxx", "ipp"];
    let (count, unparsed) = check_corpus(provider(Dialect::Cpp), "a.cc", "cpp", &exts);
    assert!(unparsed.is_empty(), "{count} files: {unparsed:?}");
}

/// Speed and the shape of the structure per project: `cargo test --profile corpus -p lang-cpp
/// --test integration -- --ignored --nocapture structure_stats`.
#[test]
#[ignore = "needs the corpus: tests/conformance/fetch-cpp-corpus.sh"]
fn structure_stats() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/corpus/cpp");
    for project in std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()) {
        let mut dirs = vec![project.clone()];
        let (mut files, mut bytes, mut secs) = (0, 0, 0.0);
        let (mut lines, mut ifs, mut groups, mut errors) = (0, 0, 0, 0);
        while let Some(d) = dirs.pop() {
            for path in std::fs::read_dir(d).unwrap().map(|e| e.unwrap().path()) {
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let dialect = match ext {
                    "c" | "h" => Dialect::C,
                    "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" | "ipp" => Dialect::Cpp,
                    _ => continue,
                };
                let start = std::time::Instant::now();
                let parsed = lang_cpp::parse(&text, dialect);
                secs += start.elapsed().as_secs_f64();
                files += 1;
                bytes += text.len();
                errors += parsed.errors.len();
                for n in parsed.tree.descendants() {
                    match n.kind() {
                        kind::LINE => lines += 1,
                        kind::PP_GROUP => groups += 1,
                        kind::DIRECTIVE => {
                            let mut words = (n.descendants_with_tokens())
                                .filter_map(|e| e.into_token())
                                .skip_while(|t| t.kind() != kind::HASH)
                                .filter(|t| matches!(t.kind(), kind::IDENT | kind::KEYWORD));
                            ifs += usize::from(
                                words.next().is_some_and(|t| t.text().starts_with("if")),
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        println!(
            "{}: files {files}, {:.1} MB/s, lines {lines}, #if {ifs} of which groups {groups}, errors {errors}",
            project.file_name().unwrap().to_string_lossy(),
            bytes as f64 / 1e6 / secs
        );
    }
}

/// Format check (round trip, meaning, idempotency) on the whole corpus.
#[test]
#[ignore = "needs the corpus: tests/conformance/fetch-cpp-corpus.sh"]
fn conformance_format() {
    let exts = ["cc", "cpp", "cxx", "hpp", "hh", "hxx", "ipp"];
    check_corpus(provider(Dialect::Cpp), "a.cc", "cpp", &exts);
    check_corpus(provider(Dialect::C), "a.c", "cpp", &["c", "h"]);
}
