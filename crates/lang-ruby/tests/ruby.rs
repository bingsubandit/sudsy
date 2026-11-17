use provider_api::format_testing::{
    assert_file_names, assert_formats, assert_lossless, assert_parses, bench_input_files,
    check_corpus,
};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_ruby::provider(), "a.rb", input)
}

/// Every Ruby file of the pinned corpus (`tests/conformance/fetch-ruby-corpus.sh`) parses, fits
/// the grammar's bodies, formats with the same tokens and idempotently.
///
/// `cargo test --profile corpus -p lang-ruby --test integration -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (_, unparsed) = check_corpus(
        lang_ruby::provider(),
        "a.rb",
        "ruby",
        &["rb", "rake", "gemspec", "ru"],
    );
    assert!(
        unparsed.is_empty(),
        "valid files with syntax errors: {unparsed:#?}"
    );
}

/// Parse speed over the corpus, tokens and tree included (best of 5 passes, files in memory):
/// `cargo test --release -p lang-ruby --test integration -- --ignored --nocapture parse_speed`
#[test]
#[ignore]
fn parse_speed() {
    use provider_api::{LanguageProvider, SourceFile};
    let mut dirs = vec![
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/conformance/corpus/ruby"),
    ];
    let mut texts = Vec::new();
    while let Some(dir) = dirs.pop() {
        for path in std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()) {
            if path.is_dir() {
                dirs.push(path);
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                texts.push(text);
            }
        }
    }
    let (provider, bytes) = (
        lang_ruby::provider(),
        texts.iter().map(String::len).sum::<usize>(),
    );
    let best = (0..5)
        .map(|_| {
            let started = std::time::Instant::now();
            for text in &texts {
                std::hint::black_box(provider.parse(&SourceFile::new("a.rb", 1, text)).unwrap());
            }
            started.elapsed()
        })
        .min()
        .unwrap();
    println!(
        "{} files, {} MB: {:.1} MB/s",
        texts.len(),
        bytes / 1_000_000,
        bytes as f64 / 1e6 / best.as_secs_f64()
    );
}

#[test]
fn file_names() {
    assert_file_names(
        &lang_ruby::provider(),
        &[
            "a.rb",
            "lib/tasks/a.rake",
            "a.gemspec",
            "config.ru",
            "Gemfile",
            "app/Rakefile",
        ],
        &["a.rbs", "a.erb", "Gemfile.lock", "a.py"],
    );
}

#[test]
fn bench_inputs_parse() {
    for (name, text) in bench_input_files("rb") {
        assert_parses(&lang_ruby::provider(), &name, &text);
    }
}

#[test]
fn verbatim_format_keeps_the_file() {
    fmt("class A\n  def f(x) = x + 1\nend\n");
    fmt("puts <<~EOS\n  a #{b}\nEOS\n__END__\nraw  data\n");
}

