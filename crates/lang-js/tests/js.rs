//! Parse, round trip and the shared format checks on the bench inputs and on probes of recent
//! syntax: every file parses without errors, prints back byte for byte, satisfies the trivia
//! attachment rule and has no `Unknown` token.

use core_syntax::print;
use lang_js::provider;
use provider_api::format_testing::{
    Formatting, assert_file_names, assert_parses, bench_input_files,
};
use provider_api::{LanguageProvider, SourceFile};

const TS: &str = r#"#!/usr/bin/env node
import type { A } from "./a";
import json from "./x.json" with { type: "json" };
export type * as ns from "./ns";
using res = getResource();
class C<in out T> {
  accessor x = 1;
  static { init(); }
  #p = 1;
  has(o: object) { return #p in o; }
  @dec() method<const U extends string>(this: C<T>, a?: U): asserts a is U {}
  declare readonly y: unique symbol;
  override z!: number;
}
type A<in out T> = T extends [infer U extends string] ? `x-${U}` : never; // trailing
enum E { A = 1, B }
declare module "m" { export const x: number }
namespace N.M { export const y = 1 }
const v = <const T,>(a: T) => a satisfies unknown as any;
let big = 1_000n, re = /[\p{L}--[a-z]]/v;
f?.<string>(x)!.y;
import foo = require("foo");
const inst = Map<string, number>;
await 1;
/* the end */
"#;

const TSX: &str = r#"const a = <T,>(x: T) => x;
const el = <ns:tag a:b="1" {...props}>{/* c */}text {x} <></></ns:tag>;
const t = <div className={`a ${b}`}>
  {list.map((x) => <li key={x}>{x}</li>)}
</div>;
"#;

const JS: &str = r#"#!/usr/bin/env node
"use strict";
const a = `t${1 + `n${2}`}`; // template
label: for (const x of y) { if (x) continue label; }
var r = a / b / c, s = /=/.test(x);
return a;
"#;

/// Every check on one file. `name` picks the dialect; the shared format checks use `subject`.
fn check(name: &str, text: &str, subject: &'static str) {
    assert_parses(&provider(), name, text);
    core_format::testing::assert_formats(&Formatting::new(provider(), subject), text);
}

#[test]
fn probes_of_recent_syntax() {
    check("a.ts", TS, "a.ts");
    check("a.tsx", TSX, "a.tsx");
    check("a.js", JS, "a.js");
}

#[test]
fn bench_inputs() {
    for (name, text) in bench_input_files("js") {
        let subject = match name.rsplit('.').next() {
            Some("ts") => "a.ts",
            Some("tsx") => "a.tsx",
            Some("jsx") => "a.jsx",
            _ => "a.js",
        };
        check(&name, &text, subject);
    }
}

#[test]
fn syntax_errors_are_reported_and_lossless() {
    for (name, text) in [
        ("a.js", "let x = ;\n"),
        ("a.ts", "class { \u{0} }"),
        ("a.js", "@@@"),
    ] {
        let parsed = provider().parse(&SourceFile::new(name, 1, text)).unwrap();
        assert!(!parsed.errors.is_empty(), "{text:?}");
        assert_eq!(print(&parsed.tree), text);
    }
}

#[test]
fn matches_the_extensions() {
    assert_file_names(
        &provider(),
        &[
            "a.js", "a.mjs", "a.cjs", "a.jsx", "a.ts", "a.mts", "a.cts", "a.tsx", "a.d.ts",
        ],
        &["a.json", "a.jsonc", "a.css"],
    );
}

/// The fields of the roles resolve to the parts a layout or binder reads, and the tree fits the
/// bodies.
#[test]
fn role_fields_resolve() {
    let p = provider();
    let g = p.grammar();
    let src = "a.b?.c(x, 1)[2]; x += y in z; i++; --j; new Foo(1);\nclass K { m(a) { return a ? 1 : 2 } }\n";
    let tree = p.parse(&SourceFile::new("a.ts", 1, src)).unwrap().tree;
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
    assert_eq!(field("CallExpression", 0, "callee").unwrap(), "a.b?.c");
    assert_eq!(field("CallExpression", 0, "arguments").unwrap(), "x");
    assert_eq!(field("NewExpression", 0, "callee").unwrap(), "Foo");
    assert_eq!(
        field("ComputedMemberExpression", 0, "arguments").unwrap(),
        "2"
    );
    assert_eq!(field("AssignmentExpression", 0, "operator").unwrap(), "+=");
    assert_eq!(field("BinaryExpression", 0, "operator").unwrap(), "in");
    assert_eq!(field("UpdateExpression", 0, "operator").unwrap(), "++");
    assert_eq!(field("UpdateExpression", 1, "operator").unwrap(), "--");
    assert_eq!(field("ConditionalExpression", 0, "else").unwrap(), "2");
    assert_eq!(
        field("Class", 0, "members").unwrap().chars().next(),
        Some('{')
    );
    assert_eq!(field("MethodDefinition", 0, "name").unwrap(), "m");
    assert_eq!(field("Function", 0, "parameters").unwrap(), "(a)");
}

/// Every corpus file that parses fits the bodies of `js.grammar`.
///
/// `bash tests/conformance/fetch-js-corpus.sh && cargo test --profile corpus -p lang-js --test integration -- --ignored fits_the_grammar`
#[test]
#[ignore]
fn corpus_fits_the_grammar() {
    provider_api::format_testing::check_corpus_shapes(
        provider(),
        "js",
        &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"],
    );
}