/// Syntax of Ruby 3.3 and 3.4: pattern matching, endless methods, `it` and `_1`, hash shorthand,
/// anonymous argument forwarding, rescue modifier, `BEGIN`, flip-flop, `%` literals, rational and
/// imaginary numbers, safe navigation, lambdas, multiple assignment.
#[test]
fn modern_syntax_parses() {
    assert_parses(
        &lang_ruby::provider(),
        "a.rb",
        r##"#!/usr/bin/env ruby
# frozen_string_literal: true
BEGIN { puts 1 }
module A::B
  class C < D
    Point = Data.define(:x, :y)
    def hi(n) = puts "x#{n}"
    def fwd(*, **, &) = g(*, **, &)
    def all(...) = g(...)
    def self.[](k, v = 1, *r, a:, b: 2, **o, &blk); super; end
    def x = @x ||= [1, 2.5r, 3i, 0x1f, -4, ?a, :sym, %w[a b], %i[c d], %q(e), %r{f}i].map { it + 1 }
    def y = [1].map { _1 + 1 }
    def m(h) = case h
      in { name: String => n, age: 18.. } then n
      in [Integer => a, *rest] if a > 1 then rest
      in [*, 3, *] | nil then 0
      in ^(1 + 1) then 2
      else raise ArgumentError, "no" rescue nil
    end
  end
end
x = { a:, "b": 1, 'c' => 2, **o }
a, (b, *c), d = 1, [2, 3], 4
y = foo&.bar(1)&.baz { |q, (r, s), t = 1; u| q } || -> (z) { z }.()
puts "#{x}" "b" 'c'
(1..10).each { |i| puts i if (i == 3)..(i == 5) }
begin; 1; rescue A, B => e; 2; else 3; ensure 4; end
puts(<<~A, <<-B, <<'C')
  a #{x}
A
  b
  B
raw #{no}
C
z = [<<~ONE.strip, 2]
  one
ONE
puts 1 \
  + 2
=begin
block comment
=end
alias foo bar
undef baz
p __FILE__, __LINE__, __method__, $stdout, $1, $~, @@c
"##,
    );
}

/// A file with `__END__`, `=begin` blocks, CRLF line ends, heredocs at the end of the file and no
/// final newline keeps every byte.
#[test]
fn odd_files_are_lossless() {
    let p = lang_ruby::provider();
    for (name, text) in [
        ("empty", ""),
        ("comment", "# only\n"),
        ("end", "puts 1\n__END__\nraw data\nwith  #{no} parsing\n"),
        ("end-empty", "puts 1\n__END__\n"),
        ("end-eof", "puts 1\n__END__"),
        ("block", "=begin\nx\n=end\nputs 1\n=begin\ny\n=end"),
        ("crlf", "a = 1\r\nputs <<~A\r\n  x\r\nA\r\nb\r\n"),
        ("heredoc-eof", "puts <<~A\n  x\nA"),
        ("heredoc-last", "def f\n  puts <<~A\n    x\n  A\nend\n"),
        (
            "heredoc-chain",
            "a = <<~A.lines.map(&:strip).join(\n  x\nA\n)\n",
        ),
        ("heredoc-two", "f(<<A, <<B)\na\nA\nb\nB\n"),
        ("heredoc-empty", "f(<<A)\nA\n"),
        ("heredoc-nested", "f(<<A)\n#{<<B}\nb\nB\nA\n"),
        ("bom", "\u{feff}puts 1\n"),
    ] {
        let parsed = assert_lossless(&p, "a.rb", text);
        assert!(parsed.errors.is_empty(), "{name}: {:?}", parsed.errors);
    }
}

/// Prism recurses on the C stack up to 10,000 levels: nesting the guard does not count
/// (`!`, `not`, `if`, `def`) parses on a small thread stack, the tree is lossless even where Prism gives up.
#[test]
fn deep_nesting_parses_on_a_small_stack() {
    let texts = [
        "!".repeat(9000) + "x\n",
        "not ".repeat(9000) + "x\n",
        "if a\n".repeat(5000) + &"end\n".repeat(5000),
        "def f\n".repeat(5000) + &"end\n".repeat(5000),
        "!".repeat(20000) + "x\n",
    ];
    for text in texts {
        std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn(move || drop(assert_lossless(&lang_ruby::provider(), "a.rb", &text)))
            .unwrap()
            .join()
            .unwrap();
    }
}

/// A file with a syntax error is a tree with errors: lossless, and refused by the formatter.
#[test]
fn syntax_errors_are_reported() {
    let p = lang_ruby::provider();
    for text in [
        "def f(\n",
        "class A\n  def x\nend\n",
        "foo(1,\n",
        "x = [1, 2\n",
        "puts <<~A\nno end\n",
        "\"abc\n",
    ] {
        let parsed = assert_lossless(&p, "a.rb", text);
        assert!(!parsed.errors.is_empty(), "{text:?}");
    }
}
